//! Export options and the rules that decide which files get downloaded.

use std::collections::HashSet;
use std::path::PathBuf;

use crate::model::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    Html,
    Json,
    Both,
}

impl Format {
    pub fn html(&self) -> bool {
        matches!(self, Format::Html | Format::Both)
    }
    pub fn json(&self) -> bool {
        matches!(self, Format::Json | Format::Both)
    }
}

#[derive(Clone, Debug)]
pub struct Settings {
    pub path: PathBuf,
    pub format: Format,

    pub personal_info: bool,
    pub contacts: bool,
    pub userpics: bool,

    pub personal_chats: bool,
    pub bot_chats: bool,
    pub private_groups: bool,
    pub public_groups: bool,
    pub private_channels: bool,
    pub public_channels: bool,
    pub saved_messages: bool,
    pub left_channels: bool,
    /// Full history of groups and channels. Off by default, which exports only
    /// your own messages there, as Telegram Desktop does.
    pub full_groups: bool,
    pub full_channels: bool,

    pub media: HashSet<MediaClassKey>,
    pub size_limit: i64,

    // Date range as unix seconds, 0 means no limit.
    pub from: i32,
    pub till: i32,

    /// Export just this chat (`@username`, numeric id or `me`).
    pub only_peer: Option<String>,
}

/// Like `MediaClass`, but without `Always`, so it can go into a set.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum MediaClassKey {
    Photo,
    Video,
    VoiceMessage,
    VideoMessage,
    Sticker,
    Gif,
    File,
}

pub fn class_key(c: MediaClass) -> Option<MediaClassKey> {
    Some(match c {
        MediaClass::Photo => MediaClassKey::Photo,
        MediaClass::Video => MediaClassKey::Video,
        MediaClass::VoiceMessage => MediaClassKey::VoiceMessage,
        MediaClass::VideoMessage => MediaClassKey::VideoMessage,
        MediaClass::Sticker => MediaClassKey::Sticker,
        MediaClass::Gif => MediaClassKey::Gif,
        MediaClass::File => MediaClassKey::File,
        MediaClass::Always => return None,
    })
}

impl Settings {
    pub fn wants_dialog(&self, t: DialogType) -> bool {
        match t {
            DialogType::Saved => self.saved_messages || self.personal_chats,
            DialogType::Personal => self.personal_chats,
            DialogType::Bot => self.bot_chats,
            DialogType::PrivateGroup | DialogType::PrivateSupergroup => self.private_groups,
            DialogType::PublicSupergroup => self.public_groups,
            DialogType::PrivateChannel => self.private_channels,
            DialogType::PublicChannel => self.public_channels,
        }
    }

    pub fn full_for(&self, t: DialogType) -> bool {
        match t {
            DialogType::Personal | DialogType::Bot | DialogType::Saved => true,
            DialogType::PrivateGroup
            | DialogType::PrivateSupergroup
            | DialogType::PublicSupergroup => self.full_groups,
            DialogType::PrivateChannel | DialogType::PublicChannel => self.full_channels,
        }
    }

    pub fn any_group_chats(&self) -> bool {
        self.personal_chats
            || self.bot_chats
            || self.private_groups
            || self.public_groups
            || self.saved_messages
    }

    pub fn any_channels(&self) -> bool {
        self.private_channels || self.public_channels || self.private_groups || self.public_groups
    }

    pub fn wants_files(&self) -> bool {
        !self.media.is_empty() && self.size_limit > 0
    }

    /// Decides whether a file should be downloaded. If not, the reason is
    /// stored on the file so the writers can explain why it is missing.
    pub fn apply_policy(&self, file: &mut FileRef) -> bool {
        if matches!(file.source, Source::None) {
            file.skip = Some(SkipReason::Unavailable);
            return false;
        }
        if let Some(key) = class_key(file.class) {
            if !self.media.contains(&key) {
                file.skip = Some(SkipReason::FileType);
                return false;
            }
            if file.size > self.size_limit {
                file.skip = Some(SkipReason::FileSize);
                return false;
            }
        }
        true
    }
}
