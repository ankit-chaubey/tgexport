//! Requests for the account level sections: contacts, frequent contacts,
//! sessions, stories and profile music.

use ferogram::tl;
use tl::enums as E;

use crate::api::{Api, Res, SLICE_LIMIT};
use crate::model::*;

impl Api {
    /// Phone contacts saved in the account.
    pub async fn saved_contacts(&self) -> Res<Vec<Contact>> {
        let list = self.main(tl::functions::contacts::GetSaved {}).await?;
        Ok(list
            .into_iter()
            .map(|E::SavedContact::SavedPhoneContact(c)| Contact {
                first_name: c.first_name,
                last_name: c.last_name,
                phone: if c.phone.is_empty() || c.phone.starts_with('+') {
                    c.phone
                } else {
                    format!("+{}", c.phone)
                },
                date: c.date,
            })
            .collect())
    }

    /// People, inline bots and calls the user talks to most.
    pub async fn top_peers(&mut self, out: &mut ContactsList) -> Res<()> {
        let mut offset = 0;
        loop {
            let res = self
                .main(tl::functions::contacts::GetTopPeers {
                    correspondents: true,
                    bots_pm: false,
                    bots_inline: true,
                    phone_calls: true,
                    forward_users: false,
                    forward_chats: false,
                    groups: false,
                    channels: false,
                    bots_app: false,
                    bots_guestchat: false,
                    offset,
                    limit: SLICE_LIMIT,
                    hash: 0,
                })
                .await?;
            let E::contacts::TopPeers::TopPeers(top) = res else {
                // Not modified, or the user turned frequent contacts off.
                return Ok(());
            };
            self.peers.absorb_users(&top.users);
            self.peers.absorb_chats(&top.chats);

            let mut loaded = true;
            for category in &top.categories {
                let E::TopPeerCategoryPeers::TopPeerCategoryPeers(c) = category;
                let entries: Vec<TopPeerEntry> = c
                    .peers
                    .iter()
                    .map(|E::TopPeer::TopPeer(t)| {
                        let id = peer_from_tl(&t.peer);
                        TopPeerEntry {
                            id,
                            name: self.peers.name(id),
                            kind: self.peer_kind(id),
                            rating: t.rating,
                        }
                    })
                    .collect();
                let target = match c.category {
                    E::TopPeerCategory::Correspondents => &mut out.correspondents,
                    E::TopPeerCategory::BotsInline => &mut out.inline_bots,
                    E::TopPeerCategory::PhoneCalls => &mut out.phone_calls,
                    _ => continue,
                };
                target.extend(entries);
                if offset + (c.peers.len() as i32) < c.count {
                    loaded = false;
                }
            }
            if loaded {
                return Ok(());
            }
            offset = out
                .correspondents
                .len()
                .max(out.inline_bots.len())
                .max(out.phone_calls.len()) as i32;
        }
    }

    fn peer_kind(&self, id: PeerId) -> &'static str {
        match id {
            PeerId::User(_) => "user",
            PeerId::Chat(_) => "private_group",
            PeerId::Channel(_) => {
                let info = self.peers.get(id);
                let public = info.map(|p| !p.username.is_empty()).unwrap_or(false);
                let supergroup = info.map(|p| p.is_megagroup).unwrap_or(false);
                match (supergroup, public) {
                    (true, true) => "public_supergroup",
                    (true, false) => "private_supergroup",
                    (false, true) => "public_channel",
                    (false, false) => "private_channel",
                }
            }
        }
    }

    /// Active sessions and the websites the account is logged in on.
    pub async fn sessions(&mut self) -> Res<SessionsList> {
        let mut out = SessionsList::default();
        let E::account::Authorizations::Authorizations(auth) = self
            .main(tl::functions::account::GetAuthorizations {})
            .await?;
        for a in auth.authorizations {
            let E::Authorization::Authorization(a) = a;
            out.list.push(Session {
                last_active: a.date_active,
                ip: a.ip,
                country: a.country,
                region: a.region,
                app_name: a.app_name,
                app_version: a.app_version,
                device_model: a.device_model,
                platform: a.platform,
                system_version: a.system_version,
                created: a.date_created,
            });
        }

        let E::account::WebAuthorizations::WebAuthorizations(web) = self
            .main(tl::functions::account::GetWebAuthorizations {})
            .await?;
        self.peers.absorb_users(&web.users);
        for a in web.authorizations {
            let E::WebAuthorization::WebAuthorization(a) = a;
            let bot_username = self
                .peers
                .get(PeerId::User(a.bot_id))
                .map(|p| p.username.clone())
                .unwrap_or_default();
            out.web_list.push(WebSession {
                last_active: a.date_active,
                ip: a.ip,
                region: a.region,
                domain: a.domain,
                browser: a.browser,
                platform: a.platform,
                created: a.date_created,
                bot_username,
            });
        }
        Ok(out)
    }

    /// One page of the story archive, newest first. Returns the stories and the
    /// id to continue from (0 when there is nothing more).
    pub async fn stories_page(&mut self, offset_id: i32) -> Res<(Vec<E::StoryItem>, i32)> {
        let res = self
            .main(tl::functions::stories::GetStoriesArchive {
                peer: E::InputPeer::PeerSelf,
                offset_id,
                limit: SLICE_LIMIT,
            })
            .await?;
        let E::stories::Stories::Stories(s) = res;
        self.peers.absorb_users(&s.users);
        self.peers.absorb_chats(&s.chats);
        let last = s.stories.last().map(|item| match item {
            E::StoryItem::Deleted(d) => d.id,
            E::StoryItem::Skipped(d) => d.id,
            E::StoryItem::StoryItem(d) => d.id,
        });
        Ok((s.stories, last.unwrap_or(0)))
    }

    /// One page of the music saved on the profile.
    pub async fn profile_music_page(&self, offset: i32) -> Res<Vec<E::Document>> {
        let res = self
            .main(tl::functions::users::GetSavedMusic {
                id: E::InputUser::UserSelf,
                offset,
                limit: SLICE_LIMIT,
                hash: 0,
            })
            .await?;
        Ok(match res {
            E::users::SavedMusic::SavedMusic(m) => m.documents,
            _ => Vec::new(),
        })
    }
}
