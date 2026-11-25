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

//! HTML for rich messages. The renderer follows Telegram Desktop's one, so the
//! markup, class names and the rules for which links are allowed are the same.

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};
use unicode_general_category::{GeneralCategory as Gc, get_general_category};

use crate::model::*;
use crate::rich::*;

const TARGET_MAX_BYTES: usize = 4096;
const ANCHOR_DIRECT_BYTES: usize = 72;
const ANCHOR_PREFIX_BYTES: usize = 48;
const TABLE_SPAN_MAX: i32 = 1000;
const HASHTAG_MIN_LENGTH: usize = 2;
const HASHTAG_MAX_LENGTH: usize = 64;
const COMMAND_MAX_LENGTH: usize = 64;
const CASHTAG_MAX_LENGTH: usize = 8;
const USERNAME_MIN_LENGTH: usize = 5;
const USERNAME_MAX_LENGTH: usize = 32;

/// Marker that stands for a custom emoji that could not be loaded.
pub const UNAVAILABLE_EMOJI: &str = "(unavailable)";

/// Link prefix for usernames (`internalLinksDomain`).
const INTERNAL_LINKS_DOMAIN: &str = "https://t.me/";

// Escaping and text helpers.

/// Same escaping Telegram Desktop uses for all text and attribute values.
pub fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 8);
    let mut chars = value.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\n' => out.push_str("<br>"),
            '"' => out.push_str("&quot;"),
            '&' => out.push_str("&amp;"),
            '\'' => out.push_str("&apos;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\u{2028}' | '\u{2029}' => out.push_str("<br>"),
            c if (c as u32) < 32 => out.push_str(&format!("&#x{:02X};", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// `QUrl::toPercentEncoding`: everything except unreserved characters.
fn percent_encode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

pub fn base64_url(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(data.len() * 4 / 3 + 3);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[(n >> 6) as usize & 63] as char);
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[n as usize & 63] as char);
        }
    }
    out
}

fn hex_value(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

fn has_encoded_control(value: &str) -> bool {
    let b = value.as_bytes();
    let mut i = 0;
    while i + 2 < b.len() {
        if b[i] == b'%' {
            if let (Some(high), Some(low)) = (hex_value(b[i + 1]), hex_value(b[i + 2])) {
                let decoded = (high << 4) | low;
                if decoded < 32 || decoded == 127 {
                    return true;
                }
            }
        }
        i += 1;
    }
    false
}

fn is_strict_target(value: &str) -> bool {
    if value.is_empty() || value.len() > TARGET_MAX_BYTES {
        return false;
    }
    if value.bytes().any(|b| b < 32 || b == 127) {
        return false;
    }
    if value.trim() != value {
        return false;
    }
    !value.chars().any(|c| {
        matches!(
            get_general_category(c),
            Gc::Control | Gc::LineSeparator | Gc::ParagraphSeparator
        )
    })
}

const ALLOWED_SCHEMES: [&str; 5] = ["http", "https", "mailto", "tel", "tg"];

fn parse_strict(value: &str) -> Option<url::Url> {
    url::Url::parse(value).ok()
}

/// A link that is safe to put into the page, or `None`.
pub fn safe_message_href(value: &str) -> Option<String> {
    if !is_strict_target(value) || value.starts_with("//") || has_encoded_control(value) {
        return None;
    }
    let parsed = match parse_strict(value) {
        Some(u) => u,
        // No scheme: Telegram Desktop tries again with https:// in front.
        None => parse_strict(&format!("https://{value}"))?,
    };
    let scheme = parsed.scheme().to_lowercase();
    let host_scheme = scheme == "http" || scheme == "https";
    if !ALLOWED_SCHEMES.contains(&scheme.as_str())
        || (host_scheme && parsed.host_str().map(|h| h.is_empty()).unwrap_or(true))
    {
        return None;
    }
    let result = parsed.to_string();
    (is_strict_target(&result) && !has_encoded_control(&result)).then_some(result)
}

pub fn safe_http_href(value: &str) -> Option<String> {
    if !is_strict_target(value) || value.starts_with("//") || has_encoded_control(value) {
        return None;
    }
    let parsed = parse_strict(value)?;
    let scheme = parsed.scheme().to_lowercase();
    if parsed.host_str().map(|h| h.is_empty()).unwrap_or(true)
        || (scheme != "http" && scheme != "https")
    {
        return None;
    }
    let result = parsed.to_string();
    (is_strict_target(&result) && !has_encoded_control(&result)).then_some(result)
}

fn safe_email_href(address: &str) -> Option<String> {
    if !is_strict_target(address) {
        return None;
    }
    let sep = address.find('@')?;
    if sep == 0 || Some(sep) != address.rfind('@') || sep + 1 == address.len() {
        return None;
    }
    if address.chars().any(|c| c.is_whitespace()) {
        return None;
    }
    if address.chars().any(|c| "/\\:%?#&=".contains(c)) {
        return None;
    }
    let result = format!("mailto:{address}");
    is_strict_target(&result).then_some(result)
}

fn safe_phone_href(phone: &str) -> Option<String> {
    if !is_strict_target(phone) {
        return None;
    }
    let mut has_digit = false;
    for c in phone.chars() {
        if c.is_ascii_digit() {
            has_digit = true;
        } else if !matches!(c, '+' | '-' | ' ' | '.' | '(' | ')') {
            return None;
        }
    }
    let result = format!("tel:{phone}");
    (has_digit && is_strict_target(&result)).then_some(result)
}

fn safe_mention_href(mention: &str) -> Option<String> {
    if !is_strict_target(mention) || mention.len() < 2 || !mention.starts_with('@') {
        return None;
    }
    let username = &mention[1..];
    if !username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return None;
    }
    safe_http_href(&format!("{INTERNAL_LINKS_DOMAIN}{username}"))
}

fn is_ascii_word(value: &str, min: usize, max: usize) -> bool {
    value.len() >= min
        && value.len() <= max
        && value.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn is_unicode_hashtag_word(value: &str) -> bool {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() < HASHTAG_MIN_LENGTH || chars.len() > HASHTAG_MAX_LENGTH {
        return false;
    }
    let mut all_digits = true;
    for &c in &chars {
        let cat = get_general_category(c);
        let is_mark = matches!(
            cat,
            Gc::NonspacingMark | Gc::SpacingMark | Gc::EnclosingMark
        );
        let letter_or_number = matches!(
            cat,
            Gc::UppercaseLetter
                | Gc::LowercaseLetter
                | Gc::TitlecaseLetter
                | Gc::ModifierLetter
                | Gc::OtherLetter
                | Gc::DecimalNumber
                | Gc::LetterNumber
                | Gc::OtherNumber
        );
        if !letter_or_number && cat != Gc::ConnectorPunctuation && !is_mark {
            return false;
        }
        all_digits = all_digits && cat == Gc::DecimalNumber;
    }
    !all_digits
}

/// Splits `name@bot` into its two parts, rejecting more than one `@`.
fn split_bot(value: &str) -> Option<(&str, Option<&str>)> {
    match value.find('@') {
        None => Some((value, None)),
        Some(i) if Some(i) == value.rfind('@') => Some((&value[..i], Some(&value[i + 1..]))),
        Some(_) => None,
    }
}

fn is_hashtag_action(value: &str) -> bool {
    let Some((tag, bot)) = split_bot(value) else {
        return false;
    };
    is_unicode_hashtag_word(tag)
        && bot
            .map(|b| is_ascii_word(b, 1, USERNAME_MAX_LENGTH))
            .unwrap_or(true)
}

fn is_bot_command_action(value: &str) -> bool {
    let Some((command, bot)) = split_bot(value) else {
        return false;
    };
    is_ascii_word(command, 1, COMMAND_MAX_LENGTH)
        && bot
            .map(|b| is_ascii_word(b, USERNAME_MIN_LENGTH, USERNAME_MAX_LENGTH))
            .unwrap_or(true)
}

fn is_cashtag_action(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= CASHTAG_MAX_LENGTH
        && value.chars().all(|c| c.is_ascii_uppercase())
}

fn safe_action_data(target: &str, prefix: char) -> Option<String> {
    if !is_strict_target(target) || !target.starts_with(prefix) || target.len() < 2 {
        return None;
    }
    let result = &target[prefix.len_utf8()..];
    let valid = match prefix {
        '#' => is_hashtag_action(result),
        '/' => is_bot_command_action(result),
        '$' => is_cashtag_action(result),
        _ => false,
    };
    (valid && is_strict_target(result)).then(|| result.to_string())
}

fn is_ascii_decimal(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit())
}

fn safe_relative_emoji_href(path: &str, base: &str) -> Option<String> {
    if !is_strict_target(path)
        || path.starts_with('/')
        || path.contains(['\\', ':', '%', '?', '#'])
        || path == UNAVAILABLE_EMOJI
        || is_ascii_decimal(path)
    {
        return None;
    }
    if path
        .split('/')
        .any(|c| c.is_empty() || c == "." || c == "..")
    {
        return None;
    }
    let result = format!("{base}{path}");
    is_strict_target(&result).then_some(result)
}

// Plain text of a text, used for link targets.

fn append_plain_target(result: &mut String, value: &str) -> bool {
    if result.len() > TARGET_MAX_BYTES || value.len() > TARGET_MAX_BYTES - result.len() {
        return false;
    }
    result.push_str(value);
    true
}

fn append_plain_target_text(result: &mut String, t: &RichText) -> bool {
    use RichTextKind as K;
    match t.kind {
        K::Empty => true,
        K::Plain => append_plain_target(result, &t.text),
        K::Concat
        | K::Bold
        | K::Italic
        | K::Underline
        | K::Strike
        | K::Fixed
        | K::Url
        | K::Email
        | K::Phone
        | K::Subscript
        | K::Superscript
        | K::Marked
        | K::Anchor
        | K::Spoiler
        | K::Mention
        | K::Hashtag
        | K::BotCommand
        | K::Cashtag
        | K::AutoUrl
        | K::AutoEmail
        | K::AutoPhone
        | K::BankCard
        | K::TonAddress
        | K::MentionName
        | K::FormattedDate
        | K::Diff => t
            .children
            .iter()
            .all(|c| append_plain_target_text(result, c)),
        K::Button | K::Math | K::InlineImage => false,
        K::CustomEmoji => append_plain_target(result, &t.text),
    }
}

fn plain_target(t: &RichText) -> Option<String> {
    let mut result = String::new();
    append_plain_target_text(&mut result, t).then_some(result)
}

fn anchor_token(name: &str) -> String {
    if name.is_empty() {
        return "empty".into();
    }
    let bytes = name.as_bytes();
    if bytes.len() <= ANCHOR_DIRECT_BYTES {
        return base64_url(bytes);
    }
    let hash = Sha256::digest(bytes);
    let hex: String = hash.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}",
        base64_url(&bytes[..ANCHOR_PREFIX_BYTES]),
        &hex[..16]
    )
}

// Presentation of files and small labels.

/// A saved file, or the reason why it is missing, as the page shows it.
#[derive(Clone, Debug, Default)]
pub struct FileView {
    pub path: String,
    pub skip: Option<SkipReason>,
    pub size: i64,
}

pub fn file_view(file: Option<&FileRef>) -> FileView {
    let Some(f) = file else {
        return FileView {
            skip: Some(SkipReason::Unavailable),
            ..Default::default()
        };
    };
    if !f.relative_path.is_empty() {
        return FileView {
            path: f.relative_path.clone(),
            skip: None,
            size: f.size,
        };
    }
    let skip = match f.skip {
        Some(r @ (SkipReason::FileType | SkipReason::FileSize)) => r,
        _ => SkipReason::Unavailable,
    };
    FileView {
        path: String::new(),
        skip: Some(skip),
        size: f.size,
    }
}

pub fn no_file_description(reason: Option<SkipReason>) -> &'static str {
    match reason {
        Some(SkipReason::Unavailable) => "Unavailable, please try again later.",
        Some(SkipReason::FileSize) => {
            "Exceeds maximum size, change data exporting settings to download."
        }
        Some(SkipReason::FileType) => "Not included, change data exporting settings to download.",
        None => "",
    }
}

fn heading_tag(level: i32) -> &'static str {
    match level {
        1 => "h1",
        2 => "h2",
        3 => "h3",
        4 => "h4",
        5 => "h5",
        _ => "h6",
    }
}

fn bool_attribute(v: bool) -> &'static str {
    if v { "true" } else { "false" }
}

fn action_name(k: InlineActionKind) -> &'static str {
    use InlineActionKind as K;
    match k {
        K::Url => "url",
        K::Auth => "auth",
        K::WebView => "web_view",
        K::Callback => "callback",
        K::CallbackWithPassword => "callback_with_password",
        K::Game => "game",
        K::Buy => "buy",
        K::SwitchInline => "switch_inline",
        K::SwitchInlineSame => "switch_inline_same",
        K::UserProfile => "user_profile",
        K::CopyText => "copy_text",
        K::Disabled => "disabled",
    }
}

fn peer_type_name(t: InlinePeerType) -> &'static str {
    match t {
        InlinePeerType::SameBotPm => "same_bot_pm",
        InlinePeerType::Pm => "pm",
        InlinePeerType::Chat => "chat",
        InlinePeerType::Megagroup => "megagroup",
        InlinePeerType::Broadcast => "broadcast",
        InlinePeerType::BotPm => "bot_pm",
    }
}

fn style_name(s: ButtonStyle) -> &'static str {
    match s {
        ButtonStyle::Default => "default",
        ButtonStyle::Primary => "primary",
        ButtonStyle::Success => "success",
        ButtonStyle::Danger => "danger",
        ButtonStyle::Link => "link",
    }
}

/// Attributes are kept sorted by name, like the `std::map` Telegram Desktop
/// builds them in.
pub type Attrs = BTreeMap<String, String>;

pub fn attrs(pairs: &[(&str, &str)]) -> Attrs {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn button_attributes(
    action: &InlineAction,
    style: Option<ButtonStyle>,
    inline_button: bool,
) -> Attrs {
    use InlineActionKind as K;
    let style_text = style_name(style.unwrap_or(ButtonStyle::Default));
    let classes = format!(
        "rich_button {} rich_button_style_{} rich_button_action_{}",
        if inline_button {
            "rich_button_inline"
        } else {
            "rich_button_block"
        },
        style_text,
        action_name(action.kind)
    );
    let mut r = Attrs::new();
    r.insert("class".into(), classes);
    r.insert("data-button-type".into(), action_name(action.kind).into());
    r.insert("inline".into(), String::new());
    if style.is_some() {
        r.insert("data-button-style".into(), style_text.into());
    }
    match action.kind {
        K::Url | K::Auth | K::WebView => {
            r.insert("data-button-url".into(), percent_encode(&action.url));
            if action.kind == K::Auth {
                if let Some(t) = &action.forward_text {
                    r.insert("data-button-forward-text".into(), percent_encode(t));
                }
                r.insert("data-button-id".into(), action.button_id.to_string());
            }
        }
        K::Callback | K::CallbackWithPassword => {
            r.insert(
                "data-callback-data-base64".into(),
                base64_url(&action.callback_data),
            );
            r.insert(
                "data-button-requires-password".into(),
                bool_attribute(action.requires_password).into(),
            );
        }
        K::Game | K::Buy | K::Disabled => {}
        K::SwitchInline | K::SwitchInlineSame => {
            r.insert("data-button-query".into(), percent_encode(&action.query));
            r.insert(
                "data-button-same-peer".into(),
                bool_attribute(action.same_peer).into(),
            );
            if let Some(types) = &action.peer_types {
                let list: Vec<String> = types
                    .iter()
                    .map(|t| format!("\"{}\"", peer_type_name(*t)))
                    .collect();
                r.insert(
                    "data-button-peer-types".into(),
                    percent_encode(&format!("[{}]", list.join(","))),
                );
            }
        }
        K::UserProfile => {
            r.insert("data-button-user-id".into(), action.user_id.to_string());
        }
        K::CopyText => {
            r.insert("data-copy-text".into(), percent_encode(&action.copy_text));
        }
    }
    if action.kind == K::Disabled {
        r.insert("aria-disabled".into(), "true".into());
    }
    r
}

fn percent_value(value: i32, total: i32) -> String {
    format!("{:.3}%", value as f64 * 100.0 / total as f64)
}

fn aspect_ratio_style(w: i32, h: i32) -> String {
    format!("aspect-ratio: {w} / {h}")
}

fn text_has_output(t: &RichText) -> bool {
    use RichTextKind as K;
    match t.kind {
        K::Empty => false,
        K::Plain => !t.text.is_empty(),
        K::Concat => t.children.iter().any(text_has_output),
        _ => true,
    }
}

fn caption_has_output(c: &RichCaption) -> bool {
    text_has_output(&c.text) || text_has_output(&c.credit)
}

fn channel_status(c: &RichChannel) -> String {
    let label = match c.source {
        ChannelSource::ChatEmpty | ChannelSource::Chat => "Chat",
        ChannelSource::ChatForbidden => "Chat unavailable",
        ChannelSource::Channel => "Channel",
        ChannelSource::ChannelForbidden => "Channel unavailable",
        ChannelSource::Community => "Community",
        ChannelSource::CommunityForbidden => "Community unavailable",
    };
    match &c.username {
        Some(u) if !u.is_empty() => format!("@{u}, {label}"),
        _ => label.to_string(),
    }
}

fn channel_source_name(c: ChannelSource) -> &'static str {
    match c {
        ChannelSource::ChatEmpty => "chat_empty",
        ChannelSource::Chat => "chat",
        ChannelSource::ChatForbidden => "chat_forbidden",
        ChannelSource::Channel => "channel",
        ChannelSource::ChannelForbidden => "channel_forbidden",
        ChannelSource::Community => "community",
        ChannelSource::CommunityForbidden => "community_forbidden",
    }
}

fn map_source_name(c: MapSource) -> &'static str {
    match c {
        MapSource::GeoPointEmpty => "geo_point_empty",
        MapSource::GeoPoint => "geo_point",
        MapSource::InputGeoPointEmpty => "input_geo_point_empty",
        MapSource::InputGeoPoint => "input_geo_point",
    }
}

/// Latitude and longitude as text, when they are real coordinates.
fn map_coordinates(p: &MapPoint) -> Option<(String, String)> {
    match p.source {
        MapSource::GeoPointEmpty | MapSource::InputGeoPointEmpty => return None,
        MapSource::GeoPoint | MapSource::InputGeoPoint => {}
    }
    if !p.latitude.is_finite()
        || !p.longitude.is_finite()
        || !(-90.0..=90.0).contains(&p.latitude)
        || !(-180.0..=180.0).contains(&p.longitude)
    {
        return None;
    }
    Some((format!("{:.6}", p.latitude), format!("{:.6}", p.longitude)))
}

// The tag builder: block tags go on their own lines, inline tags do not.

/// Mirrors the tag stack of Telegram Desktop's HTML context. Attributes come
/// out sorted by name, and `inline` / `empty` are switches, not attributes.
#[derive(Default)]
pub struct Html {
    tags: Vec<(String, bool)>,
}

impl Html {
    /// A builder that starts inside `depth` open block tags, so the
    /// indentation matches the surrounding page.
    pub fn with_depth(depth: usize) -> Self {
        Html {
            tags: vec![(String::new(), true); depth],
        }
    }

    fn indent(&self) -> String {
        " ".repeat(self.tags.len())
    }

    pub fn push_tag(&mut self, tag: &str, attributes: Attrs) -> String {
        let mut block = true;
        let mut empty = false;
        let mut inner = String::new();
        for (name, value) in &attributes {
            if name == "inline" {
                block = false;
            } else if name == "empty" {
                empty = true;
            } else {
                inner.push(' ');
                inner.push_str(name);
                inner.push_str("=\"");
                inner.push_str(&escape(value));
                inner.push('"');
            }
        }
        let result = format!(
            "{}<{}{}{}>{}",
            if block {
                format!("\n{}", self.indent())
            } else {
                String::new()
            },
            tag,
            inner,
            if empty { "/" } else { "" },
            if block { "\n" } else { "" }
        );
        if !empty {
            self.tags.push((tag.to_string(), block));
        }
        result
    }

    pub fn pop_tag(&mut self) -> String {
        let (name, block) = self.tags.pop().expect("tag stack is empty");
        format!(
            "{}</{}>{}",
            if block {
                format!("\n{}", self.indent())
            } else {
                String::new()
            },
            name,
            if block { "\n" } else { "" }
        )
    }
}

// What the page writer provides for media.

/// A box with a title, description and status (`MediaData` in the page writer).
#[derive(Clone, Debug, Default)]
pub struct MediaData {
    pub classes: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub link: String,
    pub thumb: String,
}

pub trait MediaRenderer {
    fn photo(&self, html: &mut Html, photo: Option<&RichPhoto>) -> String;
    fn video(&self, html: &mut Html, document: Option<&RichDocument>) -> String;
    fn audio(&self, html: &mut Html, document: Option<&RichDocument>) -> String;
    fn file(&self, html: &mut Html, document: Option<&RichDocument>) -> String;
    fn generic(&self, html: &mut Html, data: &MediaData) -> String;
    fn photo_card(&self, html: &mut Html, data: &MediaData, photo: Option<&RichPhoto>) -> String;
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TailState {
    Empty,
    Media,
    Other,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SourceMetaKind {
    Source,
    EmbedPost,
}

/// `YYYY-style` date and time with seconds, or nothing for 0.
pub fn format_date_time(ts: i32) -> String {
    if ts == 0 {
        return String::new();
    }
    use chrono::{Local, TimeZone};
    match Local.timestamp_opt(ts as i64, 0).single() {
        Some(d) => d.format("%d.%m.%Y %H:%M:%S").to_string(),
        None => String::new(),
    }
}

fn block_attributes(classes: &str, kind: &str) -> Attrs {
    attrs(&[
        ("class", &format!("rich_block {classes}")),
        ("data-rich-kind", kind),
    ])
}

const GROUP_WIDTH_MAX: i32 = 430;
const GROUP_WIDTH_MIN: i32 = 100;
const GROUP_SKIP: i32 = 4;

pub struct Renderer<'a> {
    html: Html,
    message: &'a RichMessage,
    message_id: i32,
    relative_base: &'a str,
    media: &'a dyn MediaRenderer,
    link_active: bool,
    first_anchor_ids: BTreeMap<String, String>,
    anchor_occurrences: BTreeMap<String, i32>,
    text_anchor_ids: std::collections::HashMap<usize, String>,
    block_anchor_ids: std::collections::HashMap<usize, String>,
}

impl<'a> Renderer<'a> {
    pub fn new(
        depth: usize,
        message: &'a RichMessage,
        message_id: i32,
        relative_base: &'a str,
        media: &'a dyn MediaRenderer,
    ) -> Self {
        Renderer {
            html: Html::with_depth(depth),
            message,
            message_id,
            relative_base,
            media,
            link_active: false,
            first_anchor_ids: BTreeMap::new(),
            anchor_occurrences: BTreeMap::new(),
            text_anchor_ids: Default::default(),
            block_anchor_ids: Default::default(),
        }
    }

    fn find_photo(&self, id: u64) -> Option<&'a RichPhoto> {
        if id == 0 {
            return None;
        }
        self.message.photos.get(&id)
    }

    fn find_document(&self, id: u64) -> Option<&'a RichDocument> {
        if id == 0 {
            return None;
        }
        self.message.documents.get(&id)
    }

    fn media_item_size(&self, block: &RichBlock) -> Option<(i32, i32)> {
        let valid = |w: i32, h: i32| if w > 0 && h > 0 { Some((w, h)) } else { None };
        match block.kind {
            BlockKind::Photo => {
                let photo = self.find_photo(block.photo_id)?;
                if photo.file.relative_path.is_empty() {
                    return None;
                }
                valid(photo.width, photo.height)
            }
            BlockKind::Video => {
                let doc = self.find_document(block.document_id)?;
                let thumb_saved = doc
                    .thumb
                    .as_ref()
                    .map(|t| !t.file.relative_path.is_empty())
                    .unwrap_or(false);
                if doc.file.relative_path.is_empty() || !thumb_saved {
                    return None;
                }
                valid(doc.width, doc.height)
            }
            _ => None,
        }
    }

    fn media_group_sizes(&self, blocks: &[RichBlock]) -> Vec<(i32, i32)> {
        if blocks.is_empty() {
            return Vec::new();
        }
        let mut result = Vec::with_capacity(blocks.len());
        for block in blocks {
            let size = self.media_item_size(block);
            let has_url = block
                .optional_url
                .as_ref()
                .map(|u| !u.is_empty())
                .unwrap_or(false);
            match size {
                Some(s) if !caption_has_output(&block.caption) && !has_url => result.push(s),
                _ => return Vec::new(),
            }
        }
        result
    }

    fn tail_state_blocks(&self, blocks: &[RichBlock]) -> TailState {
        for b in blocks.iter().rev() {
            let state = self.tail_state(b);
            if state != TailState::Empty {
                return state;
            }
        }
        TailState::Empty
    }

    fn tail_state(&self, block: &RichBlock) -> TailState {
        use BlockKind as K;
        let has_url = |b: &RichBlock| {
            b.optional_url
                .as_ref()
                .map(|u| !u.is_empty())
                .unwrap_or(false)
        };
        match block.kind {
            K::Unsupported
            | K::Heading
            | K::Paragraph
            | K::Footer
            | K::Thinking
            | K::AuthorDate
            | K::Code
            | K::Divider
            | K::Math
            | K::Table
            | K::ButtonRow
            | K::Unknown => TailState::Other,
            K::Anchor => TailState::Empty,
            K::List => {
                let Some(item) = block.list_items.last() else {
                    return TailState::Empty;
                };
                if item.content == ItemContent::Blocks {
                    let state = self.tail_state_blocks(&item.blocks);
                    if state != TailState::Empty {
                        return state;
                    }
                }
                TailState::Other
            }
            K::Quote => {
                if text_has_output(&block.quote_caption) {
                    return TailState::Other;
                }
                if block.quote_content == QuoteContent::Blocks {
                    let state = self.tail_state_blocks(&block.blocks);
                    if state != TailState::Empty {
                        return state;
                    }
                }
                TailState::Other
            }
            K::Photo => {
                if caption_has_output(&block.caption) || has_url(block) {
                    TailState::Other
                } else {
                    TailState::Media
                }
            }
            K::Video | K::Audio | K::File | K::Map | K::InputMap => {
                if caption_has_output(&block.caption) {
                    TailState::Other
                } else {
                    TailState::Media
                }
            }
            K::Cover => self.tail_state_blocks(&block.blocks),
            K::Embed => {
                if caption_has_output(&block.caption)
                    || (block.poster_photo_id.is_some() && has_url(block))
                {
                    TailState::Other
                } else {
                    TailState::Media
                }
            }
            K::EmbedPost => {
                if caption_has_output(&block.caption) {
                    return TailState::Other;
                }
                let state = self.tail_state_blocks(&block.blocks);
                if state != TailState::Empty {
                    return state;
                }
                if block.url.is_empty() {
                    TailState::Media
                } else {
                    TailState::Other
                }
            }
            K::Collage | K::Slideshow => {
                if caption_has_output(&block.caption) {
                    TailState::Other
                } else {
                    self.tail_state_blocks(&block.blocks)
                }
            }
            K::Channel => TailState::Media,
            K::Details => {
                if block.open {
                    let state = self.tail_state_blocks(&block.blocks);
                    if state != TailState::Empty {
                        return state;
                    }
                }
                TailState::Other
            }
            K::RelatedArticles => match block.related_articles.last() {
                None => {
                    if text_has_output(&block.text) {
                        TailState::Other
                    } else {
                        TailState::Empty
                    }
                }
                Some(article) => {
                    if !article.url.is_empty() {
                        return TailState::Other;
                    }
                    let description = article.description.clone().unwrap_or_default();
                    if let (Some(pid), false) = (article.photo_id, description.is_empty()) {
                        let photo = self.find_photo(pid);
                        let view = file_view(photo.map(|p| &p.file));
                        if !no_file_description(view.skip).is_empty() {
                            return TailState::Other;
                        }
                    }
                    TailState::Media
                }
            },
        }
    }

    pub fn render_message(&mut self) -> String {
        self.collect_anchors();
        let mut result = self.html.push_tag(
            "div",
            attrs(&[
                ("class", "text rich_message"),
                ("data-rich-message", &self.message_id.to_string()),
                ("data-rich-part", bool_attribute(self.message.part)),
                ("dir", if self.message.rtl { "rtl" } else { "auto" }),
                ("inline", ""),
            ]),
        );
        let message = self.message;
        result += &self.render_blocks(&message.blocks);
        result += &self.html.pop_tag();
        result
    }

    fn render_caption(&mut self, caption: &RichCaption) -> String {
        let text = self.render_text(&caption.text);
        let credit = self.render_text(&caption.credit);
        if text.is_empty() && credit.is_empty() {
            return String::new();
        }
        let mut result = self.html.push_tag(
            "figcaption",
            attrs(&[("class", "rich_media_caption"), ("dir", "auto")]),
        );
        if !text.is_empty() {
            result += &self
                .html
                .push_tag("div", attrs(&[("class", "rich_caption_text")]));
            result += &text;
            result += &self.html.pop_tag();
        }
        if !credit.is_empty() {
            result += &self
                .html
                .push_tag("cite", attrs(&[("class", "rich_caption_credit")]));
            result += &credit;
            result += &self.html.pop_tag();
        }
        result += &self.html.pop_tag();
        result
    }

    fn render_source_meta(&mut self, source: &str, kind: SourceMetaKind) -> String {
        if source.is_empty() {
            return String::new();
        }
        let container_class = match kind {
            SourceMetaKind::Source => "rich_source_meta",
            SourceMetaKind::EmbedPost => "rich_embed_post_meta",
        };
        let mut result = self
            .html
            .push_tag("div", attrs(&[("class", container_class)]));
        let href = safe_http_href(source);
        match href {
            Some(href) if !self.link_active => {
                result += &self.html.push_tag(
                    "a",
                    attrs(&[
                        ("class", "rich_source_link"),
                        ("href", &href),
                        ("inline", ""),
                    ]),
                );
                let previous = self.link_active;
                self.link_active = true;
                result += &escape(source);
                result += &self.html.pop_tag();
                self.link_active = previous;
            }
            _ => {
                result += &self.html.push_tag(
                    "span",
                    attrs(&[("class", "rich_source_text"), ("inline", "")]),
                );
                result += &escape(source);
                result += &self.html.pop_tag();
            }
        }
        result += &self.html.pop_tag();
        result
    }

    fn render_photo(&mut self, block: &RichBlock) -> String {
        let mut a = block_attributes("rich_media_item", "photo");
        a.insert("data-photo-id".into(), block.photo_id.to_string());
        a.insert("data-spoiler".into(), bool_attribute(block.spoiler).into());
        if let Some(id) = block.optional_webpage_id {
            a.insert("data-webpage-id".into(), id.to_string());
        }
        let mut result = self.html.push_tag("figure", a);
        let photo = self.find_photo(block.photo_id);
        result += &self.media.photo(&mut self.html, photo);
        if let Some(url) = &block.optional_url {
            result += &self.render_source_meta(url, SourceMetaKind::Source);
        }
        result += &self.render_caption(&block.caption);
        result += &self.html.pop_tag();
        result
    }

    fn render_video(&mut self, block: &RichBlock) -> String {
        let mut a = block_attributes("rich_media_item", "video");
        a.insert("data-document-id".into(), block.document_id.to_string());
        a.insert(
            "data-autoplay".into(),
            bool_attribute(block.autoplay).into(),
        );
        a.insert("data-loop".into(), bool_attribute(block.looped).into());
        a.insert("data-spoiler".into(), bool_attribute(block.spoiler).into());
        let mut result = self.html.push_tag("figure", a);
        let document = self.find_document(block.document_id);
        result += &self.media.video(&mut self.html, document);
        result += &self.render_caption(&block.caption);
        result += &self.html.pop_tag();
        result
    }

    fn render_audio(&mut self, block: &RichBlock) -> String {
        let mut a = block_attributes("rich_media_item", "audio");
        a.insert("data-document-id".into(), block.document_id.to_string());
        let mut result = self.html.push_tag("figure", a);
        let document = self.find_document(block.document_id);
        result += &self.media.audio(&mut self.html, document);
        result += &self.render_caption(&block.caption);
        result += &self.html.pop_tag();
        result
    }

    fn render_file(&mut self, block: &RichBlock) -> String {
        let mut a = block_attributes("rich_media_item", "file");
        a.insert("data-document-id".into(), block.document_id.to_string());
        let mut result = self.html.push_tag("figure", a);
        let document = self.find_document(block.document_id);
        result += &self.media.file(&mut self.html, document);
        result += &self.render_caption(&block.caption);
        result += &self.html.pop_tag();
        result
    }

    fn render_media_group(
        &mut self,
        block: &RichBlock,
        kind: &str,
        render_group_caption: bool,
    ) -> String {
        let mut a = block_attributes("rich_media_group", kind);
        a.insert(
            "data-rich-has-items".into(),
            bool_attribute(!block.blocks.is_empty()).into(),
        );
        a.insert(
            "data-rich-items-end-media".into(),
            bool_attribute(self.tail_state_blocks(&block.blocks) == TailState::Media).into(),
        );
        let mut result = self.html.push_tag("section", a);
        result += &self
            .html
            .push_tag("div", attrs(&[("class", "rich_media_group_items")]));
        result += &self.render_blocks(&block.blocks);
        result += &self.html.pop_tag();
        if render_group_caption {
            result += &self.render_caption(&block.caption);
        }
        result += &self.html.pop_tag();
        result
    }

    fn render_collage(&mut self, block: &RichBlock) -> String {
        let sizes = self.media_group_sizes(&block.blocks);
        if sizes.is_empty() {
            return self.render_media_group(block, "collage", true);
        }
        let layout = crate::grouped_layout::layout_media_group(
            &sizes,
            GROUP_WIDTH_MAX,
            GROUP_WIDTH_MIN,
            GROUP_SKIP,
        );
        let mut full = crate::grouped_layout::Rect {
            x: 0,
            y: 0,
            w: 0,
            h: 0,
        };
        for part in &layout {
            full = full.united(part);
        }
        if layout.len() != sizes.len() || full.is_empty() {
            return self.render_media_group(block, "collage", true);
        }
        let mut a = block_attributes("rich_media_group", "collage");
        a.insert("data-rich-has-items".into(), bool_attribute(true).into());
        a.insert(
            "data-rich-items-end-media".into(),
            bool_attribute(true).into(),
        );
        let mut result = self.html.push_tag("section", a);
        result += &self.html.push_tag(
            "div",
            attrs(&[
                ("class", "rich_collage_box"),
                ("style", &aspect_ratio_style(full.w, full.h)),
            ]),
        );
        for (i, item) in block.blocks.iter().enumerate() {
            let g = layout[i];
            let style = format!(
                "left: {}; top: {}; width: {}; height: {}",
                percent_value(g.x - full.x, full.w),
                percent_value(g.y - full.y, full.h),
                percent_value(g.w, full.w),
                percent_value(g.h, full.h)
            );
            result += &self.html.push_tag(
                "div",
                attrs(&[("class", "rich_collage_item"), ("style", &style)]),
            );
            result += &self.render_block(item);
            result += &self.html.pop_tag();
        }
        result += &self.html.pop_tag();
        result += &self.render_caption(&block.caption);
        result += &self.html.pop_tag();
        result
    }

    fn render_slideshow(&mut self, block: &RichBlock) -> String {
        let sizes = self.media_group_sizes(&block.blocks);
        if sizes.is_empty() {
            return self.render_media_group(block, "slideshow", true);
        }
        let mut widest = sizes[0];
        for size in &sizes {
            if size.0 as i64 * widest.1 as i64 > widest.0 as i64 * size.1 as i64 {
                widest = *size;
            }
        }
        let count = block.blocks.len();
        let controls = count > 1;
        let mut a = block_attributes("rich_media_group", "slideshow");
        a.insert("data-rich-has-items".into(), bool_attribute(true).into());
        a.insert(
            "data-rich-items-end-media".into(),
            bool_attribute(true).into(),
        );
        let mut result = self.html.push_tag("section", a);
        result += &self.html.push_tag(
            "div",
            attrs(&[
                ("class", "rich_slideshow_box"),
                ("style", &aspect_ratio_style(widest.0, widest.1)),
            ]),
        );
        result += &self
            .html
            .push_tag("div", attrs(&[("class", "rich_slideshow_track")]));
        for item in &block.blocks {
            result += &self.html.push_tag("div", attrs(&[("class", "rich_slide")]));
            result += &self.render_block(item);
            result += &self.html.pop_tag();
        }
        result += &self.html.pop_tag();
        if controls {
            result += &self.html.push_tag(
                "button",
                attrs(&[
                    ("class", "rich_slideshow_prev"),
                    ("type", "button"),
                    ("inline", ""),
                ]),
            );
            result += "&#8592;";
            result += &self.html.pop_tag();
            result += &self.html.push_tag(
                "button",
                attrs(&[
                    ("class", "rich_slideshow_next"),
                    ("type", "button"),
                    ("inline", ""),
                ]),
            );
            result += "&#8594;";
            result += &self.html.pop_tag();
            result += &self
                .html
                .push_tag("div", attrs(&[("class", "rich_slideshow_dots")]));
            for _ in 0..count {
                result += &self.html.push_tag(
                    "button",
                    attrs(&[
                        ("class", "rich_slideshow_dot"),
                        ("type", "button"),
                        ("inline", ""),
                    ]),
                );
                result += &self.html.pop_tag();
            }
            result += &self.html.pop_tag();
        }
        result += &self.html.pop_tag();
        result += &self.render_caption(&block.caption);
        result += &self.html.pop_tag();
        result
    }

    fn render_embed(&mut self, block: &RichBlock) -> String {
        let has_poster = block.poster_photo_id.is_some();
        let has_url = block
            .optional_url
            .as_ref()
            .map(|u| !u.is_empty())
            .unwrap_or(false);
        let has_meta = has_poster && has_url;
        let mut a = block_attributes("rich_media_item rich_embed", "embed");
        a.insert(
            "data-full-width".into(),
            bool_attribute(block.full_width).into(),
        );
        a.insert(
            "data-allow-scrolling".into(),
            bool_attribute(block.allow_scrolling).into(),
        );
        a.insert("data-rich-has-meta".into(), bool_attribute(has_meta).into());
        if let Some(id) = block.poster_photo_id {
            a.insert("data-poster-photo-id".into(), id.to_string());
        }
        if let Some(w) = block.width {
            a.insert("data-source-width".into(), w.to_string());
        }
        if let Some(h) = block.height {
            a.insert("data-source-height".into(), h.to_string());
        }
        let mut result = self.html.push_tag("figure", a);
        if let Some(pid) = block.poster_photo_id {
            let poster = self.find_photo(pid);
            result += &self.media.photo(&mut self.html, poster);
            if let Some(url) = &block.optional_url {
                result += &self.render_source_meta(url, SourceMetaKind::Source);
            }
        } else {
            let mut media = MediaData {
                title: "Embed".into(),
                classes: "media_file".into(),
                ..Default::default()
            };
            if let Some(url) = &block.optional_url {
                media.status = url.clone();
                if let Some(href) = safe_http_href(url) {
                    media.link = href;
                }
            }
            result += &self.media.generic(&mut self.html, &media);
        }
        result += &self.render_caption(&block.caption);
        result += &self.html.pop_tag();
        result
    }

    fn render_embed_post(&mut self, block: &RichBlock) -> String {
        let photo = self.find_photo(block.author_photo_id);
        let has_meta = !block.url.is_empty();
        let mut a = block_attributes("rich_embed_post", "embed-post");
        a.insert("data-webpage-id".into(), block.webpage_id.to_string());
        a.insert(
            "data-author-photo-id".into(),
            block.author_photo_id.to_string(),
        );
        a.insert("data-date".into(), block.date.to_string());
        a.insert("data-rich-has-meta".into(), bool_attribute(has_meta).into());
        a.insert(
            "data-rich-has-items".into(),
            bool_attribute(!block.blocks.is_empty()).into(),
        );
        a.insert(
            "data-rich-items-end-media".into(),
            bool_attribute(self.tail_state_blocks(&block.blocks) == TailState::Media).into(),
        );
        let mut result = self.html.push_tag("section", a);
        result += &self
            .html
            .push_tag("div", attrs(&[("class", "rich_embed_post_author")]));
        let media = MediaData {
            title: if block.author.is_empty() {
                "Embed post".into()
            } else {
                block.author.clone()
            },
            description: no_file_description(file_view(photo.map(|p| &p.file)).skip).into(),
            status: if block.date != 0 {
                format_date_time(block.date)
            } else {
                String::new()
            },
            classes: "media_contact".into(),
            ..Default::default()
        };
        result += &self.media.photo_card(&mut self.html, &media, photo);
        result += &self.html.pop_tag();
        result += &self.render_source_meta(&block.url, SourceMetaKind::EmbedPost);
        result += &self
            .html
            .push_tag("div", attrs(&[("class", "rich_embed_post_blocks")]));
        result += &self.render_blocks(&block.blocks);
        result += &self.html.pop_tag();
        result += &self.render_caption(&block.caption);
        result += &self.html.pop_tag();
        result
    }

    fn render_channel(&mut self, block: &RichBlock) -> String {
        let channel = &block.channel;
        let mut a = block_attributes("rich_reference_block", "channel");
        a.insert(
            "data-channel-source".into(),
            channel_source_name(channel.source).into(),
        );
        a.insert("data-channel-id".into(), channel.id.to_string());
        a.insert(
            "data-broadcast".into(),
            bool_attribute(channel.broadcast).into(),
        );
        a.insert(
            "data-megagroup".into(),
            bool_attribute(channel.megagroup).into(),
        );
        a.insert(
            "data-monoforum".into(),
            bool_attribute(channel.monoforum).into(),
        );
        if let Some(h) = channel.access_hash {
            a.insert("data-access-hash".into(), h.to_string());
        }
        let mut result = self.html.push_tag("section", a);
        let mut media = MediaData {
            title: match &channel.title {
                Some(t) if !t.is_empty() => t.clone(),
                _ => "Channel".into(),
            },
            status: channel_status(channel),
            classes: "media_contact".into(),
            ..Default::default()
        };
        if let Some(u) = &channel.username {
            if is_ascii_word(u, USERNAME_MIN_LENGTH, USERNAME_MAX_LENGTH) {
                if let Some(href) = safe_http_href(&format!("{INTERNAL_LINKS_DOMAIN}{u}")) {
                    media.link = href;
                }
            }
        }
        result += &self.media.generic(&mut self.html, &media);
        result += &self.html.pop_tag();
        result
    }

    fn render_map(&mut self, block: &RichBlock, kind: &str) -> String {
        let point = &block.map_point;
        let mut a = block_attributes("rich_reference_block", kind);
        a.insert(
            "data-point-source".into(),
            map_source_name(point.source).into(),
        );
        a.insert("data-zoom".into(), block.zoom.to_string());
        a.insert("data-source-width".into(), block.map_width.to_string());
        a.insert("data-source-height".into(), block.map_height.to_string());
        if let Some(h) = point.access_hash {
            a.insert("data-access-hash".into(), h.to_string());
        }
        if let Some(r) = point.accuracy_radius {
            a.insert("data-accuracy-radius".into(), r.to_string());
        }
        let mut result = self.html.push_tag("figure", a);
        let mut media = MediaData {
            title: "Location".into(),
            classes: "media_location".into(),
            ..Default::default()
        };
        if let Some((lat, lon)) = map_coordinates(point) {
            let joined = format!("{lat},{lon}");
            media.status = format!("{lat}, {lon}");
            let target = format!("https://maps.google.com/maps?q={joined}&ll={joined}&z=16");
            if let Some(href) = safe_http_href(&target) {
                media.link = href;
            }
        }
        result += &self.media.generic(&mut self.html, &media);
        result += &self.render_caption(&block.caption);
        result += &self.html.pop_tag();
        result
    }

    fn render_related_article(&mut self, article: &RelatedArticle, index: usize) -> String {
        let mut a = attrs(&[
            ("class", "rich_related_article"),
            ("data-rich-article-index", &index.to_string()),
            ("data-webpage-id", &article.webpage_id.to_string()),
        ]);
        if let Some(id) = article.photo_id {
            a.insert("data-photo-id".into(), id.to_string());
        }
        if let Some(d) = article.published_date {
            a.insert("data-published-date".into(), d.to_string());
        }
        let mut result = self.html.push_tag("article", a);
        let mut media = MediaData {
            title: match &article.title {
                Some(t) if !t.is_empty() => t.clone(),
                _ => "Related article".into(),
            },
            ..Default::default()
        };
        if let Some(author) = &article.author {
            if !author.is_empty() {
                media.status = author.clone();
            }
        }
        let published = article
            .published_date
            .map(format_date_time)
            .unwrap_or_default();
        if !published.is_empty() {
            if !media.status.is_empty() {
                media.status.push_str(", ");
            }
            media.status.push_str(&published);
        }
        let description = article.description.clone().unwrap_or_default();
        let mut displaced = false;
        if let Some(pid) = article.photo_id {
            let photo = self.find_photo(pid);
            let file_description = no_file_description(file_view(photo.map(|p| &p.file)).skip);
            media.classes = "media_photo".into();
            media.description = if file_description.is_empty() {
                description.clone()
            } else {
                file_description.to_string()
            };
            displaced = !file_description.is_empty() && !description.is_empty();
            result += &self.media.photo_card(&mut self.html, &media, photo);
        } else {
            media.classes = "media_file".into();
            media.description = description.clone();
            result += &self.media.generic(&mut self.html, &media);
        }
        if displaced {
            result += &self
                .html
                .push_tag("div", attrs(&[("class", "rich_article_description")]));
            result += &escape(&description);
            result += &self.html.pop_tag();
        }
        result += &self.render_source_meta(&article.url, SourceMetaKind::Source);
        result += &self.html.pop_tag();
        result
    }

    fn render_related_articles(&mut self, block: &RichBlock) -> String {
        let title = self.render_text(&block.text);
        let mut result = self.html.push_tag(
            "section",
            block_attributes("rich_related_articles", "related-articles"),
        );
        if !title.is_empty() {
            result += &self.html.push_tag(
                "div",
                attrs(&[("class", "rich_related_title"), ("dir", "auto")]),
            );
            result += &title;
            result += &self.html.pop_tag();
        }
        result += &self
            .html
            .push_tag("div", attrs(&[("class", "rich_related_items")]));
        for (index, article) in block.related_articles.iter().enumerate() {
            result += &self.render_related_article(article, index);
        }
        result += &self.html.pop_tag();
        result += &self.html.pop_tag();
        result
    }
}

// Anchors: every named anchor gets an id that is unique within the message.

fn key_of<T>(value: &T) -> usize {
    value as *const T as usize
}

impl Renderer<'_> {
    fn collect_anchors(&mut self) {
        self.first_anchor_ids.clear();
        self.anchor_occurrences.clear();
        self.text_anchor_ids.clear();
        self.block_anchor_ids.clear();
        let message = self.message;
        self.collect_block_anchors_list(&message.blocks);
    }

    fn register_anchor(&mut self, name: &str) -> String {
        let base = format!(
            "rich-message-{}-anchor-{}",
            self.message_id,
            anchor_token(name)
        );
        let occurrence = self.anchor_occurrences.entry(base.clone()).or_insert(0);
        *occurrence += 1;
        let result = if *occurrence > 1 {
            format!("{base}-{occurrence}")
        } else {
            base
        };
        self.first_anchor_ids
            .entry(name.to_string())
            .or_insert_with(|| result.clone());
        result
    }

    fn collect_text_anchors(&mut self, text: &RichText) {
        use RichTextKind as K;
        match text.kind {
            K::Anchor => {
                let id = self.register_anchor(&text.data);
                self.text_anchor_ids.entry(key_of(text)).or_insert(id);
                self.collect_text_anchors_list(&text.children);
            }
            K::Concat
            | K::Bold
            | K::Italic
            | K::Underline
            | K::Strike
            | K::Fixed
            | K::Url
            | K::Email
            | K::Phone
            | K::Subscript
            | K::Superscript
            | K::Marked
            | K::Spoiler
            | K::Mention
            | K::Hashtag
            | K::BotCommand
            | K::Cashtag
            | K::AutoUrl
            | K::AutoEmail
            | K::AutoPhone
            | K::BankCard
            | K::TonAddress
            | K::MentionName
            | K::FormattedDate
            | K::Button => self.collect_text_anchors_list(&text.children),
            K::Diff => {
                self.collect_text_anchors_list(&text.children);
                self.collect_text_anchors_list(&text.old_children);
            }
            K::Empty | K::Plain | K::Math | K::CustomEmoji | K::InlineImage => {}
        }
    }

    fn collect_text_anchors_list(&mut self, texts: &[RichText]) {
        for t in texts {
            self.collect_text_anchors(t);
        }
    }

    fn collect_caption_anchors(&mut self, c: &RichCaption) {
        self.collect_text_anchors(&c.text);
        self.collect_text_anchors(&c.credit);
    }

    fn collect_block_anchors_list(&mut self, blocks: &[RichBlock]) {
        for b in blocks {
            self.collect_block_anchors(b);
        }
    }

    fn collect_block_anchors(&mut self, block: &RichBlock) {
        use BlockKind as K;
        match block.kind {
            K::Heading | K::Paragraph | K::Footer | K::Thinking | K::AuthorDate | K::Code => {
                self.collect_text_anchors(&block.text)
            }
            K::Anchor => {
                let id = self.register_anchor(&block.name);
                self.block_anchor_ids.entry(key_of(block)).or_insert(id);
            }
            K::List => {
                for item in &block.list_items {
                    match item.content {
                        ItemContent::Text => {
                            if let Some(t) = &item.text {
                                self.collect_text_anchors(t);
                            }
                        }
                        ItemContent::Blocks => self.collect_block_anchors_list(&item.blocks),
                    }
                }
            }
            K::Quote => {
                match block.quote_content {
                    QuoteContent::Text => self.collect_text_anchors(&block.text),
                    QuoteContent::Blocks => self.collect_block_anchors_list(&block.blocks),
                }
                self.collect_text_anchors(&block.quote_caption);
            }
            K::Photo | K::Video | K::Audio | K::File | K::Embed | K::Map | K::InputMap => {
                self.collect_caption_anchors(&block.caption)
            }
            K::EmbedPost => {
                self.collect_block_anchors_list(&block.blocks);
                self.collect_caption_anchors(&block.caption);
            }
            K::Cover => self.collect_block_anchors_list(&block.blocks),
            K::Collage | K::Slideshow => {
                self.collect_block_anchors_list(&block.blocks);
                self.collect_caption_anchors(&block.caption);
            }
            K::Table => {
                self.collect_text_anchors(&block.text);
                for row in &block.table_rows {
                    for cell in &row.cells {
                        if let Some(t) = &cell.text {
                            self.collect_text_anchors(t);
                        }
                    }
                }
            }
            K::Details => {
                self.collect_text_anchors(&block.text);
                self.collect_block_anchors_list(&block.blocks);
            }
            K::RelatedArticles => self.collect_text_anchors(&block.text),
            K::ButtonRow => {
                for b in &block.buttons {
                    self.collect_text_anchors(b);
                }
            }
            K::Unsupported | K::Divider | K::Channel | K::Math | K::Unknown => {}
        }
    }

    fn fragment_href(&self, target: &str) -> Option<String> {
        if !is_strict_target(target) || !target.starts_with('#') {
            return None;
        }
        self.first_anchor_ids
            .get(&target[1..])
            .map(|id| format!("#{id}"))
    }

    fn text_anchor_id(&self, text: &RichText) -> Option<String> {
        self.text_anchor_ids.get(&key_of(text)).cloned()
    }

    fn block_anchor_id(&self, block: &RichBlock) -> Option<String> {
        self.block_anchor_ids.get(&key_of(block)).cloned()
    }

    fn render_texts(&mut self, texts: &[RichText]) -> String {
        let mut result = String::new();
        for t in texts {
            result += &self.render_text(t);
        }
        result
    }
}

// Inline pieces: text children, links, buttons.

fn emplace(a: &mut Attrs, key: &str, value: &str) {
    a.entry(key.to_string())
        .or_insert_with(|| value.to_string());
}

impl Renderer<'_> {
    fn render_text_children(&mut self, text: &RichText) -> String {
        if !text.children.is_empty() {
            return self.render_texts(&text.children);
        }
        let mut result = self.html.push_tag(
            "span",
            attrs(&[
                ("class", "rich_inline_fallback"),
                ("data-rich-kind", "unsupported-text"),
                ("inline", ""),
            ]),
        );
        result += "Unsupported rich text";
        result += &self.html.pop_tag();
        result
    }

    fn render_text_link(
        &mut self,
        text: &RichText,
        href: Option<&str>,
        mut attributes: Attrs,
    ) -> String {
        emplace(&mut attributes, "inline", "");
        match href {
            Some(href) if !self.link_active => {
                attributes.insert("href".into(), href.to_string());
                let mut result = self.html.push_tag("a", attributes);
                let previous = self.link_active;
                self.link_active = true;
                result += &self.render_text_children(text);
                self.link_active = previous;
                result += &self.html.pop_tag();
                result
            }
            _ => {
                attributes.remove("href");
                attributes.remove("onclick");
                emplace(&mut attributes, "class", "rich_inert_link");
                let mut result = self.html.push_tag("span", attributes);
                result += &self.render_text_children(text);
                result += &self.html.pop_tag();
                result
            }
        }
    }

    fn render_button(&mut self, button: &RichText, inline_button: bool) -> String {
        let payload = button
            .button
            .as_ref()
            .expect("a button text always has its payload");
        let action = &payload.action;
        let mut attributes = button_attributes(action, payload.style, inline_button);
        let mut tag = "span";
        let nested = self.link_active;
        let url = if action.kind == InlineActionKind::Url {
            safe_message_href(&action.url)
        } else {
            None
        };
        match url {
            Some(u) if !nested => {
                tag = "a";
                emplace(&mut attributes, "href", &u);
            }
            _ if !nested && action.kind == InlineActionKind::CopyText => {
                tag = "button";
                emplace(&mut attributes, "type", "button");
                emplace(
                    &mut attributes,
                    "onclick",
                    "return ShowTextCopied(decodeURIComponent(this.dataset.copyText))",
                );
            }
            _ => {}
        }
        let mut result = self.html.push_tag(tag, attributes);
        let previous = self.link_active;
        self.link_active = true;
        if let Some(first) = button.children.first() {
            result += &self.render_text(first);
        }
        self.link_active = previous;
        result += &self.html.pop_tag();
        result
    }

    fn render_button_row(&mut self, block: &RichBlock) -> String {
        let alignment = match block.button_alignment {
            ButtonAlignment::Stretch => "stretch",
            ButtonAlignment::Left => "left",
            ButtonAlignment::Center => "center",
            ButtonAlignment::Right => "right",
        };
        let mut a = block_attributes(
            &format!("rich_button_row rich_button_align_{alignment}"),
            "button-row",
        );
        a.insert("data-button-alignment".into(), alignment.into());
        let mut result = self.html.push_tag("div", a);
        result += &self
            .html
            .push_tag("div", attrs(&[("class", "rich_button_group")]));
        for button in &block.buttons {
            result += &self.render_button(button, false);
        }
        result += &self.html.pop_tag();
        result += &self.html.pop_tag();
        result
    }

    fn render_custom_emoji_link(&mut self, alt: &str, mut attributes: Attrs) -> String {
        emplace(&mut attributes, "inline", "");
        if self.link_active {
            attributes.remove("href");
            attributes.remove("onclick");
            emplace(&mut attributes, "class", "rich_inert_link");
            let mut result = self.html.push_tag("span", attributes);
            result += alt;
            result += &self.html.pop_tag();
            return result;
        }
        let mut result = self.html.push_tag("a", attributes);
        let previous = self.link_active;
        self.link_active = true;
        result += alt;
        self.link_active = previous;
        result += &self.html.pop_tag();
        result
    }
}

// Text.

impl Renderer<'_> {
    fn wrap_children(&mut self, tag: &str, mut attributes: Attrs, text: &RichText) -> String {
        emplace(&mut attributes, "inline", "");
        let mut result = self.html.push_tag(tag, attributes);
        result += &self.render_text_children(text);
        result += &self.html.pop_tag();
        result
    }

    pub fn render_text(&mut self, text: &RichText) -> String {
        use RichTextKind as K;
        match text.kind {
            K::Empty => String::new(),
            K::Plain => escape(&text.text),
            K::Concat => self.render_texts(&text.children),
            K::Bold => self.wrap_children("strong", Attrs::new(), text),
            K::Italic => self.wrap_children("em", Attrs::new(), text),
            K::Underline => self.wrap_children("u", Attrs::new(), text),
            K::Strike => self.wrap_children("s", Attrs::new(), text),
            K::Fixed => self.wrap_children("code", attrs(&[("class", "rich_inline_code")]), text),
            K::Url => {
                let href = if text.data.starts_with('#') {
                    self.fragment_href(&text.data)
                } else {
                    safe_http_href(&text.data)
                };
                let mut a = Attrs::new();
                if text.id != 0 {
                    a.insert("data-webpage-id".into(), text.id.to_string());
                }
                self.render_text_link(text, href.as_deref(), a)
            }
            K::Email => {
                let href = safe_email_href(&text.data);
                self.render_text_link(text, href.as_deref(), Attrs::new())
            }
            K::Phone => {
                let href = safe_phone_href(&text.data);
                self.render_text_link(text, href.as_deref(), Attrs::new())
            }
            K::Subscript => self.wrap_children("sub", Attrs::new(), text),
            K::Superscript => self.wrap_children("sup", Attrs::new(), text),
            K::Marked => self.wrap_children("mark", Attrs::new(), text),
            K::Anchor => {
                let mut a = attrs(&[("class", "rich_reference"), ("inline", "")]);
                if let Some(id) = self.text_anchor_id(text) {
                    emplace(&mut a, "id", &id);
                }
                let mut result = self.html.push_tag("span", a);
                result += &self.render_texts(&text.children);
                result += &self.html.pop_tag();
                result
            }
            K::Math => {
                let mut result = self.html.push_tag(
                    "span",
                    attrs(&[
                        ("class", "rich_math_inline"),
                        ("data-rich-kind", "inline-math"),
                        ("dir", "ltr"),
                        ("inline", ""),
                    ]),
                );
                result += &escape(&text.data);
                result += &self.html.pop_tag();
                result
            }
            K::CustomEmoji => {
                let mut result = self.html.push_tag(
                    "span",
                    attrs(&[
                        ("class", "rich_custom_emoji"),
                        ("data-document-id", &text.id.to_string()),
                        ("inline", ""),
                    ]),
                );
                let alt = escape(&text.text);
                let unavailable = text.custom_emoji_data == UNAVAILABLE_EMOJI;
                let not_loaded =
                    text.custom_emoji_data.is_empty() || is_ascii_decimal(&text.custom_emoji_data);
                let href = if unavailable || not_loaded {
                    None
                } else {
                    safe_relative_emoji_href(&text.custom_emoji_data, self.relative_base)
                };
                if let Some(href) = href {
                    result += &self.render_custom_emoji_link(&alt, attrs(&[("href", &href)]));
                } else if unavailable || not_loaded {
                    let onclick = if unavailable {
                        "return ShowNotAvailableEmoji()"
                    } else {
                        "return ShowNotLoadedEmoji()"
                    };
                    result += &self.render_custom_emoji_link(
                        &alt,
                        attrs(&[("href", ""), ("onclick", onclick)]),
                    );
                } else {
                    result += &alt;
                }
                result += &self.html.pop_tag();
                result
            }
            K::Spoiler => {
                let mut result = self.html.push_tag(
                    "span",
                    attrs(&[
                        ("class", "spoiler hidden"),
                        ("inline", ""),
                        ("onclick", "ShowSpoiler(this)"),
                    ]),
                );
                result += &self
                    .html
                    .push_tag("span", attrs(&[("aria-hidden", "true"), ("inline", "")]));
                result += &self.render_text_children(text);
                result += &self.html.pop_tag();
                result += &self.html.pop_tag();
                result
            }
            K::Mention => {
                let href = plain_target(text).and_then(|t| safe_mention_href(&t));
                self.render_text_link(text, href.as_deref(), Attrs::new())
            }
            K::Hashtag | K::BotCommand | K::Cashtag => {
                let target = plain_target(text);
                let prefix = match text.kind {
                    K::Hashtag => '#',
                    K::BotCommand => '/',
                    _ => '$',
                };
                let action = target.and_then(|t| safe_action_data(&t, prefix));
                let Some(action) = action else {
                    return self.render_text_link(text, None, Attrs::new());
                };
                let attribute = if text.kind == K::BotCommand {
                    "data-command"
                } else {
                    "data-tag"
                };
                let callback = match text.kind {
                    K::Hashtag => "return ShowHashtag(this.dataset.tag)",
                    K::BotCommand => "return ShowBotCommand(this.dataset.command)",
                    _ => "return ShowCashtag(this.dataset.tag)",
                };
                self.render_text_link(
                    text,
                    Some(""),
                    attrs(&[(attribute, &action), ("onclick", callback)]),
                )
            }
            K::AutoUrl => {
                let href = plain_target(text).and_then(|t| safe_http_href(&t));
                self.render_text_link(text, href.as_deref(), Attrs::new())
            }
            K::AutoEmail => {
                let href = plain_target(text).and_then(|t| safe_email_href(&t));
                self.render_text_link(text, href.as_deref(), Attrs::new())
            }
            K::AutoPhone => {
                let href = plain_target(text).and_then(|t| safe_phone_href(&t));
                self.render_text_link(text, href.as_deref(), Attrs::new())
            }
            K::BankCard => self.wrap_children("span", attrs(&[("class", "rich_bank_card")]), text),
            K::TonAddress => {
                self.wrap_children("span", attrs(&[("class", "rich_ton_address")]), text)
            }
            K::MentionName => self.render_text_link(
                text,
                Some(""),
                attrs(&[
                    ("data-user-id", &text.id.to_string()),
                    ("onclick", "return ShowMentionName()"),
                ]),
            ),
            K::FormattedDate => {
                let mut result = self.html.push_tag(
                    "time",
                    attrs(&[
                        ("class", "rich_formatted_date"),
                        ("data-date", &text.date.to_string()),
                        ("data-day-of-week", bool_attribute(text.day_of_week)),
                        ("data-long-date", bool_attribute(text.long_date)),
                        ("data-long-time", bool_attribute(text.long_time)),
                        ("data-relative", bool_attribute(text.relative)),
                        ("data-short-date", bool_attribute(text.short_date)),
                        ("data-short-time", bool_attribute(text.short_time)),
                        ("datetime", &iso_date(text.date)),
                        ("dir", "ltr"),
                        ("inline", ""),
                        ("title", &format_date_time(text.date)),
                    ]),
                );
                result += &self.render_text_children(text);
                result += &self.html.pop_tag();
                result
            }
            K::InlineImage => {
                let mut result = self.html.push_tag(
                    "span",
                    attrs(&[
                        ("class", "rich_inline_fallback"),
                        ("data-document-id", &text.id.to_string()),
                        ("data-rich-kind", "inline-image"),
                        ("data-source-height", &text.height.to_string()),
                        ("data-source-width", &text.width.to_string()),
                        ("inline", ""),
                    ]),
                );
                result += "Inline image";
                result += &self.html.pop_tag();
                result
            }
            K::Diff => {
                let mut result = self.html.push_tag(
                    "span",
                    attrs(&[
                        ("class", "rich_diff"),
                        ("data-rich-kind", "diff"),
                        ("inline", ""),
                    ]),
                );
                result += &self.render_text_children(text);
                result += &self.html.push_tag(
                    "del",
                    attrs(&[("class", "rich_diff_previous"), ("inline", "")]),
                );
                result += "Previous: ";
                if text.old_children.is_empty() {
                    result += &self.html.push_tag(
                        "span",
                        attrs(&[
                            ("class", "rich_inline_fallback"),
                            ("data-rich-kind", "unsupported-text"),
                            ("inline", ""),
                        ]),
                    );
                    result += "Unsupported rich text";
                    result += &self.html.pop_tag();
                } else {
                    result += &self.render_texts(&text.old_children);
                }
                result += &self.html.pop_tag();
                result += &self.html.pop_tag();
                result
            }
            K::Button => self.render_button(text, true),
        }
    }
}

// Blocks: text based ones.

impl Renderer<'_> {
    fn render_text_block(&mut self, tag: &str, attributes: Attrs, text: &RichText) -> String {
        let mut result = self.html.push_tag(tag, attributes);
        result += &self.render_text(text);
        result += &self.html.pop_tag();
        result
    }

    fn render_author_date(&mut self, block: &RichBlock) -> String {
        let mut a = block_attributes("rich_author_date", "author-date");
        emplace(&mut a, "dir", "auto");
        let mut result = self.html.push_tag("address", a);
        result += &self.render_text(&block.text);
        if block.date != 0 {
            result += " \u{2022} ";
            result += &self.html.push_tag(
                "time",
                attrs(&[
                    ("data-date", &block.date.to_string()),
                    ("datetime", &iso_date(block.date)),
                    ("dir", "ltr"),
                    ("inline", ""),
                ]),
            );
            result += &escape(&format_date_time(block.date));
            result += &self.html.pop_tag();
        }
        result += &self.html.pop_tag();
        result
    }

    fn render_code(&mut self, block: &RichBlock) -> String {
        let mut a = block_attributes("rich_code", "code");
        emplace(&mut a, "dir", "auto");
        emplace(&mut a, "inline", "");
        if !block.language.is_empty() {
            emplace(&mut a, "data-language", &block.language);
        }
        let mut result = self.html.push_tag("pre", a);
        result += &self.html.push_tag(
            "code",
            attrs(&[("class", "rich_code_content"), ("inline", "")]),
        );
        result += &self.render_text(&block.text);
        result += &self.html.pop_tag();
        result += &self.html.pop_tag();
        result
    }

    fn render_list(&mut self, block: &RichBlock) -> String {
        let ordered = block.list_kind == ListKind::Ordered;
        let mut a = block_attributes("rich_list", if ordered { "ordered-list" } else { "list" });
        if ordered {
            if block.ordered_list.reversed {
                emplace(&mut a, "reversed", "");
            }
            if let Some(start) = block.ordered_list.start {
                emplace(&mut a, "start", &start.to_string());
            }
            if let Some(kind) = &block.ordered_list.kind {
                let native = kind.len() == 1
                    && matches!(kind.as_bytes()[0], b'1' | b'a' | b'A' | b'i' | b'I');
                emplace(&mut a, if native { "type" } else { "data-list-type" }, kind);
            }
        }
        let mut result = self.html.push_tag(if ordered { "ol" } else { "ul" }, a);
        for item in &block.list_items {
            result += &self.render_list_item(item, ordered);
        }
        result += &self.html.pop_tag();
        result
    }

    fn render_list_item(&mut self, item: &RichListItem, ordered: bool) -> String {
        let task = item.task_state != TaskState::None;
        let checked = item.task_state == TaskState::Checked;
        let custom_marker =
            ordered && !task && item.num.as_ref().map(|n| !n.is_empty()).unwrap_or(false);
        let mut classes = String::from("rich_list_item");
        if task {
            classes.push_str(" rich_task_item");
        } else if custom_marker {
            classes.push_str(" rich_order_item");
        }
        let mut a = attrs(&[
            ("class", &classes),
            (
                "data-rich-content",
                if item.content == ItemContent::Blocks {
                    "blocks"
                } else {
                    "text"
                },
            ),
        ]);
        if let Some(num) = &item.num {
            emplace(&mut a, "data-num", num);
        }
        if let Some(kind) = &item.kind {
            emplace(&mut a, "data-item-type", kind);
        }
        if let Some(value) = item.value {
            emplace(&mut a, "value", &value.to_string());
        }
        let mut result = self.html.push_tag("li", a);
        if task {
            result += &self.html.push_tag(
                "span",
                attrs(&[
                    ("aria-checked", bool_attribute(checked)),
                    ("class", "rich_task_marker"),
                    ("inline", ""),
                    ("role", "checkbox"),
                ]),
            );
            result += if checked { "\u{2611}" } else { "\u{2610}" };
            result += &self.html.pop_tag();
        } else if custom_marker {
            result += &self.html.push_tag(
                "span",
                attrs(&[("class", "rich_order_marker"), ("inline", "")]),
            );
            result += &escape(item.num.as_deref().unwrap_or_default());
            result += &self.html.pop_tag();
        }
        result += &self.html.push_tag(
            if item.content == ItemContent::Blocks {
                "div"
            } else {
                "span"
            },
            attrs(&[("class", "rich_list_content"), ("inline", "")]),
        );
        match item.content {
            ItemContent::Text => {
                if let Some(t) = &item.text {
                    result += &self.render_text(t);
                }
            }
            ItemContent::Blocks => result += &self.render_blocks(&item.blocks),
        }
        result += &self.html.pop_tag();
        result += &self.html.pop_tag();
        result
    }
}

// Blocks: quotes, details, tables and the dispatcher.

impl Renderer<'_> {
    fn render_quote(&mut self, block: &RichBlock) -> String {
        let kind = if block.pullquote {
            "pullquote"
        } else if block.quote_content == QuoteContent::Blocks {
            "quote-blocks"
        } else {
            "quote"
        };
        let classes = if block.pullquote {
            "rich_quote rich_pullquote"
        } else {
            "rich_quote"
        };
        let mut result = self.html.push_tag(
            if block.pullquote {
                "aside"
            } else {
                "blockquote"
            },
            block_attributes(classes, kind),
        );
        result += &self
            .html
            .push_tag("div", attrs(&[("class", "rich_quote_body")]));
        match block.quote_content {
            QuoteContent::Text => result += &self.render_text(&block.text),
            QuoteContent::Blocks => result += &self.render_blocks(&block.blocks),
        }
        result += &self.html.pop_tag();
        let caption = self.render_text(&block.quote_caption);
        if !caption.is_empty() {
            result += &self
                .html
                .push_tag("cite", attrs(&[("class", "rich_quote_caption")]));
            result += &caption;
            result += &self.html.pop_tag();
        }
        result += &self.html.pop_tag();
        result
    }

    fn render_details(&mut self, block: &RichBlock) -> String {
        let mut a = block_attributes("rich_details", "details");
        if block.open {
            emplace(&mut a, "open", "");
        }
        let mut result = self.html.push_tag("details", a);
        result += &self.html.push_tag("summary", Attrs::new());
        result += &self.render_text(&block.text);
        result += &self.html.pop_tag();
        result += &self
            .html
            .push_tag("div", attrs(&[("class", "rich_details_body")]));
        result += &self.render_blocks(&block.blocks);
        result += &self.html.pop_tag();
        result += &self.html.pop_tag();
        result
    }

    fn render_table(&mut self, block: &RichBlock) -> String {
        let mut wrapper = block_attributes("rich_table_wrap", "table");
        emplace(&mut wrapper, "tabindex", "0");
        let mut result = self.html.push_tag("div", wrapper);
        let mut classes = String::from("rich_table");
        if block.bordered {
            classes.push_str(" bordered");
        }
        if block.striped {
            classes.push_str(" striped");
        }
        if block.compact {
            classes.push_str(" compact");
        }
        result += &self.html.push_tag("table", attrs(&[("class", &classes)]));
        let caption = self.render_text(&block.text);
        if !caption.is_empty() {
            result += &self.html.push_tag("caption", Attrs::new());
            result += &caption;
            result += &self.html.pop_tag();
        }
        result += &self.html.push_tag("tbody", Attrs::new());
        for row in &block.table_rows {
            result += &self.render_table_row(row);
        }
        result += &self.html.pop_tag();
        result += &self.html.pop_tag();
        result += &self.html.pop_tag();
        result
    }

    fn render_table_row(&mut self, row: &TableRow) -> String {
        let mut result = self.html.push_tag("tr", Attrs::new());
        for cell in &row.cells {
            result += &self.render_table_cell(cell);
        }
        result += &self.html.pop_tag();
        result
    }

    fn render_table_cell(&mut self, cell: &TableCell) -> String {
        let horizontal = match cell.align {
            TableAlign::Left => "rich_align_left",
            TableAlign::Center => "rich_align_center",
            TableAlign::Right => "rich_align_right",
        };
        let vertical = match cell.valign {
            TableValign::Top => "rich_valign_top",
            TableValign::Middle => "rich_valign_middle",
            TableValign::Bottom => "rich_valign_bottom",
        };
        let mut a = attrs(&[("class", &format!("{horizontal} {vertical}"))]);
        for (value, name, source_name) in [
            (cell.colspan, "colspan", "data-source-colspan"),
            (cell.rowspan, "rowspan", "data-source-rowspan"),
        ] {
            if let Some(source) = value.filter(|v| *v > 0) {
                let span = source.min(TABLE_SPAN_MAX);
                emplace(&mut a, name, &span.to_string());
                if span != source {
                    emplace(&mut a, source_name, &source.to_string());
                }
            }
        }
        let mut result = self.html.push_tag(if cell.header { "th" } else { "td" }, a);
        if let Some(t) = &cell.text {
            result += &self.render_text(t);
        }
        result += &self.html.pop_tag();
        result
    }

    fn render_fallback(&mut self, kind: &str, label: &str) -> String {
        let mut result = self
            .html
            .push_tag("div", block_attributes("rich_fallback", kind));
        result += &escape(label);
        result += &self.html.pop_tag();
        result
    }

    fn render_blocks(&mut self, blocks: &[RichBlock]) -> String {
        let mut result = String::new();
        for b in blocks {
            result += &self.render_block(b);
        }
        result
    }

    fn render_block(&mut self, block: &RichBlock) -> String {
        use BlockKind as K;
        match block.kind {
            K::Unsupported => self.render_fallback("unsupported", "Unsupported rich content"),
            K::Heading => {
                let mut a = block_attributes("rich_heading", "heading");
                emplace(&mut a, "data-level", &block.heading_level.to_string());
                self.render_text_block(heading_tag(block.heading_level), a, &block.text)
            }
            K::Paragraph => {
                let mut a = block_attributes("rich_paragraph", "paragraph");
                emplace(&mut a, "dir", "auto");
                self.render_text_block("p", a, &block.text)
            }
            K::Footer => {
                let mut a = block_attributes("rich_footer", "footer");
                emplace(&mut a, "dir", "auto");
                self.render_text_block("footer", a, &block.text)
            }
            K::Thinking => {
                let mut a = block_attributes("rich_thinking", "thinking");
                emplace(&mut a, "dir", "auto");
                self.render_text_block("p", a, &block.text)
            }
            K::AuthorDate => self.render_author_date(block),
            K::Code => self.render_code(block),
            K::Divider => {
                let mut a = block_attributes("rich_divider", "divider");
                emplace(&mut a, "empty", "");
                self.html.push_tag("hr", a)
            }
            K::Anchor => {
                let Some(id) = self.block_anchor_id(block) else {
                    return self.render_fallback("malformed", "Malformed rich content");
                };
                let mut a = block_attributes("rich_anchor", "anchor");
                emplace(&mut a, "id", &id);
                emplace(&mut a, "inline", "");
                let mut result = self.html.push_tag("span", a);
                result += &self.html.pop_tag();
                result
            }
            K::List => self.render_list(block),
            K::Quote => self.render_quote(block),
            K::Photo => self.render_photo(block),
            K::Video => self.render_video(block),
            K::Cover => self.render_media_group(block, "cover", false),
            K::Embed => self.render_embed(block),
            K::EmbedPost => self.render_embed_post(block),
            K::Collage => self.render_collage(block),
            K::Slideshow => self.render_slideshow(block),
            K::Channel => self.render_channel(block),
            K::Audio => self.render_audio(block),
            K::File => self.render_file(block),
            K::Math => {
                let mut a = block_attributes("rich_math_display", "math");
                emplace(&mut a, "dir", "ltr");
                emplace(&mut a, "inline", "");
                let mut result = self.html.push_tag("div", a);
                result += &escape(&block.formula);
                result += &self.html.pop_tag();
                result
            }
            K::Table => self.render_table(block),
            K::Details => self.render_details(block),
            K::RelatedArticles => self.render_related_articles(block),
            K::Map => self.render_map(block, "map"),
            K::InputMap => self.render_map(block, "input-map"),
            K::ButtonRow => self.render_button_row(block),
            K::Unknown => self.render_fallback("unknown", "Unknown rich content"),
        }
    }
}
