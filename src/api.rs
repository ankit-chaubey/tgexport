//! Everything that talks to Telegram. All requests go through a takeout
//! session, and message requests are also limited to one message range (a
//! "split", as reported by `messages.getSplitRanges`).

use std::path::Path;
use std::time::Duration;

use ferogram::{Client, InvocationError, tl};
use tl::enums as E;
use tokio::io::AsyncWriteExt;

use crate::model::*;

pub type Res<T> = Result<T, InvocationError>;

/// Size of a single download request.
const FILE_CHUNK: i32 = 128 * 1024;
pub const SLICE_LIMIT: i32 = 100;
/// Largest file size takeout accepts (4000 MB).
pub const FILE_MAX_SIZE: i64 = 4000 * 1024 * 1024;

#[derive(Clone, Copy, Default, Debug)]
pub struct TakeoutFlags {
    pub contacts: bool,
    pub message_users: bool,
    pub message_chats: bool,
    pub message_megagroups: bool,
    pub message_channels: bool,
    pub files: bool,
    pub file_max_size: Option<i64>,
}

/// `invokeWithTakeout`. The generated `InvokeWithTakeout<X>` returns the query
/// type itself instead of the query's result, so it cannot wrap a request.
/// This one can.
pub struct WithTakeout<R> {
    pub takeout_id: i64,
    pub query: R,
}

impl<R: tl::Serializable> tl::Serializable for WithTakeout<R> {
    fn serialize(&self, buf: &mut impl Extend<u8>) {
        0xaca9fd2eu32.serialize(buf);
        self.takeout_id.serialize(buf);
        self.query.serialize(buf);
    }
}

impl<R: tl::RemoteCall> tl::RemoteCall for WithTakeout<R> {
    type Return = R::Return;
}

/// `invokeWithMessagesRange`, for the same reason as `WithTakeout`.
pub struct WithRange<R> {
    pub range: tl::types::MessageRange,
    pub query: R,
}

impl<R: tl::Serializable> tl::Serializable for WithRange<R> {
    fn serialize(&self, buf: &mut impl Extend<u8>) {
        0x365275f2u32.serialize(buf);
        E::MessageRange::MessageRange(self.range.clone()).serialize(buf);
        self.query.serialize(buf);
    }
}

impl<R: tl::RemoteCall> tl::RemoteCall for WithRange<R> {
    type Return = R::Return;
}

pub enum Download {
    Saved(u64),
    Unavailable,
}

pub struct Api {
    pub client: Client,
    pub takeout_id: i64,
    pub splits: Vec<tl::types::MessageRange>,
    pub peers: Peers,
    pub self_id: i64,
}

pub fn rpc_name(e: &InvocationError) -> Option<&str> {
    match e {
        InvocationError::Rpc(r) => Some(r.name.as_str()),
        _ => None,
    }
}

impl Api {
    /// Loads the current user and opens the takeout session. If Telegram wants
    /// the request confirmed from another device (`TAKEOUT_INIT_DELAY`), we ask
    /// the user to do that and keep retrying.
    pub async fn start(client: Client, flags: TakeoutFlags) -> Res<Api> {
        let me = client
            .invoke(&tl::functions::users::GetUsers {
                id: vec![E::InputUser::UserSelf],
            })
            .await?;
        let mut peers = Peers::default();
        peers.absorb_users(&me);
        let self_id = match me.first() {
            Some(E::User::User(u)) => u.id,
            _ => 0,
        };
        if let Some(p) = peers.map.get_mut(&PeerId::User(self_id)) {
            p.is_self = true;
        }

        let mut waited = false;
        let takeout_id = loop {
            let req = tl::functions::account::InitTakeoutSession {
                contacts: flags.contacts,
                message_users: flags.message_users,
                message_chats: flags.message_chats,
                message_megagroups: flags.message_megagroups,
                message_channels: flags.message_channels,
                files: flags.files,
                file_max_size: if flags.files {
                    flags.file_max_size
                } else {
                    None
                },
            };
            match client.invoke(&req).await {
                Ok(E::account::Takeout::Takeout(t)) => break t.id,
                Err(e) if rpc_name(&e) == Some("TAKEOUT_INIT_DELAY") => {
                    if !waited {
                        eprintln!(
                            "Telegram requires confirmation of the export request.\n\
                             Open Telegram on another device and tap 'Allow' on the \
                             data export notification. Waiting..."
                        );
                        waited = true;
                    }
                    tokio::time::sleep(Duration::from_secs(15)).await;
                }
                Err(e) => return Err(e),
            }
        };

        let mut api = Api {
            client,
            takeout_id,
            splits: Vec::new(),
            peers,
            self_id,
        };
        api.load_splits().await?;
        Ok(api)
    }

    async fn load_splits(&mut self) -> Res<()> {
        let ranges = self
            .main(tl::functions::messages::GetSplitRanges {})
            .await?;
        self.splits = ranges
            .into_iter()
            .map(|E::MessageRange::MessageRange(r)| r)
            .collect();
        if self.splits.is_empty() {
            self.splits.push(tl::types::MessageRange {
                min_id: 1,
                max_id: i32::MAX,
            });
        }
        Ok(())
    }

    /// Sends a request inside the takeout session.
    pub async fn main<R: tl::RemoteCall>(&self, query: R) -> Res<R::Return> {
        self.client
            .invoke(&WithTakeout {
                takeout_id: self.takeout_id,
                query,
            })
            .await
    }

    /// Same, but restricted to one message range.
    pub async fn split<R: tl::RemoteCall>(&self, index: usize, query: R) -> Res<R::Return> {
        let range = self.splits[index].clone();
        self.main(WithRange { range, query }).await
    }

    pub async fn finish(&self, success: bool) {
        let _ = self
            .client
            .invoke(&tl::functions::account::FinishTakeoutSession { success })
            .await;
    }

    /// Lists chats, going through each split. With `only_last_split` only the
    /// newest range is read, which is enough when just channels are wanted.
    pub async fn dialogs(&mut self, only_last_split: bool) -> Res<Vec<DialogInfo>> {
        let mut out: Vec<DialogInfo> = Vec::new();
        let mut index_by_peer: std::collections::HashMap<PeerId, usize> = Default::default();
        let first = if only_last_split {
            self.splits.len() - 1
        } else {
            0
        };
        for split in first..self.splits.len() {
            let mut offset_date = 0;
            let mut offset_id = 0;
            let mut offset_peer = E::InputPeer::Empty;
            loop {
                let res = self
                    .split(
                        split,
                        tl::functions::messages::GetDialogs {
                            exclude_pinned: false,
                            folder_id: None,
                            offset_date,
                            offset_id,
                            offset_peer: offset_peer.clone(),
                            limit: SLICE_LIMIT,
                            hash: 0,
                        },
                    )
                    .await?;
                let (dialogs, messages, chats, users, last_slice) = match res {
                    E::messages::Dialogs::Dialogs(d) => {
                        (d.dialogs, d.messages, d.chats, d.users, true)
                    }
                    E::messages::Dialogs::Slice(d) => {
                        (d.dialogs, d.messages, d.chats, d.users, false)
                    }
                    E::messages::Dialogs::NotModified(_) => break,
                };
                self.peers.absorb_users(&users);
                self.peers.absorb_chats(&chats);
                if dialogs.is_empty() {
                    break;
                }
                let mut last: Option<(i32, i32, PeerId)> = None;
                for d in &dialogs {
                    let E::Dialog::Dialog(d) = d else { continue };
                    let peer = peer_from_tl(&d.peer);
                    for m in &messages {
                        let (id, date, mp) = match m {
                            E::Message::Message(m) => (m.id, m.date, peer_from_tl(&m.peer_id)),
                            E::Message::Service(m) => (m.id, m.date, peer_from_tl(&m.peer_id)),
                            E::Message::Empty(_) => continue,
                        };
                        if id == d.top_message && mp == peer {
                            last = Some((id, date, peer));
                        }
                    }
                    if let Some(info) = self.make_dialog_info(peer) {
                        let idx = match index_by_peer.get(&info.peer) {
                            Some(&i) => i,
                            None => {
                                index_by_peer.insert(info.peer, out.len());
                                out.push(info);
                                out.len() - 1
                            }
                        };
                        out[idx].splits.push(split);
                    }
                }
                if last_slice {
                    break;
                }
                match last {
                    Some((id, date, peer)) => {
                        offset_id = id;
                        offset_date = date;
                        offset_peer = self.peers.input_peer(peer);
                    }
                    None => break,
                }
            }
        }
        Ok(out)
    }

    /// Works out the chat type (personal, bot, group, channel, ...) of a peer.
    fn make_dialog_info(&self, peer: PeerId) -> Option<DialogInfo> {
        let info = self.peers.get(peer)?;
        let (dtype, name) = match peer {
            PeerId::User(_) => {
                if info.is_self {
                    (DialogType::Saved, "Saved Messages".to_string())
                } else if info.is_bot {
                    (DialogType::Bot, info.name())
                } else {
                    (DialogType::Personal, info.name())
                }
            }
            PeerId::Chat(_) => (DialogType::PrivateGroup, info.name()),
            PeerId::Channel(_) => {
                let public = !info.username.is_empty();
                let t = if info.is_megagroup {
                    if public {
                        DialogType::PublicSupergroup
                    } else {
                        DialogType::PrivateSupergroup
                    }
                } else if public {
                    DialogType::PublicChannel
                } else {
                    DialogType::PrivateChannel
                };
                (t, info.name())
            }
        };
        Some(DialogInfo {
            peer,
            dtype,
            name,
            is_left: info.left,
            only_my_messages: false,
            migrated_from: None,
            relative_path: String::new(),
            splits: Vec::new(),
        })
    }

    /// Channels the user has left. They are not part of the normal chat list.
    pub async fn left_channels(&mut self) -> Res<Vec<DialogInfo>> {
        let mut out = Vec::new();
        let mut offset = 0;
        loop {
            let res = self
                .main(tl::functions::channels::GetLeftChannels { offset })
                .await?;
            let (chats, partial) = match res {
                E::messages::Chats::Chats(c) => (c.chats, false),
                E::messages::Chats::Slice(c) => (c.chats, true),
            };
            if chats.is_empty() {
                break;
            }
            offset += chats.len() as i32;
            self.peers.absorb_chats(&chats);
            for c in &chats {
                if let E::Chat::Channel(c) = c {
                    if let Some(mut d) = self.make_dialog_info(PeerId::Channel(c.id)) {
                        d.is_left = true;
                        d.only_my_messages = true;
                        d.splits = vec![self.splits.len() - 1];
                        out.push(d);
                    }
                }
            }
            if !partial {
                break;
            }
        }
        Ok(out)
    }

    /// Fetches up to 100 messages with ids from `offset_id` upward, oldest first.
    /// Start with 1, then continue from the highest id seen plus one.
    pub async fn history_page(
        &mut self,
        peer: PeerId,
        split: usize,
        offset_id: i32,
        only_mine: bool,
    ) -> Res<Vec<E::Message>> {
        let input = self.peers.input_peer(peer);
        let res = if only_mine {
            self.split(
                split,
                tl::functions::messages::Search {
                    peer: input,
                    q: String::new(),
                    from_id: Some(E::InputPeer::PeerSelf),
                    saved_peer_id: None,
                    saved_reaction: None,
                    top_msg_id: None,
                    filter: E::MessagesFilter::InputMessagesFilterEmpty,
                    min_date: 0,
                    max_date: 0,
                    offset_id,
                    add_offset: -SLICE_LIMIT,
                    limit: SLICE_LIMIT,
                    max_id: 0,
                    min_id: 0,
                    hash: 0,
                },
            )
            .await?
        } else {
            self.split(
                split,
                tl::functions::messages::GetHistory {
                    peer: input,
                    offset_id,
                    offset_date: 0,
                    add_offset: -SLICE_LIMIT,
                    limit: SLICE_LIMIT,
                    max_id: 0,
                    min_id: 0,
                    hash: 0,
                },
            )
            .await?
        };
        let (messages, chats, users) = match res {
            E::messages::Messages::Messages(m) => (m.messages, m.chats, m.users),
            E::messages::Messages::Slice(m) => (m.messages, m.chats, m.users),
            E::messages::Messages::ChannelMessages(m) => (m.messages, m.chats, m.users),
            E::messages::Messages::NotModified(_) => return Ok(Vec::new()),
        };
        self.peers.absorb_users(&users);
        self.peers.absorb_chats(&chats);
        let mut messages = messages;
        messages.reverse(); // the server sends newest first
        Ok(messages)
    }

    /// Downloads one file in chunks through the takeout session.
    pub async fn download(&self, file: &FileRef, dest: &Path) -> Res<Download> {
        let (dc, location) = match &file.source {
            Source::Photo(p) => (
                p.dc_id,
                E::InputFileLocation::InputPhotoFileLocation(tl::types::InputPhotoFileLocation {
                    id: p.id,
                    access_hash: p.access_hash,
                    file_reference: p.file_reference.clone(),
                    thumb_size: largest_thumb_type(&p.sizes),
                }),
            ),
            Source::Document(d) => (
                d.dc_id,
                E::InputFileLocation::InputDocumentFileLocation(
                    tl::types::InputDocumentFileLocation {
                        id: d.id,
                        access_hash: d.access_hash,
                        file_reference: d.file_reference.clone(),
                        thumb_size: String::new(),
                    },
                ),
            ),
            Source::Bytes(b) => {
                write_all(dest, b).await;
                return Ok(Download::Saved(b.len() as u64));
            }
            Source::None => return Ok(Download::Unavailable),
        };

        if let Some(dir) = dest.parent() {
            let _ = tokio::fs::create_dir_all(dir).await;
        }
        let part = dest.with_extension(format!(
            "{}.part",
            dest.extension().and_then(|e| e.to_str()).unwrap_or("")
        ));
        let mut out = match tokio::fs::File::create(&part).await {
            Ok(f) => f,
            Err(_) => return Ok(Download::Unavailable),
        };

        let mut offset: i64 = 0;
        loop {
            let req = WithTakeout {
                takeout_id: self.takeout_id,
                query: tl::functions::upload::GetFile {
                    precise: false,
                    cdn_supported: false,
                    location: location.clone(),
                    offset,
                    limit: FILE_CHUNK,
                },
            };
            let res = match self.client.invoke_on_dc(dc, &req).await {
                Ok(r) => r,
                Err(e) => {
                    drop(out);
                    let _ = tokio::fs::remove_file(&part).await;
                    return match rpc_name(&e) {
                        Some("LOCATION_INVALID")
                        | Some("VERSION_INVALID")
                        | Some("LOCATION_NOT_AVAILABLE")
                        | Some("TAKEOUT_FILE_EMPTY")
                        | Some("FILE_REFERENCE_EXPIRED")
                        | Some("FILE_REFERENCE_INVALID")
                        | Some("FILE_REFERENCE_EMPTY") => Ok(Download::Unavailable),
                        _ => Err(e),
                    };
                }
            };
            let bytes = match res {
                E::upload::File::File(f) => f.bytes,
                E::upload::File::CdnRedirect(_) => {
                    drop(out);
                    let _ = tokio::fs::remove_file(&part).await;
                    return Ok(Download::Unavailable);
                }
            };
            let n = bytes.len();
            if n > 0 && out.write_all(&bytes).await.is_err() {
                drop(out);
                let _ = tokio::fs::remove_file(&part).await;
                return Ok(Download::Unavailable);
            }
            offset += n as i64;
            if (n as i32) < FILE_CHUNK {
                break;
            }
        }
        let _ = out.flush().await;
        drop(out);
        if tokio::fs::rename(&part, dest).await.is_err() {
            return Ok(Download::Unavailable);
        }
        Ok(Download::Saved(offset as u64))
    }
}

async fn write_all(dest: &Path, bytes: &[u8]) {
    if let Some(dir) = dest.parent() {
        let _ = tokio::fs::create_dir_all(dir).await;
    }
    let _ = tokio::fs::write(dest, bytes).await;
}

/// Type letter of the largest photo size, which is what the file location needs.
pub fn largest_thumb_type(sizes: &[E::PhotoSize]) -> String {
    let mut best = (0i64, String::new());
    for s in sizes {
        let (t, w, h) = match s {
            E::PhotoSize::PhotoSize(x) => (x.r#type.clone(), x.w, x.h),
            E::PhotoSize::Progressive(x) => (x.r#type.clone(), x.w, x.h),
            _ => continue,
        };
        let area = w as i64 * h as i64;
        if area >= best.0 {
            best = (area, t);
        }
    }
    best.1
}
