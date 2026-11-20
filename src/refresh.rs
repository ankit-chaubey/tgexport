//! Fresh file references. A reference stored in a message expires after a
//! while, so when a download fails because of it, the message (or story, or
//! emoji) is requested again and the new reference is used.

use ferogram::tl;
use tl::enums as E;

use crate::api::{Api, Res};
use crate::model::*;

impl Api {
    /// Requests the origin of a file again and returns every photo and
    /// document found in it, with up to date references.
    pub async fn refresh(&mut self, origin: &Origin) -> Res<Vec<Source>> {
        let mut out = Vec::new();
        match origin {
            Origin::None => {}
            Origin::Message { peer, id, split } => {
                let ids = vec![E::InputMessage::Id(tl::types::InputMessageId { id: *id })];
                let res = match peer {
                    PeerId::Channel(c) => {
                        let access_hash = self.peers.get(*peer).map(|p| p.access_hash).unwrap_or(0);
                        self.main(tl::functions::channels::GetMessages {
                            channel: E::InputChannel::InputChannel(tl::types::InputChannel {
                                channel_id: *c,
                                access_hash,
                            }),
                            id: ids,
                        })
                        .await?
                    }
                    _ => {
                        self.split(*split, tl::functions::messages::GetMessages { id: ids })
                            .await?
                    }
                };
                let (messages, chats, users) = match res {
                    E::messages::Messages::Messages(m) => (m.messages, m.chats, m.users),
                    E::messages::Messages::Slice(m) => (m.messages, m.chats, m.users),
                    E::messages::Messages::ChannelMessages(m) => (m.messages, m.chats, m.users),
                    E::messages::Messages::NotModified(_) => return Ok(out),
                };
                self.peers.absorb_users(&users);
                self.peers.absorb_chats(&chats);
                for m in &messages {
                    match m {
                        E::Message::Message(m) if m.id == *id => {
                            if let Some(media) = &m.media {
                                collect_media(media, &mut out);
                            }
                        }
                        E::Message::Service(m) if m.id == *id => {
                            if let E::MessageAction::ChatEditPhoto(a) = &m.action {
                                if let E::Photo::Photo(p) = &a.photo {
                                    out.push(Source::Photo(p.clone()));
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            Origin::Story { id } => {
                let res = self
                    .main(tl::functions::stories::GetStoriesById {
                        peer: E::InputPeer::PeerSelf,
                        id: vec![*id],
                    })
                    .await?;
                let E::stories::Stories::Stories(s) = res;
                for item in &s.stories {
                    if let E::StoryItem::StoryItem(st) = item {
                        collect_media(&st.media, &mut out);
                    }
                }
            }
            Origin::RichMessage { peer, id } => {
                if let Some(E::RichMessage::RichMessage(rich)) =
                    self.full_rich_message(*peer, *id).await?
                {
                    for photo in &rich.photos {
                        if let E::Photo::Photo(p) = photo {
                            out.push(Source::Photo(p.clone()));
                        }
                    }
                    for doc in &rich.documents {
                        if let E::Document::Document(d) = doc {
                            out.push(Source::Document(d.clone()));
                        }
                    }
                }
            }
            Origin::CustomEmoji { id } => {
                for doc in self.custom_emoji_docs(&[*id]).await? {
                    if let E::Document::Document(d) = doc {
                        out.push(Source::Document(d));
                    }
                }
            }
        }
        Ok(out)
    }

    /// The complete rich message of a message. Long ones arrive as a part
    /// first; this asks for the whole content. Returns `None` unless the
    /// answer holds exactly one matching message with a complete rich message.
    pub async fn full_rich_message(
        &mut self,
        peer: PeerId,
        id: i32,
    ) -> Res<Option<E::RichMessage>> {
        let res = self
            .main(tl::functions::messages::GetRichMessage {
                peer: self.peers.input_peer(peer),
                id,
            })
            .await?;
        let (messages, chats, users) = match res {
            E::messages::Messages::Messages(m) => (m.messages, m.chats, m.users),
            E::messages::Messages::Slice(m) => (m.messages, m.chats, m.users),
            E::messages::Messages::ChannelMessages(m) => (m.messages, m.chats, m.users),
            E::messages::Messages::NotModified(_) => return Ok(None),
        };
        self.peers.absorb_users(&users);
        self.peers.absorb_chats(&chats);
        let mut matches = 0;
        let mut found = None;
        for entry in &messages {
            let (entry_id, rich) = match entry {
                E::Message::Message(m) => (m.id, m.rich_message.as_ref()),
                E::Message::Service(m) => (m.id, None),
                E::Message::Empty(m) => (m.id, None),
            };
            if entry_id != id {
                continue;
            }
            matches += 1;
            if let Some(r) = rich {
                let E::RichMessage::RichMessage(d) = r;
                if !d.part {
                    found = Some(r.clone());
                }
            }
        }
        Ok(if matches == 1 { found } else { None })
    }

    /// Looks up the sticker documents behind custom emoji ids.
    pub async fn custom_emoji_docs(&self, ids: &[i64]) -> Res<Vec<E::Document>> {
        self.main(tl::functions::messages::GetCustomEmojiDocuments {
            document_id: ids.to_vec(),
        })
        .await
    }
}

fn collect_media(media: &E::MessageMedia, out: &mut Vec<Source>) {
    match media {
        E::MessageMedia::Photo(m) => {
            if let Some(E::Photo::Photo(p)) = &m.photo {
                out.push(Source::Photo(p.clone()));
            }
        }
        E::MessageMedia::Document(m) => {
            if let Some(E::Document::Document(d)) = &m.document {
                out.push(Source::Document(d.clone()));
            }
        }
        _ => {}
    }
}

/// Points `source` at the matching entry of `fresh`. Returns false when the
/// same photo or document is not among them.
pub fn rebind(source: &mut Source, fresh: &[Source]) -> bool {
    for f in fresh {
        match (&*source, f) {
            (Source::Photo(old), Source::Photo(new)) if old.id == new.id => {
                *source = Source::Photo(new.clone());
                return true;
            }
            (Source::Document(old), Source::Document(new)) if old.id == new.id => {
                *source = Source::Document(new.clone());
                return true;
            }
            (Source::DocumentThumb(old, kind), Source::Document(new)) if old.id == new.id => {
                *source = Source::DocumentThumb(new.clone(), kind.clone());
                return true;
            }
            _ => {}
        }
    }
    false
}
