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
    pub userpics: bool,
    pub stories: bool,
    pub profile_music: bool,
    pub contacts: bool,
    pub sessions: bool,
    pub other_data: bool,

    pub personal_chats: bool,
    pub bot_chats: bool,
    pub private_groups: bool,
    pub public_groups: bool,
    pub private_channels: bool,
    pub public_channels: bool,

    /// Full history of private groups and channels. Everything else that is
    /// not a personal chat only exports your own messages.
    pub full_private_groups: bool,
    pub full_private_channels: bool,

    /// Keep files that are already complete in the output folder.
    pub resume: bool,

    pub media: HashSet<MediaClassKey>,
    pub size_limit: i64,

    // Date range as unix seconds, 0 means no limit.
    pub from: i32,
    pub till: i32,

    /// Export just this chat (`@username`, numeric id or `me`).
    pub only_peer: Option<String>,
    /// With `only_peer`: export only this forum topic (id of its first message).
    pub topic: Option<i32>,
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
            DialogType::Saved
            | DialogType::Replies
            | DialogType::VerifyCodes
            | DialogType::Personal => self.personal_chats,
            DialogType::Bot => self.bot_chats,
            DialogType::PrivateGroup | DialogType::PrivateSupergroup => self.private_groups,
            DialogType::PublicSupergroup => self.public_groups,
            DialogType::PrivateChannel => self.private_channels,
            DialogType::PublicChannel => self.public_channels,
        }
    }

    pub fn full_for(&self, t: DialogType) -> bool {
        match t {
            DialogType::Personal
            | DialogType::Saved
            | DialogType::Replies
            | DialogType::VerifyCodes
            | DialogType::Bot => true,
            DialogType::PrivateGroup | DialogType::PrivateSupergroup => self.full_private_groups,
            DialogType::PrivateChannel => self.full_private_channels,
            DialogType::PublicSupergroup | DialogType::PublicChannel => false,
        }
    }

    pub fn any_chats(&self) -> bool {
        self.personal_chats
            || self.bot_chats
            || self.private_groups
            || self.public_groups
            || self.private_channels
            || self.public_channels
    }

    /// Chats that are not channels. When none are wanted, only the newest
    /// takeout split has to be read.
    pub fn non_channel_chats(&self) -> bool {
        self.personal_chats || self.bot_chats || self.private_groups
    }

    pub fn groups_or_channels(&self) -> bool {
        self.private_groups || self.public_groups || self.private_channels || self.public_channels
    }

    /// Whether message files (not userpics or stories) are downloaded at all.
    pub fn wants_media(&self) -> bool {
        !self.media.is_empty() && self.size_limit > 0
    }

    /// Whether takeout has to be opened with permission to download files.
    pub fn needs_files(&self) -> bool {
        self.wants_media() || self.userpics || self.stories || self.profile_music
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
