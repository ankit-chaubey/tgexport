/*
 * Copyright (c) 2026 Ankit Chaubey <ankitchaubey.dev@gmail.com>
 * https://github.com/ankit-chaubey
 *
 * Project: tgexport
 * Repository: https://github.com/ankit-chaubey/tgexport
 *
 * Licensed under either the Apache License, Version 2.0
 * (LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0)
 * or the MIT License (LICENSE-MIT or https://opensource.org/licenses/MIT),
 * at your option.
 */

//! Data structures shared by the parser and both writers.

use std::collections::HashMap;

use ferogram::tl;

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum PeerId {
    User(i64),
    Chat(i64),
    Channel(i64),
}

impl PeerId {
    pub fn bare(&self) -> i64 {
        match *self {
            PeerId::User(i) | PeerId::Chat(i) | PeerId::Channel(i) => i,
        }
    }

    /// Tag used for `from_id` in the JSON output, e.g. `user123`.
    pub fn tag(&self) -> String {
        match *self {
            PeerId::User(i) => format!("user{i}"),
            PeerId::Chat(i) => format!("chat{i}"),
            PeerId::Channel(i) => format!("channel{i}"),
        }
    }

    pub fn color_index(&self) -> u8 {
        color_index_for(self.bare())
    }
}

/// Color of a userpic placeholder, chosen from an id.
pub fn color_index_for(bare: i64) -> u8 {
    const MAP: [u8; 7] = [0, 7, 4, 1, 6, 3, 5];
    MAP[(bare.unsigned_abs() % 7) as usize]
}

pub fn peer_from_tl(p: &tl::enums::Peer) -> PeerId {
    match p {
        tl::enums::Peer::User(u) => PeerId::User(u.user_id),
        tl::enums::Peer::Chat(c) => PeerId::Chat(c.chat_id),
        tl::enums::Peer::Channel(c) => PeerId::Channel(c.channel_id),
    }
}

#[derive(Clone, Debug, Default)]
pub struct PeerInfo {
    pub first_name: String,
    pub last_name: String,
    /// Only set for groups and channels.
    pub title: String,
    pub username: String,
    pub phone: String,
    pub is_bot: bool,
    pub is_self: bool,
    pub is_deleted: bool,
    pub is_chat: bool,
    pub is_broadcast: bool,
    pub is_megagroup: bool,
    pub access_hash: i64,
    /// Set when this group was upgraded to a supergroup (the new channel id).
    pub migrated_to: Option<i64>,
    pub left: bool,
}

impl PeerInfo {
    pub fn name(&self) -> String {
        if self.is_chat {
            return self.title.clone();
        }
        let full = format!("{} {}", self.first_name, self.last_name);
        full.trim().to_string()
    }
}

#[derive(Default)]
pub struct Peers {
    pub map: HashMap<PeerId, PeerInfo>,
}

impl Peers {
    pub fn get(&self, id: PeerId) -> Option<&PeerInfo> {
        self.map.get(&id)
    }

    pub fn name(&self, id: PeerId) -> String {
        self.map.get(&id).map(|p| p.name()).unwrap_or_default()
    }

    pub fn absorb_users(&mut self, users: &[tl::enums::User]) {
        for u in users {
            if let tl::enums::User::User(u) = u {
                let e = self.map.entry(PeerId::User(u.id)).or_default();
                e.first_name = u.first_name.clone().unwrap_or_default();
                e.last_name = u.last_name.clone().unwrap_or_default();
                e.username = u.username.clone().unwrap_or_default();
                if e.username.is_empty() {
                    if let Some(list) = &u.usernames {
                        for n in list {
                            let tl::enums::Username::Username(n) = n;
                            if n.active && n.editable {
                                e.username = n.username.clone();
                            }
                        }
                    }
                }
                if let Some(p) = &u.phone {
                    e.phone = p.clone();
                }
                e.is_bot = u.bot;
                e.is_self = u.is_self;
                e.is_deleted = u.deleted;
                e.access_hash = u.access_hash.unwrap_or(e.access_hash);
            } else if let tl::enums::User::Empty(u) = u {
                self.map.entry(PeerId::User(u.id)).or_default();
            }
        }
    }

    pub fn absorb_chats(&mut self, chats: &[tl::enums::Chat]) {
        for c in chats {
            match c {
                tl::enums::Chat::Chat(c) => {
                    let e = self.map.entry(PeerId::Chat(c.id)).or_default();
                    e.is_chat = true;
                    e.title = c.title.clone();
                    e.left = c.left;
                    e.migrated_to = match &c.migrated_to {
                        Some(tl::enums::InputChannel::InputChannel(ic)) => Some(ic.channel_id),
                        _ => None,
                    };
                }
                tl::enums::Chat::Channel(c) => {
                    let e = self.map.entry(PeerId::Channel(c.id)).or_default();
                    e.is_chat = true;
                    e.title = c.title.clone();
                    e.username = c.username.clone().unwrap_or_default();
                    if e.username.is_empty() {
                        if let Some(list) = &c.usernames {
                            for n in list {
                                let tl::enums::Username::Username(n) = n;
                                if n.active && n.editable {
                                    e.username = n.username.clone();
                                }
                            }
                        }
                    }
                    e.is_broadcast = c.broadcast;
                    e.is_megagroup = c.megagroup;
                    e.left = c.left;
                    e.access_hash = c.access_hash.unwrap_or(e.access_hash);
                }
                tl::enums::Chat::Forbidden(c) => {
                    let e = self.map.entry(PeerId::Chat(c.id)).or_default();
                    e.is_chat = true;
                    e.title = c.title.clone();
                }
                tl::enums::Chat::ChannelForbidden(c) => {
                    let e = self.map.entry(PeerId::Channel(c.id)).or_default();
                    e.is_chat = true;
                    e.title = c.title.clone();
                    e.access_hash = c.access_hash;
                    e.is_broadcast = c.broadcast;
                    e.is_megagroup = c.megagroup;
                }
                _ => {}
            }
        }
    }

    pub fn input_peer(&self, id: PeerId) -> tl::enums::InputPeer {
        match id {
            PeerId::User(u) => tl::enums::InputPeer::User(tl::types::InputPeerUser {
                user_id: u,
                access_hash: self.map.get(&id).map(|p| p.access_hash).unwrap_or(0),
            }),
            PeerId::Chat(c) => tl::enums::InputPeer::Chat(tl::types::InputPeerChat { chat_id: c }),
            PeerId::Channel(c) => tl::enums::InputPeer::Channel(tl::types::InputPeerChannel {
                channel_id: c,
                access_hash: self.map.get(&id).map(|p| p.access_hash).unwrap_or(0),
            }),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TextKind {
    Text,
    Unknown,
    Mention,
    Hashtag,
    BotCommand,
    Url,
    Email,
    Bold,
    Italic,
    Code,
    Pre,
    TextUrl,
    MentionName,
    Phone,
    Cashtag,
    Underline,
    Strike,
    Blockquote,
    BankCard,
    TonAddress,
    Spoiler,
    CustomEmoji,
}

#[derive(Clone, Debug)]
pub struct TextPart {
    pub kind: TextKind,
    pub text: String,
    pub additional: String,
}

pub type Text = Vec<TextPart>;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SkipReason {
    Unavailable,
    FileSize,
    FileType,
}

/// Where the content of a file comes from.
#[derive(Clone, Debug)]
pub enum Source {
    Photo(tl::types::Photo),
    Document(tl::types::Document),
    /// Content we already have, such as a contact's vCard.
    Bytes(Vec<u8>),
    /// A thumbnail of a document: the document and the size letter to request.
    DocumentThumb(tl::types::Document, String),
    /// The "other data" file that takeout offers for download.
    Takeout,
    None,
}

/// Kind of file, used by the `--media` filter.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MediaClass {
    Photo,
    Video,
    VoiceMessage,
    VideoMessage,
    Sticker,
    Gif,
    File,
    /// Never filtered out (vCards, profile pictures).
    Always,
}

#[derive(Clone, Debug)]
pub struct FileRef {
    pub source: Source,
    pub class: MediaClass,
    pub size: i64,
    /// Final path relative to the export folder. Empty if the file was skipped.
    pub suggested: String,
    pub relative_path: String,
    pub skip: Option<SkipReason>,
}

impl FileRef {
    pub fn none() -> Self {
        FileRef {
            source: Source::None,
            class: MediaClass::Always,
            size: 0,
            suggested: String::new(),
            relative_path: String::new(),
            skip: None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct DocFlags {
    pub sticker: bool,
    pub animated: bool,
    pub video_file: bool,
    pub video_message: bool,
    pub voice: bool,
    pub audio: bool,
}

#[derive(Clone, Debug)]
pub struct Poll {
    pub question: Text,
    pub closed: bool,
    pub total_votes: i32,
    pub answers: Vec<(Text, i32, bool)>,
}

#[derive(Clone, Debug)]
pub struct Thumb {
    pub file: FileRef,
}

#[derive(Clone, Debug)]
pub enum Media {
    None,
    Photo {
        file: FileRef,
        width: i32,
        height: i32,
        spoiler: bool,
        ttl: i32,
    },
    Document {
        file: FileRef,
        name: String,
        mime: String,
        duration: i32,
        width: i32,
        height: i32,
        flags: DocFlags,
        sticker_emoji: String,
        performer: String,
        title: String,
        spoiler: bool,
        ttl: i32,
        thumb: Option<Thumb>,
    },
    Contact {
        first_name: String,
        last_name: String,
        phone: String,
        vcard: Option<FileRef>,
    },
    Geo {
        point: Option<(f64, f64)>,
        live_period: i32,
    },
    Venue {
        point: Option<(f64, f64)>,
        title: String,
        address: String,
    },
    Game {
        title: String,
        description: String,
        bot_id: i64,
        short_name: String,
    },
    Invoice {
        title: String,
        description: String,
        amount: i64,
        currency: String,
        receipt_msg_id: i32,
    },
    Poll(Poll),
    Todo {
        title: Text,
        others_can_append: bool,
        others_can_complete: bool,
        items: Vec<crate::actions::TodoItem>,
    },
    GiveawayStart {
        countries: Vec<String>,
        channels: Vec<i64>,
        additional_prize: String,
        until_date: i32,
        credits: i64,
        quantity: i32,
        months: i32,
        all: bool,
    },
    GiveawayResults {
        channel: i64,
        winners: Vec<i64>,
        additional_prize: String,
        until_date: i32,
        launch_id: i32,
        additional_peers_count: i32,
        winners_count: i32,
        unclaimed_count: i32,
        months: i32,
        credits: i64,
        refunded: bool,
        all: bool,
    },
    Paid {
        stars: i64,
    },
    Unsupported,
}

pub use crate::actions::Action;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ButtonKind {
    Default,
    Url,
    Callback,
    CallbackWithPassword,
    RequestPhone,
    RequestLocation,
    RequestPoll,
    RequestPeer,
    SwitchInline,
    SwitchInlineSame,
    Game,
    Buy,
    Auth,
    UserProfile,
    WebView,
    SimpleWebView,
    CopyText,
    Disabled,
}

impl ButtonKind {
    pub fn name(&self) -> &'static str {
        match self {
            ButtonKind::Default => "default",
            ButtonKind::Url => "url",
            ButtonKind::Callback => "callback",
            ButtonKind::CallbackWithPassword => "callback_with_password",
            ButtonKind::RequestPhone => "request_phone",
            ButtonKind::RequestLocation => "request_location",
            ButtonKind::RequestPoll => "request_poll",
            ButtonKind::RequestPeer => "request_peer",
            ButtonKind::SwitchInline => "switch_inline",
            ButtonKind::SwitchInlineSame => "switch_inline_same",
            ButtonKind::Game => "game",
            ButtonKind::Buy => "buy",
            ButtonKind::Auth => "auth",
            ButtonKind::UserProfile => "user_profile",
            ButtonKind::WebView => "web_view",
            ButtonKind::SimpleWebView => "simple_web_view",
            ButtonKind::CopyText => "copy_text",
            ButtonKind::Disabled => "disabled",
        }
    }
}

/// An inline button under a message. What `data` holds depends on the kind:
/// a link, callback bytes, a query, a user id or the text to copy.
#[derive(Clone, Debug)]
pub struct Button {
    pub kind: ButtonKind,
    pub text: String,
    pub data: Vec<u8>,
    pub forward_text: String,
    pub button_id: i32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ReactionKind {
    Empty,
    Emoji,
    Custom,
    Paid,
}

impl ReactionKind {
    pub fn name(&self) -> &'static str {
        match self {
            ReactionKind::Empty => "empty",
            ReactionKind::Emoji => "emoji",
            ReactionKind::Custom => "custom_emoji",
            ReactionKind::Paid => "paid",
        }
    }
}

/// A reaction. For custom emoji `custom` first holds the document id and,
/// once the emoji file is handled, its path (empty if it was not saved).
#[derive(Clone, Debug)]
pub struct Reaction {
    pub kind: ReactionKind,
    pub emoji: String,
    pub custom: Option<String>,
    pub count: i32,
    /// Who reacted recently (only known in some chats) and when.
    pub recent: Vec<(PeerId, i32)>,
}

#[derive(Clone, Debug)]
pub struct Message {
    pub id: i32,
    pub out: bool,
    pub date: i32,
    pub edited: i32,
    pub from: Option<PeerId>,
    pub signature: String,
    pub fwd_from: Option<PeerId>,
    pub fwd_name: String,
    pub fwd_date: i32,
    pub forwarded: bool,
    pub show_forwarded_as_original: bool,
    pub saved_from: Option<PeerId>,
    pub reply_to: i32,
    pub reply_to_peer: Option<PeerId>,
    pub via_bot: Option<i64>,
    pub text: Text,
    pub media: Media,
    pub action: Option<Action>,
    pub buttons: Vec<Vec<Button>>,
    pub reactions: Vec<Reaction>,
    pub rich: Option<crate::rich::RichMessage>,
    pub unsupported: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DialogType {
    Saved,
    Replies,
    VerifyCodes,
    Personal,
    Bot,
    PrivateGroup,
    PrivateSupergroup,
    PublicSupergroup,
    PrivateChannel,
    PublicChannel,
}

impl DialogType {
    pub fn json_name(&self) -> &'static str {
        match self {
            DialogType::Saved => "saved_messages",
            DialogType::Replies => "replies",
            DialogType::VerifyCodes => "verification_codes",
            DialogType::Personal => "personal_chat",
            DialogType::Bot => "bot_chat",
            DialogType::PrivateGroup => "private_group",
            DialogType::PrivateSupergroup => "private_supergroup",
            DialogType::PublicSupergroup => "public_supergroup",
            DialogType::PrivateChannel => "private_channel",
            DialogType::PublicChannel => "public_channel",
        }
    }

    pub fn is_channel(&self) -> bool {
        matches!(self, DialogType::PrivateChannel | DialogType::PublicChannel)
    }
}

#[derive(Clone, Debug)]
pub struct DialogInfo {
    pub peer: PeerId,
    pub dtype: DialogType,
    /// First name for users, the title for groups and channels.
    pub name: String,
    /// Only set for users.
    pub last_name: String,
    pub is_left: bool,
    pub only_my_messages: bool,
    /// Basic group whose history continues in this supergroup.
    pub migrated_from: Option<PeerId>,
    pub relative_path: String,
    /// Takeout splits (message id ranges) that contain messages of this chat.
    pub splits: Vec<usize>,
}

#[derive(Clone, Debug, Default)]
pub struct PersonalInfo {
    pub user_id: i64,
    pub first_name: String,
    pub last_name: String,
    pub phone: String,
    pub username: String,
    pub bio: String,
    pub color_index: u8,
}

#[derive(Clone, Debug)]
pub struct Contact {
    pub first_name: String,
    pub last_name: String,
    pub phone: String,
    pub date: i32,
}

#[derive(Clone, Debug)]
pub struct Userpic {
    pub date: i32,
    pub file: FileRef,
}

/// Local time as `YYYY-MM-DDTHH:MM:SS`, the format result.json uses.
pub fn iso_date(ts: i32) -> String {
    use chrono::{Local, TimeZone};
    match Local.timestamp_opt(ts as i64, 0).single() {
        Some(d) => d.format("%Y-%m-%dT%H:%M:%S").to_string(),
        None => String::new(),
    }
}

#[derive(Clone, Debug)]
pub struct TopPeerEntry {
    pub id: PeerId,
    pub name: String,
    /// Users only, used for the userpic initials.
    pub first_name: String,
    pub last_name: String,
    pub is_self: bool,
    pub kind: &'static str,
    pub rating: f64,
}

#[derive(Clone, Debug, Default)]
pub struct ContactsList {
    pub contacts: Vec<Contact>,
    pub correspondents: Vec<TopPeerEntry>,
    pub inline_bots: Vec<TopPeerEntry>,
    pub phone_calls: Vec<TopPeerEntry>,
}

#[derive(Clone, Debug)]
pub struct Session {
    pub last_active: i32,
    pub ip: String,
    pub country: String,
    pub region: String,
    pub app_name: String,
    pub app_version: String,
    pub device_model: String,
    pub platform: String,
    pub system_version: String,
    pub created: i32,
    pub application_id: i32,
}

#[derive(Clone, Debug)]
pub struct WebSession {
    pub last_active: i32,
    pub ip: String,
    pub region: String,
    pub domain: String,
    pub browser: String,
    pub platform: String,
    pub created: i32,
    pub bot_username: String,
}

#[derive(Clone, Debug, Default)]
pub struct SessionsList {
    pub list: Vec<Session>,
    pub web_list: Vec<WebSession>,
}

#[derive(Clone, Debug)]
pub struct Story {
    pub id: i32,
    pub date: i32,
    pub expires: i32,
    pub pinned: bool,
    pub file: FileRef,
    /// Preview of a video story, saved next to it.
    pub thumb: Option<Thumb>,
    pub caption: Text,
}

/// Where a file was found, so a fresh file reference can be requested when the
/// one we hold has expired.
#[derive(Clone, Debug)]
pub enum Origin {
    None,
    Message { peer: PeerId, id: i32, split: usize },
    Story { id: i32 },
    CustomEmoji { id: i64 },
    RichMessage { peer: PeerId, id: i32 },
}
