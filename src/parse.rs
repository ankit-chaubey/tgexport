//! Turns raw TL messages into the export model: text entities, media,
//! service actions and file names.

use ferogram::tl;
use tl::enums as E;

use crate::model::*;

/// Counters for numbering files within one chat (photo_1, video_2, ...).
#[derive(Default)]
pub struct MediaCtx {
    /// Id of the account being exported.
    pub self_id: i64,
    pub photos: u32,
    pub audios: u32,
    pub videos: u32,
    pub files: u32,
    pub contacts: u32,
    /// Folder of the current chat inside the export, e.g. `chats/chat_01/`.
    pub folder: String,
}

/// Date suffix for file names, like `@17-06-2020_13-45-02`, in local time.
pub fn file_date_part(ts: i32) -> String {
    if ts == 0 {
        return String::new();
    }
    use chrono::{Local, TimeZone};
    match Local.timestamp_opt(ts as i64, 0).single() {
        Some(d) => format!("@{}", d.format("%d-%m-%Y_%H-%M-%S")),
        None => String::new(),
    }
}

/// Replaces characters that are not allowed in file names.
pub fn sanitize_file_name(name: &str) -> String {
    let mut out: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if (c as u32) < 32 => '_',
            c => c,
        })
        .collect();
    out = out.trim().trim_matches('.').to_string();
    if out.is_empty() {
        out = "file".into();
    }
    if out.len() > 200 {
        let mut cut = 200;
        while !out.is_char_boundary(cut) {
            cut -= 1;
        }
        out.truncate(cut);
    }
    out
}

pub fn parse_text(text: &str, entities: &[E::MessageEntity]) -> Text {
    if text.is_empty() {
        return Vec::new();
    }
    let units: Vec<u16> = text.encode_utf16().collect();
    // (offset, length, kind, extra data). Offsets are in UTF-16 units, as sent by Telegram.
    let mut list: Vec<(usize, usize, TextKind, String)> = Vec::new();
    for e in entities {
        let (o, l, k, add) = match e {
            E::MessageEntity::Mention(x) => (x.offset, x.length, TextKind::Mention, String::new()),
            E::MessageEntity::Hashtag(x) => (x.offset, x.length, TextKind::Hashtag, String::new()),
            E::MessageEntity::BotCommand(x) => {
                (x.offset, x.length, TextKind::BotCommand, String::new())
            }
            E::MessageEntity::Url(x) => (x.offset, x.length, TextKind::Url, String::new()),
            E::MessageEntity::Email(x) => (x.offset, x.length, TextKind::Email, String::new()),
            E::MessageEntity::Bold(x) => (x.offset, x.length, TextKind::Bold, String::new()),
            E::MessageEntity::Italic(x) => (x.offset, x.length, TextKind::Italic, String::new()),
            E::MessageEntity::Code(x) => (x.offset, x.length, TextKind::Code, String::new()),
            E::MessageEntity::Pre(x) => (x.offset, x.length, TextKind::Pre, x.language.clone()),
            E::MessageEntity::TextUrl(x) => (x.offset, x.length, TextKind::TextUrl, x.url.clone()),
            E::MessageEntity::MentionName(x) => (
                x.offset,
                x.length,
                TextKind::MentionName,
                x.user_id.to_string(),
            ),
            E::MessageEntity::Phone(x) => (x.offset, x.length, TextKind::Phone, String::new()),
            E::MessageEntity::Cashtag(x) => (x.offset, x.length, TextKind::Cashtag, String::new()),
            E::MessageEntity::Underline(x) => {
                (x.offset, x.length, TextKind::Underline, String::new())
            }
            E::MessageEntity::Strike(x) => (x.offset, x.length, TextKind::Strike, String::new()),
            E::MessageEntity::BankCard(x) => {
                (x.offset, x.length, TextKind::BankCard, String::new())
            }
            E::MessageEntity::Spoiler(x) => (x.offset, x.length, TextKind::Spoiler, String::new()),
            E::MessageEntity::CustomEmoji(x) => (
                x.offset,
                x.length,
                TextKind::CustomEmoji,
                x.document_id.to_string(),
            ),
            E::MessageEntity::Blockquote(x) => (
                x.offset,
                x.length,
                TextKind::Blockquote,
                if x.collapsed {
                    "1".into()
                } else {
                    String::new()
                },
            ),
            E::MessageEntity::TonAddress(x) => {
                (x.offset, x.length, TextKind::TonAddress, String::new())
            }
            E::MessageEntity::Unknown(x) => (x.offset, x.length, TextKind::Unknown, String::new()),
            _ => continue,
        };
        if o < 0 || l <= 0 {
            continue;
        }
        list.push((o as usize, l as usize, k, add));
    }
    list.sort_by_key(|e| e.0);

    let mut parts: Text = Vec::new();
    let mut cursor = 0usize;
    let slice = |a: usize, b: usize| -> String {
        let a = a.min(units.len());
        let b = b.min(units.len());
        String::from_utf16_lossy(&units[a..b])
    };
    for (o, l, k, add) in list {
        if o < cursor || o >= units.len() {
            continue; // overlaps the previous entity or is out of range
        }
        if o > cursor {
            parts.push(TextPart {
                kind: TextKind::Text,
                text: slice(cursor, o),
                additional: String::new(),
            });
        }
        let end = (o + l).min(units.len());
        parts.push(TextPart {
            kind: k,
            text: slice(o, end),
            additional: add,
        });
        cursor = end;
    }
    if cursor < units.len() {
        parts.push(TextPart {
            kind: TextKind::Text,
            text: slice(cursor, units.len()),
            additional: String::new(),
        });
    }
    parts
}

fn text_with_entities(t: &E::TextWithEntities) -> Text {
    let E::TextWithEntities::TextWithEntities(t) = t;
    parse_text(&t.text, &t.entities)
}

fn best_photo_size(sizes: &[E::PhotoSize]) -> (i32, i32, i64) {
    let mut best = (0i32, 0i32, 0i64);
    for s in sizes {
        let (w, h, size) = match s {
            E::PhotoSize::PhotoSize(x) => (x.w, x.h, x.size as i64),
            E::PhotoSize::PhotoCachedSize(x) => (x.w, x.h, x.bytes.len() as i64),
            E::PhotoSize::Progressive(x) => (x.w, x.h, x.sizes.last().copied().unwrap_or(0) as i64),
            _ => continue,
        };
        if (w as i64) * (h as i64) >= (best.0 as i64) * (best.1 as i64) {
            best = (w, h, size);
        }
    }
    best
}

pub fn photo_file(photo: &E::Photo, suggested: String) -> Option<(FileRef, i32, i32)> {
    match photo {
        E::Photo::Photo(p) => {
            let (w, h, size) = best_photo_size(&p.sizes);
            Some((
                FileRef {
                    source: Source::Photo(p.clone()),
                    class: MediaClass::Photo,
                    size,
                    suggested,
                    relative_path: String::new(),
                    skip: None,
                },
                w,
                h,
            ))
        }
        E::Photo::Empty(_) => None,
    }
}

fn doc_extension(mime: &str) -> String {
    let first = mime_guess::get_mime_extensions_str(mime).and_then(|l| l.first().copied());
    match (mime, first) {
        ("video/mp4", _) => "mp4".into(),
        ("audio/mpeg", _) | ("audio/mp3", _) => "mp3".into(),
        ("application/pdf", _) => "pdf".into(),
        ("image/jpeg", _) => "jpg".into(),
        ("application/x-tgsticker", _) => "tgs".into(),
        (_, Some(e)) => e.to_string(),
        _ => String::new(),
    }
}

fn doc_folder(f: &DocFlags) -> &'static str {
    if f.video_file {
        "video_files"
    } else if f.animated {
        "animations"
    } else if f.sticker {
        "stickers"
    } else if f.voice {
        "voice_messages"
    } else if f.video_message {
        "round_video_messages"
    } else {
        "files"
    }
}

pub struct DocInfo {
    pub file: FileRef,
    pub name: String,
    pub mime: String,
    pub duration: i32,
    pub width: i32,
    pub height: i32,
    pub flags: DocFlags,
    pub sticker_emoji: String,
    pub performer: String,
    pub title: String,
    pub thumb: Option<Thumb>,
}

/// The biggest real thumbnail of a document, saved next to it as `<name>_thumb.jpg`.
fn document_thumb(d: &tl::types::Document, path: &str) -> Option<Thumb> {
    let sizes = d.thumbs.as_ref()?;
    let mut best: Option<(i64, Thumb)> = None;
    for size in sizes {
        let (w, h, source, bytes) = match size {
            E::PhotoSize::PhotoSize(x) => (
                x.w,
                x.h,
                Source::DocumentThumb(d.clone(), x.r#type.clone()),
                x.size as i64,
            ),
            E::PhotoSize::PhotoCachedSize(x) => (
                x.w,
                x.h,
                Source::Bytes(x.bytes.clone()),
                x.bytes.len() as i64,
            ),
            E::PhotoSize::Progressive(x) => {
                let Some(last) = x.sizes.last() else { continue };
                (
                    x.w,
                    x.h,
                    Source::DocumentThumb(d.clone(), x.r#type.clone()),
                    *last as i64,
                )
            }
            _ => continue,
        };
        let area = w as i64 * h as i64;
        if area == 0 {
            continue;
        }
        if best.as_ref().map(|(a, _)| area >= *a).unwrap_or(true) {
            best = Some((
                area,
                Thumb {
                    file: FileRef {
                        source,
                        class: MediaClass::Always,
                        size: bytes,
                        suggested: format!("{path}_thumb.jpg"),
                        relative_path: String::new(),
                        skip: None,
                    },
                },
            ));
        }
    }
    best.map(|(_, t)| t)
}

pub fn parse_document(ctx: &mut MediaCtx, doc: &E::Document, date: i32) -> Option<DocInfo> {
    let E::Document::Document(d) = doc else {
        return None;
    };
    let mut info = DocInfo {
        file: FileRef::none(),
        name: String::new(),
        mime: d.mime_type.clone(),
        duration: 0,
        width: 0,
        height: 0,
        flags: DocFlags::default(),
        sticker_emoji: String::new(),
        performer: String::new(),
        title: String::new(),
        thumb: None,
    };
    for a in &d.attributes {
        match a {
            E::DocumentAttribute::ImageSize(x) => {
                info.width = x.w;
                info.height = x.h;
            }
            E::DocumentAttribute::Animated => info.flags.animated = true,
            E::DocumentAttribute::Sticker(x) => {
                info.flags.sticker = true;
                info.sticker_emoji = x.alt.clone();
            }
            E::DocumentAttribute::CustomEmoji(x) => {
                info.flags.sticker = true;
                info.sticker_emoji = x.alt.clone();
            }
            E::DocumentAttribute::Video(x) => {
                if x.round_message {
                    info.flags.video_message = true;
                } else {
                    info.flags.video_file = true;
                }
                info.width = x.w;
                info.height = x.h;
                info.duration = x.duration as i32;
            }
            E::DocumentAttribute::Audio(x) => {
                if x.voice {
                    info.flags.voice = true;
                } else {
                    info.flags.audio = true;
                }
                info.performer = x.performer.clone().unwrap_or_default();
                info.title = x.title.clone().unwrap_or_default();
                info.duration = x.duration;
            }
            E::DocumentAttribute::Filename(x) => info.name = x.file_name.clone(),
            _ => {}
        }
    }

    // Use the name Telegram gives us, otherwise make one up from the kind and date.
    let date_part = file_date_part(date);
    let file_name = if !info.name.is_empty() {
        info.name.clone()
    } else if info.flags.voice {
        ctx.audios += 1;
        let ext = if d.mime_type.eq_ignore_ascii_case("audio/mp3") {
            "mp3"
        } else {
            "ogg"
        };
        format!("audio_{}{}.{}", ctx.audios, date_part, ext)
    } else if info.flags.video_file {
        ctx.videos += 1;
        let ext = doc_extension(&d.mime_type);
        let ext = if ext.is_empty() {
            "mov".to_string()
        } else {
            ext
        };
        format!("video_{}{}.{}", ctx.videos, date_part, ext)
    } else {
        ctx.files += 1;
        let ext = doc_extension(&d.mime_type);
        let ext = if ext.is_empty() {
            "unknown".to_string()
        } else {
            ext
        };
        format!("file_{}{}.{}", ctx.files, date_part, ext)
    };

    let class = if info.flags.sticker {
        MediaClass::Sticker
    } else if info.flags.animated {
        MediaClass::Gif
    } else if info.flags.video_file {
        MediaClass::Video
    } else if info.flags.voice {
        MediaClass::VoiceMessage
    } else if info.flags.video_message {
        MediaClass::VideoMessage
    } else {
        MediaClass::File
    };

    info.file = FileRef {
        source: Source::Document(d.clone()),
        class,
        size: d.size,
        suggested: format!(
            "{}{}/{}",
            ctx.folder,
            doc_folder(&info.flags),
            sanitize_file_name(&file_name)
        ),
        relative_path: String::new(),
        skip: None,
    };
    info.thumb = document_thumb(d, &info.file.suggested);
    Some(info)
}

fn geo(g: &E::GeoPoint) -> Option<(f64, f64)> {
    match g {
        E::GeoPoint::GeoPoint(p)
            if (-90.0..=90.0).contains(&p.lat) && (-180.0..=180.0).contains(&p.long) =>
        {
            Some((p.lat, p.long))
        }
        _ => None,
    }
}

pub fn parse_media(ctx: &mut MediaCtx, media: &E::MessageMedia, date: i32, bot_id: i64) -> Media {
    match media {
        E::MessageMedia::Empty => Media::None,
        E::MessageMedia::Photo(m) => {
            let Some(photo) = &m.photo else {
                return Media::None;
            };
            ctx.photos += 1;
            let name = format!("photo_{}{}.jpg", ctx.photos, file_date_part(date));
            match photo_file(photo, format!("{}photos/{}", ctx.folder, name)) {
                Some((file, w, h)) => Media::Photo {
                    file,
                    width: w,
                    height: h,
                    spoiler: m.spoiler,
                    ttl: m.ttl_seconds.unwrap_or(0),
                },
                None => Media::None,
            }
        }
        E::MessageMedia::Document(m) => {
            let Some(doc) = &m.document else {
                return Media::None;
            };
            match parse_document(ctx, doc, date) {
                Some(i) => Media::Document {
                    file: i.file,
                    name: i.name,
                    mime: i.mime,
                    duration: i.duration,
                    width: i.width,
                    height: i.height,
                    flags: i.flags,
                    sticker_emoji: i.sticker_emoji,
                    performer: i.performer,
                    title: i.title,
                    spoiler: m.spoiler,
                    ttl: m.ttl_seconds.unwrap_or(0),
                    thumb: i.thumb,
                },
                None => Media::None,
            }
        }
        E::MessageMedia::Geo(m) => Media::Geo {
            point: geo(&m.geo),
            live_period: 0,
        },
        E::MessageMedia::GeoLive(m) => Media::Geo {
            point: geo(&m.geo),
            live_period: m.period,
        },
        E::MessageMedia::Venue(m) => Media::Venue {
            point: geo(&m.geo),
            title: m.title.clone(),
            address: m.address.clone(),
        },
        E::MessageMedia::Contact(m) => {
            let vcard = if m.vcard.is_empty() {
                None
            } else {
                ctx.contacts += 1;
                Some(FileRef {
                    source: Source::Bytes(m.vcard.clone().into_bytes()),
                    class: MediaClass::Always,
                    size: m.vcard.len() as i64,
                    suggested: format!("{}contacts/contact_{}.vcard", ctx.folder, ctx.contacts),
                    relative_path: String::new(),
                    skip: None,
                })
            };
            Media::Contact {
                first_name: m.first_name.clone(),
                last_name: m.last_name.clone(),
                phone: m.phone_number.clone(),
                vcard,
            }
        }
        E::MessageMedia::Game(m) => {
            let E::Game::Game(g) = &m.game;
            Media::Game {
                title: g.title.clone(),
                description: g.description.clone(),
                bot_id,
                short_name: g.short_name.clone(),
            }
        }
        E::MessageMedia::Invoice(m) => Media::Invoice {
            title: m.title.clone(),
            description: m.description.clone(),
            amount: m.total_amount,
            currency: m.currency.clone(),
            receipt_msg_id: m.receipt_msg_id.unwrap_or(0),
        },
        E::MessageMedia::Poll(m) => {
            let E::Poll::Poll(p) = &m.poll;
            let E::PollResults::PollResults(r) = &m.results;
            let mut answers = Vec::new();
            for a in &p.answers {
                let E::PollAnswer::PollAnswer(a) = a else {
                    continue;
                };
                let mut votes = 0;
                let mut chosen = false;
                if let Some(list) = &r.results {
                    for v in list {
                        let E::PollAnswerVoters::PollAnswerVoters(v) = v;
                        if v.option == a.option {
                            votes = v.voters.unwrap_or(0);
                            chosen = v.chosen;
                        }
                    }
                }
                answers.push((text_with_entities(&a.text), votes, chosen));
            }
            Media::Poll(Poll {
                question: text_with_entities(&p.question),
                closed: p.closed,
                total_votes: r.total_voters.unwrap_or(0),
                answers,
            })
        }
        E::MessageMedia::WebPage(_) => Media::None,
        E::MessageMedia::Unsupported => Media::Unsupported,
        _ => Media::None,
    }
}

fn variant_name<T: std::fmt::Debug>(v: &T) -> String {
    let s = format!("{v:?}");
    s.chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect()
}

pub fn snake_case(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

pub use crate::actions::parse_action;

fn parse_buttons(markup: &E::ReplyMarkup) -> Vec<Vec<Button>> {
    let E::ReplyMarkup::ReplyInlineMarkup(m) = markup else {
        return Vec::new();
    };
    let mut rows = Vec::new();
    for row in &m.rows {
        let E::KeyboardInlineButtonRow::KeyboardInlineButtonRow(row) = row;
        let mut out = Vec::new();
        for b in &row.buttons {
            let E::KeyboardInlineButton::KeyboardInlineButton(b) = b;
            let (kind, data, url) = match &b.r#type {
                E::InlineButtonType::Url(x) => ("url", Vec::new(), x.url.clone()),
                E::InlineButtonType::UrlAuth(x) => ("url_auth", Vec::new(), x.url.clone()),
                E::InlineButtonType::WebView(x) => ("webview", Vec::new(), x.url.clone()),
                E::InlineButtonType::Callback(x) => ("callback", x.data.clone(), String::new()),
                E::InlineButtonType::Game => ("game", Vec::new(), String::new()),
                E::InlineButtonType::Buy => ("buy", Vec::new(), String::new()),
                E::InlineButtonType::SwitchInline(x) => {
                    ("switch_inline", Vec::new(), x.query.clone())
                }
                other => ("unknown", Vec::new(), variant_name(other)),
            };
            out.push(Button {
                kind: kind.to_string(),
                text: b.text.clone(),
                data,
                url,
            });
        }
        if !out.is_empty() {
            rows.push(out);
        }
    }
    rows
}

fn parse_reactions(r: &E::MessageReactions) -> Vec<Reaction> {
    let E::MessageReactions::MessageReactions(r) = r;
    let recents = r.recent_reactions.as_deref().unwrap_or(&[]);
    let mut out = Vec::new();
    for c in &r.results {
        let E::ReactionCount::ReactionCount(c) = c;
        let (emoji, custom) = match &c.reaction {
            E::Reaction::Emoji(e) => (e.emoticon.clone(), None),
            E::Reaction::CustomEmoji(e) => (String::new(), Some(e.document_id.to_string())),
            _ => continue,
        };
        let recent = recents
            .iter()
            .filter_map(|E::MessagePeerReaction::MessagePeerReaction(x)| {
                let same = match (&x.reaction, &c.reaction) {
                    (E::Reaction::Emoji(a), E::Reaction::Emoji(b)) => a.emoticon == b.emoticon,
                    (E::Reaction::CustomEmoji(a), E::Reaction::CustomEmoji(b)) => {
                        a.document_id == b.document_id
                    }
                    _ => false,
                };
                same.then(|| (peer_from_tl(&x.peer_id), x.date))
            })
            .collect();
        out.push(Reaction {
            emoji,
            custom,
            count: c.count,
            recent,
        });
    }
    out
}

pub fn parse_message(ctx: &mut MediaCtx, msg: &E::Message) -> Option<Message> {
    match msg {
        E::Message::Empty(_) => None,
        E::Message::Service(m) => {
            let action = parse_action(ctx, &m.action, m.date);
            let reply = match &m.reply_to {
                Some(E::MessageReplyHeader::MessageReplyHeader(h)) => (
                    h.reply_to_msg_id.unwrap_or(0),
                    h.reply_to_peer_id.as_ref().map(peer_from_tl),
                ),
                _ => (0, None),
            };
            Some(Message {
                id: m.id,
                out: m.out,
                date: m.date,
                edited: 0,
                from: m
                    .from_id
                    .as_ref()
                    .map(peer_from_tl)
                    .or(Some(peer_from_tl(&m.peer_id))),
                signature: String::new(),
                fwd_from: None,
                fwd_name: String::new(),
                fwd_date: 0,
                forwarded: false,
                saved_from: None,
                reply_to: reply.0,
                reply_to_peer: reply.1,
                via_bot: None,
                text: Vec::new(),
                media: Media::None,
                action,
                buttons: Vec::new(),
                reactions: m
                    .reactions
                    .as_ref()
                    .map(parse_reactions)
                    .unwrap_or_default(),
                rich: None,
                unsupported: false,
            })
        }
        E::Message::Message(m) => {
            // Games link to the bot that sent them: the bot it came through,
            // else the original author, else the sender.
            let bot_id = m
                .via_bot_id
                .or(match m.fwd_from.as_ref() {
                    Some(E::MessageFwdHeader::MessageFwdHeader(f)) => {
                        match f.from_id.as_ref().map(peer_from_tl) {
                            Some(PeerId::User(u)) => Some(u),
                            _ => None,
                        }
                    }
                    None => None,
                })
                .or(match m.from_id.as_ref().map(peer_from_tl) {
                    Some(PeerId::User(u)) => Some(u),
                    _ => None,
                })
                .unwrap_or(0);
            let mut media = m
                .media
                .as_ref()
                .map(|x| parse_media(ctx, x, m.date, bot_id))
                .unwrap_or(Media::None);
            // Expiring media of other people is not available to export.
            match &mut media {
                Media::Photo { file, ttl, .. } if *ttl != 0 && !m.out => *file = FileRef::none(),
                Media::Document {
                    file, ttl, thumb, ..
                } if *ttl != 0 && !m.out => {
                    *file = FileRef::none();
                    if let Some(t) = thumb {
                        t.file = FileRef::none();
                    }
                }
                _ => {}
            }
            let (mut fwd_from, mut fwd_name, mut fwd_date, mut forwarded, mut saved_from) =
                (None, String::new(), 0, false, None);
            if let Some(E::MessageFwdHeader::MessageFwdHeader(f)) = &m.fwd_from {
                forwarded = true;
                fwd_from = f.from_id.as_ref().map(peer_from_tl);
                fwd_name = f.from_name.clone().unwrap_or_default();
                fwd_date = f.date;
                saved_from = f.saved_from_peer.as_ref().map(peer_from_tl);
            }
            let reply = match &m.reply_to {
                Some(E::MessageReplyHeader::MessageReplyHeader(h)) => (
                    h.reply_to_msg_id.unwrap_or(0),
                    h.reply_to_peer_id.as_ref().map(peer_from_tl),
                ),
                _ => (0, None),
            };
            let text = parse_text(&m.message, m.entities.as_deref().unwrap_or(&[]));
            let reactions = m
                .reactions
                .as_ref()
                .map(parse_reactions)
                .unwrap_or_default();
            let rich = m
                .rich_message
                .as_ref()
                .map(|r| crate::rich::parse_rich_message(ctx, r, m.date));
            Some(Message {
                id: m.id,
                out: m.out,
                date: m.date,
                edited: m.edit_date.unwrap_or(0),
                from: m
                    .from_id
                    .as_ref()
                    .map(peer_from_tl)
                    .or(Some(peer_from_tl(&m.peer_id))),
                signature: m.post_author.clone().unwrap_or_default(),
                fwd_from,
                fwd_name,
                fwd_date,
                forwarded,
                saved_from,
                reply_to: reply.0,
                reply_to_peer: reply.1,
                via_bot: m.via_bot_id,
                unsupported: matches!(media, Media::Unsupported) && rich.is_none(),
                text,
                media,
                action: None,
                buttons: m
                    .reply_markup
                    .as_ref()
                    .map(parse_buttons)
                    .unwrap_or_default(),
                reactions,
                rich,
            })
        }
    }
}

/// A story from the archive. Only photos and videos are exported.
pub fn parse_story(index: u32, item: &E::StoryItem) -> Option<Story> {
    let E::StoryItem::StoryItem(st) = item else {
        return None;
    };
    let date = file_date_part(st.date);
    let (file, ok) = match &st.media {
        E::MessageMedia::Photo(m) => match m
            .photo
            .as_ref()
            .and_then(|p| photo_file(p, format!("stories/story_{index}{date}.jpg")))
        {
            Some((f, _, _)) => (f, true),
            None => (FileRef::none(), false),
        },
        E::MessageMedia::Document(m) => {
            let mut ctx = MediaCtx::default();
            match m
                .document
                .as_ref()
                .and_then(|d| parse_document(&mut ctx, d, st.date))
            {
                Some(info) => {
                    let mut f = info.file;
                    let ext = match info.mime.as_str() {
                        "image/jpeg" => "jpg".to_string(),
                        "image/png" => "png".to_string(),
                        mime => doc_extension(mime),
                    };
                    let ext = if ext.is_empty() {
                        String::new()
                    } else {
                        format!(".{ext}")
                    };
                    f.suggested = format!("stories/story_{index}{date}{ext}");
                    (f, true)
                }
                None => (FileRef::none(), false),
            }
        }
        _ => (FileRef::none(), false),
    };
    if !ok {
        return None;
    }
    let mut file = file;
    file.class = MediaClass::Always;
    Some(Story {
        id: st.id,
        date: st.date,
        expires: st.expire_date,
        pinned: st.pinned,
        file,
    })
}

/// A saved profile song, shaped like a message so the writers can reuse the
/// audio rendering. Anything that is not an audio file is dropped.
pub fn parse_music(index: u32, doc: &E::Document) -> Option<Message> {
    let mut ctx = MediaCtx::default();
    let info = parse_document(&mut ctx, doc, 0)?;
    if !info.flags.audio {
        return None;
    }
    let mut file = info.file;
    let name = file
        .suggested
        .rsplit('/')
        .next()
        .unwrap_or("audio")
        .to_string();
    file.suggested = format!("profile_music/{name}");
    file.class = MediaClass::Always;
    Some(Message {
        id: index as i32,
        out: false,
        date: 0,
        edited: 0,
        from: None,
        signature: String::new(),
        fwd_from: None,
        fwd_name: String::new(),
        fwd_date: 0,
        forwarded: false,
        saved_from: None,
        reply_to: 0,
        reply_to_peer: None,
        via_bot: None,
        text: Vec::new(),
        media: Media::Document {
            file,
            name: info.name,
            mime: info.mime,
            duration: info.duration,
            width: info.width,
            height: info.height,
            flags: info.flags,
            sticker_emoji: info.sticker_emoji,
            performer: info.performer,
            title: info.title,
            spoiler: false,
            ttl: 0,
            thumb: info.thumb,
        },
        action: None,
        buttons: Vec::new(),
        reactions: Vec::new(),
        rich: None,
        unsupported: false,
    })
}
