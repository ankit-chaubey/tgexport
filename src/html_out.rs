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

//! HTML writer. The markup and class names match the stylesheet and script
//! taken from Telegram Desktop, which are copied into the export folder.

use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use chrono::{Datelike, Local, TimeZone, Timelike};

use crate::about;
use crate::assets;
use crate::model::*;
use crate::output::Writer;
use crate::phone::format_phone;
use crate::thumbs;

const MESSAGES_IN_FILE: usize = 1000;
const JOIN_WITHIN_SECONDS: i32 = 900;
const HISTORY_USERPIC: u32 = 42;
const ENTRY_USERPIC: u32 = 48;
const PERSONAL_USERPIC: u32 = 90;
const STORY_THUMB_WIDTH: u32 = 45;
const STORY_THUMB_HEIGHT: u32 = 80;
const SAVED_MESSAGES_COLOR: u8 = 3;

const CHATS_PRIORITY: i32 = 0;
const CONTACTS_PRIORITY: i32 = 2;
const FREQUENT_PRIORITY: i32 = 3;
const USERPICS_PRIORITY: i32 = 4;
const STORIES_PRIORITY: i32 = 5;
const PROFILE_MUSIC_PRIORITY: i32 = 6;
const SESSIONS_PRIORITY: i32 = 7;
const WEB_SESSIONS_PRIORITY: i32 = 8;
const OTHER_PRIORITY: i32 = 9;

pub fn esc(s: &str) -> String {
    crate::rich_html::escape(s)
}

fn esc_br(s: &str) -> String {
    esc(s).replace('\n', "<br>")
}

fn local(ts: i32) -> Option<chrono::DateTime<Local>> {
    Local.timestamp_opt(ts as i64, 0).single()
}

pub fn date_text(ts: i32) -> String {
    const M: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    match local(ts) {
        Some(d) => format!("{} {} {}", d.day(), M[d.month0() as usize], d.year()),
        None => String::new(),
    }
}

pub fn time_text(ts: i32) -> String {
    match local(ts) {
        Some(d) => format!("{:02}:{:02}", d.hour(), d.minute()),
        None => String::new(),
    }
}

/// `DD.MM.YYYY HH:MM:SS`, with the name of the time zone offset when asked.
pub fn date_time(ts: i32, with_zone: bool) -> String {
    let Some(d) = local(ts) else {
        return String::new();
    };
    if ts == 0 {
        return String::new();
    }
    let mut out = d.format("%d.%m.%Y %H:%M:%S").to_string();
    if with_zone {
        let offset = d.offset().local_minus_utc();
        out.push(' ');
        if offset == 0 {
            out.push_str("UTC");
        } else {
            let sign = if offset < 0 { '-' } else { '+' };
            let abs = offset.abs();
            out.push_str(&format!(
                "UTC{sign}{:02}:{:02}",
                abs / 3600,
                (abs % 3600) / 60
            ));
        }
    }
    out
}

fn day_key(ts: i32) -> i32 {
    local(ts).map(|d| d.num_days_from_ce()).unwrap_or(0)
}

fn file_size(n: i64) -> String {
    let n = n.max(0) as f64;
    if n < 1024.0 {
        format!("{} bytes", n as i64)
    } else if n < 1024.0 * 1024.0 {
        format!("{:.1} KB", n / 1024.0)
    } else if n < 1024.0 * 1024.0 * 1024.0 {
        format!("{:.1} MB", n / 1024.0 / 1024.0)
    } else {
        format!("{:.2} GB", n / 1024.0 / 1024.0 / 1024.0)
    }
}

fn duration(sec: i32) -> String {
    let (h, m, s) = (sec / 3600, (sec / 60) % 60, sec % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

fn first_char(s: &str) -> String {
    s.trim()
        .chars()
        .next()
        .map(|c| esc(&c.to_string()))
        .unwrap_or_default()
}

/// Relative prefix that leads from a page back to the export folder.
fn base_for(depth: usize) -> String {
    "../".repeat(depth)
}

fn page_start(base: &str) -> String {
    format!(
        "<!DOCTYPE html>\n<html>\n<head>\n<meta charset=\"utf-8\"/>\n<title>Exported Data</title>\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\"/>\n\
         <link href=\"{base}css/style.css\" rel=\"stylesheet\"/>\n\
         <script src=\"{base}js/script.js\" type=\"text/javascript\">\n</script>\n</head>\n\
         <body onload=\"CheckLocation();\">\n<div class=\"page_wrap\">\n"
    )
}

const PAGE_END: &str = "</div>\n</div>\n</div>\n</body>\n</html>\n";

fn header(title: &str, back: Option<&str>) -> String {
    match back {
        Some(p) => format!(
            "<div class=\"page_header\">\n<a class=\"content block_link\" href=\"{}\" onclick=\"return GoBack(this)\">\n\
             <div class=\"text bold\">\n{}\n</div>\n</a>\n</div>\n",
            esc(p),
            esc(title)
        ),
        None => format!(
            "<div class=\"page_header\">\n<div class=\"content\">\n<div class=\"text bold\">\n{}\n</div>\n</div>\n</div>\n",
            esc(title)
        ),
    }
}

struct Userpic {
    color: u8,
    size: u32,
    first: String,
    last: String,
    tooltip: String,
}

fn userpic_html(u: &Userpic) -> String {
    let title = if u.tooltip.is_empty() {
        String::new()
    } else {
        format!(" title=\"{}\"", esc(&u.tooltip))
    };
    format!(
        "<div class=\"userpic userpic{color}\" style=\"width: {size}px; height: {size}px\">\n<div class=\"initials\" style=\"line-height: {size}px\"{title}>\n{f}{l}\n</div>\n</div>\n",
        color = u.color as u32 + 1,
        size = u.size,
        title = title,
        f = first_char(&u.first),
        l = first_char(&u.last),
    )
}

fn peer_userpic(id: Option<PeerId>, peers: &Peers, size: u32, fallback_color: u8) -> Userpic {
    match id.and_then(|i| peers.get(i).map(|p| (i, p))) {
        Some((i, p)) => {
            if p.is_chat {
                Userpic {
                    color: i.color_index(),
                    size,
                    first: p.title.clone(),
                    last: String::new(),
                    tooltip: String::new(),
                }
            } else {
                Userpic {
                    color: i.color_index(),
                    size,
                    first: p.first_name.clone(),
                    last: p.last_name.clone(),
                    tooltip: String::new(),
                }
            }
        }
        None => Userpic {
            color: id.map(|i| i.color_index()).unwrap_or(fallback_color),
            size,
            first: String::new(),
            last: String::new(),
            tooltip: String::new(),
        },
    }
}

/// Link around the text of a custom emoji. When the file was saved it opens
/// the file, otherwise a short message explains why it is missing.
fn custom_emoji_link(path: &str, text: &str, base: &str) -> String {
    let open = if path.is_empty() {
        "<a href=\"\" onclick=\"return ShowNotLoadedEmoji();\">".to_string()
    } else if path == "(unavailable)" {
        "<a href=\"\" onclick=\"return ShowNotAvailableEmoji();\">".to_string()
    } else {
        format!("<a href = \"{}\">", esc(&format!("{base}{path}")))
    };
    format!("{open}{text}</a>")
}

pub fn format_text(parts: &Text, base: &str) -> String {
    const INTERNAL_LINKS_DOMAIN: &str = "https://t.me/";
    let mut out = String::new();
    for p in parts {
        let text = esc(&p.text);
        // `text.mid(1)`: the escaped text without its first character.
        let rest: &str = text.get(1..).unwrap_or("");
        let quoted = || esc(&format!("\"{rest}\""));
        let piece = match p.kind {
            TextKind::Text | TextKind::Unknown => text.clone(),
            TextKind::Mention => {
                format!("<a href=\"{INTERNAL_LINKS_DOMAIN}{rest}\">{text}</a>")
            }
            TextKind::Hashtag => format!(
                "<a href=\"\" onclick=\"return ShowHashtag({})\">{text}</a>",
                quoted()
            ),
            TextKind::BotCommand => format!(
                "<a href=\"\" onclick=\"return ShowBotCommand({})\">{text}</a>",
                quoted()
            ),
            TextKind::Url => text_link(&text, &p.text),
            TextKind::Email => format!("<a href=\"mailto:{text}\">{text}</a>"),
            TextKind::Bold => format!("<strong>{text}</strong>"),
            TextKind::Italic => format!("<em>{text}</em>"),
            TextKind::Code => format!("<code>{text}</code>"),
            TextKind::Pre => format!("<pre>{text}</pre>"),
            TextKind::TextUrl => text_link(&text, &p.additional),
            TextKind::MentionName => {
                format!("<a href=\"\" onclick=\"return ShowMentionName()\">{text}</a>")
            }
            TextKind::Phone => format!("<a href=\"tel:{text}\">{text}</a>"),
            TextKind::Cashtag => format!(
                "<a href=\"\" onclick=\"return ShowCashtag({})\">{text}</a>",
                quoted()
            ),
            TextKind::Underline => format!("<u>{text}</u>"),
            TextKind::Strike => format!("<s>{text}</s>"),
            TextKind::Blockquote => format!("<blockquote>{text}</blockquote>"),
            TextKind::BankCard | TextKind::TonAddress => text.clone(),
            TextKind::Spoiler => format!(
                "<span class=\"spoiler hidden\" onclick=\"ShowSpoiler(this)\"><span aria-hidden=\"true\">{text}</span></span>"
            ),
            TextKind::CustomEmoji => custom_emoji_link(&p.additional, &text, base),
        };
        out.push_str(&piece);
    }
    out
}

/// A link only when the target passes the safety checks.
fn text_link(text: &str, target: &str) -> String {
    match crate::rich_html::safe_message_href(target) {
        Some(href) => format!("<a href=\"{}\">{text}</a>", esc(&href)),
        None => text.to_string(),
    }
}

const SERVICE_PHOTO_SIZE: u32 = 60;

/// A service message, with a round picture next to it for photo changes.
fn service_block(
    id: i64,
    text: &str,
    photo: Option<&FileRef>,
    dialog: Option<&DialogInfo>,
    root: &Path,
    base: &str,
) -> String {
    let mut o = format!(
        "<div class=\"message service\" id=\"message{id}\">\n<div class=\"body details\">\n{text}\n</div>\n"
    );
    if let (Some(file), Some(d)) = (photo, dialog) {
        let large = file.relative_path.clone();
        let side = (SERVICE_PHOTO_SIZE * 2) as i32;
        let image =
            crate::thumbs::write_image_thumb(root, &large, &|_| Some((side, side)), None, "_thumb")
                .map(|(p, _)| p)
                .unwrap_or_default();
        o.push_str("<div class=\"userpic_wrap\">\n");
        let size = format!("width: {0}px; height: {0}px", SERVICE_PHOTO_SIZE);
        if !large.is_empty() {
            o.push_str(&format!(
                "<a class=\"userpic_link\" href=\"{}\">\n",
                esc(&format!("{base}{large}"))
            ));
        }
        if !image.is_empty() {
            o.push_str(&format!(
                "<img class=\"userpic\" src=\"{}\" style=\"{size}\"/>\n",
                esc(&format!("{base}{image}"))
            ));
        } else {
            o.push_str(&userpic_html(&Userpic {
                color: d.peer.color_index(),
                size: SERVICE_PHOTO_SIZE,
                first: d.name.clone(),
                last: String::new(),
                tooltip: String::new(),
            }));
        }
        if !large.is_empty() {
            o.push_str("</a>\n");
        }
        o.push_str("</div>\n");
    }
    o.push_str("</div>\n");
    o
}

/// What the next message needs to know about the one before it.
#[derive(Clone)]
struct Prev {
    default: bool,
    from: Option<PeerId>,
    via: i64,
    date: i32,
    forwarded: bool,
    show_forwarded_as_original: bool,
    fwd_from: Option<PeerId>,
    fwd_name: String,
    fwd_date: i32,
}

/// Whether a message starts a new group (with picture and name) or joins the
/// previous one.
fn needs_wrap(m: &Message, prev: Option<&Prev>) -> bool {
    let Some(p) = prev else { return true };
    if !p.default {
        return true;
    }
    if m.from.is_none() || p.from != m.from {
        return true;
    }
    if m.via_bot.unwrap_or(0) != p.via {
        return true;
    }
    if day_key(p.date) != day_key(m.date) {
        return true;
    }
    if m.forwarded != p.forwarded
        || m.show_forwarded_as_original != p.show_forwarded_as_original
        || m.fwd_from != p.fwd_from
        || m.fwd_name != p.fwd_name
    {
        return true;
    }
    let limit = if m.fwd_from.is_some() || !m.fwd_name.is_empty() {
        1
    } else {
        JOIN_WITHIN_SECONDS
    };
    (m.date - p.date).abs() > limit
}

fn forwarded_needs_wrap(m: &Message, prev: Option<&Prev>) -> bool {
    if needs_wrap(m, prev) {
        return true;
    }
    let Some(p) = prev else { return true };
    match m.fwd_from {
        Some(f) if Some(f) == p.fwd_from => {
            !matches!(f, PeerId::User(_)) || (m.fwd_date - p.fwd_date).abs() > JOIN_WITHIN_SECONDS
        }
        _ => true,
    }
}

/// A link to another message of the chat. Messages in files that are already
/// finished are reached through the file that holds them.
fn wrap_message_link(id: i32, text: &str, last_ids: &[i32]) -> String {
    match last_ids.iter().position(|max| id <= *max) {
        None => format!(
            "<a href=\"#go_to_message{id}\" onclick=\"return GoToMessage({id})\">{text}</a>"
        ),
        Some(index) => format!(
            "<a href=\"{}#go_to_message{id}\">{text}</a>",
            messages_file(index)
        ),
    }
}

/// First and last name shown in a placeholder picture, from a plain name.
fn names_from_text(full: &str) -> (String, String) {
    let full = full.trim();
    match full.find(' ') {
        Some(i) => (full[..i].to_string(), full[i + 1..].trim().to_string()),
        None => (full.to_string(), String::new()),
    }
}

fn compose_name(u: &Userpic, empty: &str) -> String {
    if u.first.is_empty() && u.last.is_empty() {
        empty.to_string()
    } else {
        format!("{} {}", u.first, u.last)
    }
}

fn message_block(
    m: &Message,
    prev: Option<&Prev>,
    dialog: &DialogInfo,
    peers: &Peers,
    root: &Path,
    base: &str,
    last_ids: &[i32],
) -> (String, Prev) {
    let info = |default: bool| Prev {
        default,
        from: m.from,
        via: m.via_bot.unwrap_or(0),
        date: m.date,
        forwarded: m.forwarded,
        show_forwarded_as_original: m.show_forwarded_as_original,
        fwd_from: m.fwd_from,
        fwd_name: m.fwd_name.clone(),
        fwd_date: m.fwd_date,
    };
    if m.unsupported {
        let t = "This message is not supported by this version of Telegram Desktop. Please update the application.";
        return (
            service_block(m.id as i64, t, None, None, root, base),
            info(false),
        );
    }
    if let Some(a) = &m.action {
        let link = |id: i32, text: &str| wrap_message_link(id, text, last_ids);
        let text = crate::actions_html::service_text(m, a, dialog, peers, base, &link);
        if !text.is_empty() {
            let photo = match a {
                Action::ChatEditPhoto { photo, .. } | Action::SuggestProfilePhoto { photo, .. } => {
                    Some(photo)
                }
                _ => None,
            };
            return (
                service_block(m.id as i64, &text, photo, Some(dialog), root, base),
                info(false),
            );
        }
    }

    let wrap = needs_wrap(m, prev);
    let show_forwarded_info = m.forwarded && !m.show_forwarded_as_original;

    let mut forwarded_userpic = Userpic {
        color: 0,
        size: HISTORY_USERPIC,
        first: String::new(),
        last: String::new(),
        tooltip: String::new(),
    };
    if m.forwarded {
        forwarded_userpic = match m.fwd_from {
            Some(id) => peer_userpic(Some(id), peers, HISTORY_USERPIC, id.color_index()),
            None => {
                let (first, last) = names_from_text(&m.fwd_name);
                Userpic {
                    color: color_index_for(m.id as i64),
                    size: HISTORY_USERPIC,
                    first,
                    last,
                    tooltip: String::new(),
                }
            }
        };
    }
    let userpic = if m.show_forwarded_as_original {
        Userpic {
            color: forwarded_userpic.color,
            size: forwarded_userpic.size,
            first: forwarded_userpic.first.clone(),
            last: forwarded_userpic.last.clone(),
            tooltip: String::new(),
        }
    } else {
        let id = m.from.unwrap_or(dialog.peer);
        peer_userpic(Some(id), peers, HISTORY_USERPIC, id.color_index())
    };
    let via = m
        .via_bot
        .and_then(|b| peers.get(PeerId::User(b)))
        .map(|p| p.username.clone())
        .filter(|u| !u.is_empty())
        .map(|u| esc(&u));

    let mut o = format!(
        "<div class=\"message default clearfix{}\" id=\"message{}\">\n",
        if wrap { "" } else { " joined" },
        m.id
    );
    if wrap {
        o.push_str("<div class=\"pull_left userpic_wrap\">\n");
        o.push_str(&userpic_html(&userpic));
        o.push_str("</div>\n");
    }
    o.push_str("<div class=\"body\">\n");
    o.push_str(&format!(
        "<div class=\"pull_right date details\" title=\"{}\">\n{}\n</div>\n",
        esc(&date_time(m.date, true)),
        time_text(m.date)
    ));
    if wrap {
        o.push_str("<div class=\"from_name\">\n");
        o.push_str(&esc(&compose_name(&userpic, "Deleted Account")));
        if let Some(v) = &via {
            if !m.forwarded || m.show_forwarded_as_original {
                o.push_str(&format!(" via @{v}"));
            }
        }
        o.push_str("\n</div>\n");
    }
    if show_forwarded_info {
        let fwrap = forwarded_needs_wrap(m, prev);
        if fwrap {
            o.push_str("<div class=\"pull_left forwarded userpic_wrap\">\n");
            o.push_str(&userpic_html(&forwarded_userpic));
            o.push_str("</div>\n");
        }
        o.push_str("<div class=\"forwarded body\">\n");
        if fwrap {
            o.push_str("<div class=\"from_name\">\n");
            o.push_str(&esc(&compose_name(&forwarded_userpic, "Deleted Account")));
            if let Some(v) = &via {
                o.push_str(&format!(" via @{v}"));
            }
            o.push_str(&format!(
                "<span class=\"date details\" title=\"{}\"> {}</span>\n</div>\n",
                esc(&date_time(m.fwd_date, true)),
                date_time(m.fwd_date, false)
            ));
        }
    }
    if m.reply_to != 0 {
        o.push_str("<div class=\"reply_to details\">\n");
        if m.reply_to_peer.is_some() {
            o.push_str("In reply to a message in another chat");
        } else {
            o.push_str("In reply to ");
            o.push_str(&wrap_message_link(m.reply_to, "this message", last_ids));
        }
        o.push_str("\n</div>\n");
    }
    // Open tags around the message at this point: the page, its body, the
    // history, the message and its body, plus the forwarded body.
    let depth = if show_forwarded_info { 6 } else { 5 };
    let link = |id: i32, text: &str| wrap_message_link(id, text, last_ids);
    o.push_str(&crate::media_html::message_media(
        m, dialog, peers, root, base, depth, &link,
    ));
    if let Some(rich) = &m.rich {
        let page = crate::media_html::MediaPage { root, base };
        let mut renderer = crate::rich_html::Renderer::new(depth, rich, m.id, base, &page);
        o.push_str(&renderer.render_message());
    } else {
        let text = format_text(&m.text, base);
        if !text.is_empty() {
            o.push_str(&format!("<div class=\"text\">\n{text}\n</div>\n"));
        }
    }
    if !m.buttons.is_empty() {
        o.push_str("<table class=\"bot_buttons_table\">\n<tbody>\n");
        for row in &m.buttons {
            o.push_str("<tr>\n<td class=\"bot_button_row\">\n");
            for (i, b) in row.iter().enumerate() {
                let data = String::from_utf8_lossy(&b.data).into_owned();
                let mut content = String::new();
                if !data.is_empty() {
                    content.push_str(&format!("Data: {data} | "));
                }
                if !b.forward_text.is_empty() {
                    content.push_str(&format!("Forward text: {} | ", b.forward_text));
                }
                content.push_str(&format!("Type: {}", b.kind.name()));
                let href = if b.kind == ButtonKind::Url {
                    crate::rich_html::safe_message_href(&data)
                } else {
                    None
                };
                let onclick = if b.kind != ButtonKind::Url {
                    Some(format!(
                        "return ShowTextCopied('{}');",
                        content.replace('\\', "\\\\").replace('\'', "\\'")
                    ))
                } else {
                    None
                };
                o.push_str("<div class=\"bot_button\">\n<a");
                if let Some(h) = &href {
                    o.push_str(&format!(" href=\"{}\"", esc(h)));
                }
                if let Some(c) = &onclick {
                    o.push_str(&format!(" onclick=\"{}\"", esc(c)));
                }
                o.push_str(">\n");
                o.push_str(&format!("<div>\n{}\n</div>\n</a>\n</div>\n", esc(&b.text)));
                if i + 1 != row.len() {
                    o.push_str("<div class=\"bot_button_column_separator\">\n</div>\n");
                }
            }
            o.push_str("</td>\n</tr>\n");
        }
        o.push_str("</tbody>\n</table>\n");
    }
    if !m.signature.is_empty() {
        o.push_str(&format!(
            "<div class=\"signature details\">\n{}\n</div>\n",
            esc(&m.signature)
        ));
    }
    if show_forwarded_info {
        o.push_str("</div>\n");
    }
    if !m.reactions.is_empty() {
        o.push_str("<span class=\"reactions\">\n");
        for r in &m.reactions {
            let mut class = String::from("reaction");
            if r.recent
                .iter()
                .any(|(who, _)| peers.get(*who).map(|p| p.is_self).unwrap_or(false))
            {
                class.push_str(" active");
            }
            if r.kind == ReactionKind::Paid {
                class.push_str(" paid");
            }
            o.push_str(&format!(
                "<span class=\"{class}\">\n<span class=\"emoji\">\n"
            ));
            match r.kind {
                ReactionKind::Emoji => o.push_str(&esc(&r.emoji)),
                ReactionKind::Custom => o.push_str(&custom_emoji_link(
                    r.custom.as_deref().unwrap_or(""),
                    "\u{1F44B}",
                    base,
                )),
                ReactionKind::Paid => o.push_str(&esc("\u{2B50}")),
                ReactionKind::Empty => {}
            }
            o.push_str("\n</span>\n");
            if !r.recent.is_empty() {
                o.push_str("<span class=\"userpics\">\n");
                for (who, _) in &r.recent {
                    let mut u = peer_userpic(Some(*who), peers, 20, who.color_index());
                    u.tooltip = peers.name(*who);
                    o.push_str(&userpic_html(&u));
                }
                o.push_str("</span>\n");
            }
            if r.recent.is_empty() || r.count as usize > r.recent.len() {
                o.push_str(&format!(
                    "<span class=\"count\">\n{}\n</span>\n",
                    r.count
                ));
            }
            o.push_str("</span>\n");
        }
        o.push_str("</span>\n");
    }
    o.push_str("</div>\n</div>\n");
    (o, info(true))
}

struct Section {
    priority: i32,
    label: &'static str,
    kind: &'static str,
    count: usize,
    path: String,
}

struct ChatEntry {
    dialog: DialogInfo,
    count: usize,
}

struct ChatState {
    dir: PathBuf,
    rel_dir: String,
    out: BufWriter<File>,
    file_index: usize,
    total: usize,
    prev: Option<Prev>,
    last_day: i32,
    date_id: i64,
    base: String,
    title: String,
    dialog: DialogInfo,
    last_date: i32,
    /// Highest message id of every finished file of this chat.
    last_ids: Vec<i32>,
    last_id: i32,
}

pub struct HtmlWriter {
    root: PathBuf,
    /// One chat only: pages go straight into the export folder, no index.
    single: bool,
    personal: Option<PersonalInfo>,
    self_color: u8,
    userpics_count: usize,
    /// Path of the newest profile picture, used for the personal info userpic.
    userpic_first: String,
    sections: Vec<Section>,
    entries: Vec<ChatEntry>,
    left_entries: Vec<ChatEntry>,
    in_left: bool,
    chat: Option<ChatState>,
}

fn write_file(path: &Path, content: &str) -> io::Result<()> {
    if let Some(p) = path.parent() {
        fs::create_dir_all(p)?;
    }
    fs::write(path, content)
}

fn messages_file(index: usize) -> String {
    if index == 0 {
        "messages.html".into()
    } else {
        format!("messages{}.html", index + 1)
    }
}

/// `pushGenericListEntry`: userpic, then info, name, subname and details.
/// `image` replaces the initials with a saved picture.
#[allow(clippy::too_many_arguments)]
fn generic_entry(
    link: &str,
    u: &Userpic,
    image: &str,
    name: &str,
    subname: &str,
    details: &[String],
    info: &str,
    base: &str,
) -> String {
    let mut o = if link.is_empty() {
        "<div class=\"entry clearfix\">\n".to_string()
    } else {
        format!(
            "<a class=\"entry block_link clearfix\" href=\"{base}{}#allow_back\">\n",
            esc(link)
        )
    };
    o.push_str("<div class=\"pull_left userpic_wrap\">\n");
    o.push_str(&userpic_box(u, image, "", base));
    o.push_str("</div>\n<div class=\"body\">\n");
    if !info.is_empty() {
        o.push_str(&format!(
            "<div class=\"pull_right info details\">\n{}\n</div>\n",
            esc(info)
        ));
    }
    if !name.is_empty() {
        o.push_str(&format!("<div class=\"name bold\">\n{}\n</div>\n", esc(name)));
    }
    if !subname.is_empty() {
        o.push_str(&format!(
            "<div class=\"subname bold\">\n{}\n</div>\n",
            esc(subname)
        ));
    }
    for d in details {
        o.push_str(&format!(
            "<div class=\"details_entry details\">\n{}\n</div>\n",
            esc(d)
        ));
    }
    o.push_str("</div>\n");
    o.push_str(if link.is_empty() { "</div>\n" } else { "</a>\n" });
    o
}

/// `pushUserpic`: a saved picture when there is one, the initials otherwise,
/// optionally wrapped in a link to the full size file.
fn userpic_box(u: &Userpic, image: &str, large: &str, base: &str) -> String {
    let mut o = String::new();
    if !large.is_empty() {
        o.push_str(&format!(
            "<a class=\"userpic_link\" href=\"{base}{}\">\n",
            esc(large)
        ));
    }
    if !image.is_empty() {
        o.push_str(&format!(
            "<img class=\"userpic\" style=\"width: {0}px; height: {0}px\" src=\"{base}{1}\"/>\n",
            u.size,
            esc(image)
        ));
    } else {
        o.push_str(&userpic_html(u));
    }
    if !large.is_empty() {
        o.push_str("</a>\n");
    }
    o
}

/// The stories list entry: a thumbnail, the caption and the details.
fn story_entry(
    link: &str,
    image: &str,
    name: &str,
    details: &[String],
    info: &str,
    caption: &Text,
    base: &str,
) -> String {
    let mut o = String::from("<div class=\"entry clearfix\">\n");
    if link.is_empty() {
        o.push_str("<div class=\"pull_left userpic_wrap\">\n");
    } else {
        o.push_str(&format!(
            "<a class=\"pull_left userpic_wrap\" href=\"{base}{}#allow_back\">\n",
            esc(link)
        ));
    }
    if !image.is_empty() {
        o.push_str(&format!(
            "<img class=\"story\" style=\"width: {STORY_THUMB_WIDTH}px; height: {STORY_THUMB_HEIGHT}px\" src=\"{base}{}\"/>\n",
            esc(image)
        ));
    }
    o.push_str(if link.is_empty() { "</div>\n" } else { "</a>\n" });
    o.push_str("<div class=\"body\">\n");
    if !info.is_empty() {
        o.push_str(&format!(
            "<div class=\"pull_right info details\">\n{}\n</div>\n",
            esc(info)
        ));
    }
    if !name.is_empty() {
        if !link.is_empty() {
            o.push_str(&format!(
                "<a class=\"block_link expanded\" href=\"{base}{}#allow_back\">\n",
                esc(link)
            ));
        }
        o.push_str(&format!("<div class=\"name bold\">\n{}\n</div>\n", esc(name)));
        if !link.is_empty() {
            o.push_str("</a>\n");
        }
    }
    let text = if caption.is_empty() {
        String::new()
    } else {
        format_text(caption, base)
    };
    if !text.is_empty() {
        o.push_str(&format!("<div class=\"text\">\n{text}\n</div>\n"));
    }
    for d in details {
        o.push_str(&format!(
            "<div class=\"details_entry details\">\n{}\n</div>\n",
            esc(d)
        ));
    }
    o.push_str("</div>\n</div>\n");
    o
}

/// The profile music entry, with a note icon instead of a userpic.
fn audio_entry(link: &str, info: &str, details: &[String], duration: &str, base: &str) -> String {
    let mut o = String::from("<div class=\"entry clearfix\">\n");
    if link.is_empty() {
        o.push_str("<div class=\"pull_left userpic_wrap\">\n");
    } else {
        o.push_str(&format!(
            "<a class=\"pull_left userpic_wrap\" href=\"{base}{}#allow_back\">\n",
            esc(link)
        ));
    }
    o.push_str("<div class=\"userpic audio_icon\">\n</div>\n");
    o.push_str(if link.is_empty() { "</div>\n" } else { "</a>\n" });
    o.push_str("<div class=\"body\">\n");
    if !duration.is_empty() {
        o.push_str(&format!(
            "<div class=\"pull_right info details\">\n{}\n</div>\n",
            esc(duration)
        ));
    }
    if !info.is_empty() {
        if !link.is_empty() {
            o.push_str(&format!(
                "<a class=\"block_link expanded\" href=\"{base}{}#allow_back\">\n",
                esc(link)
            ));
        }
        o.push_str(&format!("<div class=\"name bold\">\n{}\n</div>\n", esc(info)));
        if !link.is_empty() {
            o.push_str("</a>\n");
        }
    }
    for d in details {
        o.push_str(&format!(
            "<div class=\"details_entry details\">\n{}\n</div>\n",
            esc(d)
        ));
    }
    o.push_str("</div>\n</div>\n");
    o
}

/// What the list pages say about a saved file, or why there is none.
fn file_status(file: &FileRef, what: &str) -> String {
    match file.skip {
        Some(SkipReason::Unavailable) => format!("({what} unavailable, please try again later)"),
        Some(SkipReason::FileSize) => format!(
            "({what} exceeds maximum size. Change data exporting settings to download.)"
        ),
        Some(SkipReason::FileType) => {
            format!("({what} not included. Change data exporting settings to download.)")
        }
        None => file_size(file.size),
    }
}

/// A square thumbnail of `large`, or an empty path when it is not a picture.
fn square_thumb(root: &Path, large: &str, size: u32, postfix: &str) -> String {
    let side = size as i32;
    thumbs::write_image_thumb(root, large, &|_| Some((side, side)), None, postfix)
        .map(|t| t.0)
        .unwrap_or_default()
}

fn string_bare_peer_id(data: &str) -> i64 {
    let mut result: u32 = 0xFF;
    for b in data.bytes() {
        result = (result * 239 + b as u32) & 0xFF;
    }
    result as i64
}

fn application_color_index(api_id: i32) -> u8 {
    match api_id {
        1 | 7 => 0,
        6 | 21724 => 1,
        2834 => 2,
        2496 => 3,
        2040 => 4,
        1429 => 5,
        other => color_index_for(other as i64),
    }
}

fn domain_application_id(domain: &str) -> i32 {
    0x1000 + string_bare_peer_id(domain) as i32
}

/// `ComposeName` for plain strings.
fn joined_name(first: &str, last: &str, empty: &str) -> String {
    if first.is_empty() && last.is_empty() {
        empty.to_string()
    } else {
        format!("{first} {last}")
    }
}

impl HtmlWriter {
    pub fn create(root: &Path, single: bool) -> io::Result<Self> {
        fs::create_dir_all(root)?;
        for (path, bytes) in assets::FILES {
            let dest = root.join(path);
            if let Some(p) = dest.parent() {
                fs::create_dir_all(p)?;
            }
            fs::write(dest, bytes)?;
        }
        Ok(HtmlWriter {
            root: root.to_path_buf(),
            single,
            personal: None,
            self_color: 0,
            userpics_count: 0,
            userpic_first: String::new(),
            sections: Vec::new(),
            entries: Vec::new(),
            left_entries: Vec::new(),
            in_left: false,
            chat: None,
        })
    }

    fn open_chat_file(chat: &mut ChatState) -> io::Result<()> {
        let path = chat.dir.join(messages_file(chat.file_index));
        let mut out = BufWriter::new(File::create(path)?);
        out.write_all(page_start(&chat.base).as_bytes())?;
        let back = format!("{}lists/chats.html", chat.base);
        let back = if chat.rel_dir.is_empty() {
            None
        } else {
            Some(back.as_str())
        };
        out.write_all(header(&chat.title, back).as_bytes())?;
        out.write_all(b"<div class=\"page_body chat_page\">\n<div class=\"history\">\n")?;
        if chat.file_index > 0 {
            out.write_all(
                format!(
                    "<a class=\"pagination block_link\" href=\"{}\">\nPrevious messages\n</a>\n",
                    messages_file(chat.file_index - 1)
                )
                .as_bytes(),
            )?;
        }
        chat.out = out;
        Ok(())
    }

    fn add_section(
        &mut self,
        priority: i32,
        label: &'static str,
        kind: &'static str,
        count: usize,
        path: &str,
    ) {
        self.sections.push(Section {
            priority,
            label,
            kind,
            count,
            path: path.to_string(),
        });
    }

    /// A page of entries below `lists/`, with a link back to the index.
    fn sub_page(
        &self,
        title: &str,
        about_text: Option<&str>,
        entries: &str,
        file: &str,
    ) -> io::Result<()> {
        let base = "../";
        let mut o = page_start(base);
        o.push_str(&header(title, Some("../export_results.html")));
        o.push_str("<div class=\"page_body list_page\">\n");
        if let Some(text) = about_text {
            o.push_str(&format!(
                "<div class=\"page_about details\">\n{}\n</div>\n",
                esc_br(text)
            ));
        }
        o.push_str("<div class=\"entry_list\">\n");
        o.push_str(entries);
        o.push_str(PAGE_END);
        write_file(&self.root.join(file), &o)
    }

    /// One page for the chat list and the left chats, `lists/chats.html`.
    fn chats_page(&self) -> io::Result<()> {
        let base = "../";
        let mut o = page_start(base);
        o.push_str(&header("Chats", Some("../export_results.html")));
        o.push_str("<div class=\"page_body list_page\">\n");
        for (left, entries) in [(false, &self.entries), (true, &self.left_entries)] {
            if entries.is_empty() {
                continue;
            }
            let about_text = if left {
                about::ABOUT_LEFT_CHATS
            } else {
                about::ABOUT_CHATS
            };
            o.push_str(&format!(
                "<div class=\"page_about details\">\n{}\n</div>\n",
                esc_br(about_text)
            ));
            o.push_str("<div class=\"entry_list\">\n");
            for e in entries {
                o.push_str(&chat_entry_html(e, base));
            }
            o.push_str("</div>\n");
        }
        o.push_str(PAGE_END);
        write_file(&self.root.join("lists/chats.html"), &o)
    }

    /// The personal info block of the index page.
    fn personal_block(&self, info: &PersonalInfo) -> String {
        let image = if self.userpic_first.is_empty() {
            String::new()
        } else {
            square_thumb(&self.root, &self.userpic_first, PERSONAL_USERPIC * 2, "_info")
        };
        let large = if self.userpic_first.is_empty() {
            ""
        } else {
            "lists/profile_pictures.html"
        };
        let u = Userpic {
            color: self.self_color,
            size: PERSONAL_USERPIC,
            first: info.first_name.clone(),
            last: info.last_name.clone(),
            tooltip: String::new(),
        };
        let mut o = String::from(
            "<div class=\"personal_info clearfix\">\n<div class=\"pull_right userpic_wrap\">\n",
        );
        o.push_str(&userpic_box(&u, &image, large, ""));
        o.push_str("</div>\n");
        let rows = |name: &str, vals: &[(&str, String)]| -> String {
            let mut r = format!("<div class=\"rows {name}\">\n");
            for (k, v) in vals {
                if v.is_empty() {
                    continue;
                }
                r.push_str(&format!(
                    "<div class=\"row\">\n<div class=\"label details\">\n{}\n</div>\n<div class=\"value bold\">\n{}\n</div>\n</div>\n",
                    esc(k),
                    esc(v)
                ));
            }
            r.push_str("</div>\n");
            r
        };
        o.push_str(&rows(
            "names",
            &[
                ("First name", info.first_name.clone()),
                ("Last name", info.last_name.clone()),
            ],
        ));
        o.push_str(&rows(
            "info",
            &[
                ("Phone number", format_phone(&info.phone)),
                (
                    "Username",
                    if info.username.is_empty() {
                        String::new()
                    } else {
                        format!("@{}", info.username)
                    },
                ),
            ],
        ));
        o.push_str(&rows("bio", &[("Bio", info.bio.clone())]));
        o.push_str("</div>\n");
        o
    }
}

/// A row of the chat list.
fn chat_entry_html(e: &ChatEntry, base: &str) -> String {
    let d = &e.dialog;
    let special = matches!(
        d.dtype,
        DialogType::Saved | DialogType::Replies | DialogType::VerifyCodes
    );
    let first = match d.dtype {
        DialogType::Saved => "Saved messages".to_string(),
        DialogType::Replies => "Replies".to_string(),
        DialogType::VerifyCodes => "Verification Codes".to_string(),
        _ => d.name.clone(),
    };
    let last = if matches!(d.dtype, DialogType::Personal | DialogType::Bot) {
        d.last_name.clone()
    } else {
        String::new()
    };
    let deleted = match d.dtype {
        DialogType::Saved
        | DialogType::Replies
        | DialogType::VerifyCodes
        | DialogType::Personal
        | DialogType::Bot => "Deleted Account",
        DialogType::PrivateGroup | DialogType::PrivateSupergroup | DialogType::PublicSupergroup => {
            "Deleted Group"
        }
        DialogType::PrivateChannel | DialogType::PublicChannel => "Deleted Channel",
    };
    let kind = match d.dtype {
        DialogType::Saved | DialogType::Replies | DialogType::VerifyCodes | DialogType::Personal => {
            "private"
        }
        DialogType::Bot => "bot",
        DialogType::PrivateGroup | DialogType::PrivateSupergroup | DialogType::PublicSupergroup => {
            "group"
        }
        DialogType::PrivateChannel | DialogType::PublicChannel => "channel",
    };
    let count = match (e.count, d.only_my_messages) {
        (1, true) => "1 outgoing message".to_string(),
        (1, false) => "1 message".to_string(),
        (0, true) => "No outgoing messages".to_string(),
        (0, false) => "No messages".to_string(),
        (n, true) => format!("{n} outgoing messages"),
        (n, false) => format!("{n} messages"),
    };
    let u = Userpic {
        color: if special {
            SAVED_MESSAGES_COLOR
        } else {
            d.peer.color_index()
        },
        size: ENTRY_USERPIC,
        first,
        last,
        tooltip: String::new(),
    };
    let link = if e.count > 0 {
        format!("{}messages.html", d.relative_path)
    } else {
        String::new()
    };
    generic_entry(
        &link,
        &u,
        "",
        &compose_name(&u, deleted),
        "",
        &[count],
        kind,
        base,
    )
}

impl Writer for HtmlWriter {
    fn personal(&mut self, info: &PersonalInfo) -> io::Result<()> {
        self.self_color = info.color_index;
        self.personal = Some(info.clone());
        Ok(())
    }

    fn userpics(&mut self, list: &[crate::model::Userpic]) -> io::Result<()> {
        self.userpics_count = list.len();
        if list.is_empty() {
            return Ok(());
        }
        self.userpic_first = list[0].file.relative_path.clone();
        let mut entries = String::new();
        for pic in list {
            let u = Userpic {
                color: self.self_color,
                size: ENTRY_USERPIC,
                first: pic.file.relative_path.clone(),
                last: String::new(),
                tooltip: String::new(),
            };
            let path = &pic.file.relative_path;
            let image = square_thumb(&self.root, path, ENTRY_USERPIC * 2, "_thumb");
            let name = if path.is_empty() {
                "Photo unavailable".to_string()
            } else {
                path.clone()
            };
            let info = if pic.date > 0 {
                date_time(pic.date, false)
            } else {
                String::new()
            };
            entries.push_str(&generic_entry(
                path,
                &u,
                &image,
                &name,
                "",
                &[file_status(&pic.file, "Photo")],
                &info,
                "../",
            ));
        }
        self.sub_page(
            "Profile pictures",
            None,
            &entries,
            "lists/profile_pictures.html",
        )?;
        self.add_section(
            USERPICS_PRIORITY,
            "Profile pictures",
            "photos",
            list.len(),
            "lists/profile_pictures.html",
        );
        Ok(())
    }

    fn stories(&mut self, list: &[Story]) -> io::Result<()> {
        if list.is_empty() {
            return Ok(());
        }
        let mut entries = String::new();
        for st in list {
            let mut details = Vec::new();
            if st.pinned {
                details.push("Saved to Profile".to_string());
            }
            if st.expires > 0 {
                details.push(format!("Expiring: {}", date_time(st.expires, false)));
            }
            details.push(file_status(&st.file, "Story"));
            let path = &st.file.relative_path;
            let preview = match &st.thumb {
                Some(t) if !t.file.relative_path.is_empty() => &t.file.relative_path,
                _ => path,
            };
            let convert = |_| Some(((STORY_THUMB_WIDTH * 2) as i32, (STORY_THUMB_HEIGHT * 2) as i32));
            let image = thumbs::write_image_thumb(&self.root, preview, &convert, None, "_thumb")
                .map(|t| t.0)
                .unwrap_or_default();
            let info = if st.date > 0 {
                date_time(st.date, false)
            } else {
                String::new()
            };
            let name = if path.is_empty() {
                "Story unavailable"
            } else {
                path
            };
            entries.push_str(&story_entry(
                path, &image, name, &details, &info, &st.caption, "../",
            ));
        }
        self.sub_page("Stories archive", None, &entries, "lists/stories.html")?;
        self.add_section(
            STORIES_PRIORITY,
            "Stories archive",
            "stories",
            list.len(),
            "lists/stories.html",
        );
        Ok(())
    }

    fn profile_music(&mut self, list: &[Message], _peers: &Peers) -> io::Result<()> {
        let mut entries = String::new();
        let mut count = 0;
        for m in list {
            let Media::Document {
                file,
                name,
                duration: dur,
                performer,
                title,
                flags,
                ..
            } = &m.media
            else {
                continue;
            };
            if !flags.audio {
                continue;
            }
            count += 1;
            let song_title = if !title.is_empty() {
                title.as_str()
            } else if !name.is_empty() {
                name.as_str()
            } else {
                "Unknown Track"
            };
            let song_performer = if !performer.is_empty() {
                performer.as_str()
            } else {
                "Unknown Artist"
            };
            let duration_text = if *dur > 0 {
                duration(*dur)
            } else {
                String::new()
            };
            entries.push_str(&audio_entry(
                &file.relative_path,
                &format!("{song_performer} - {song_title}"),
                &[file_status(file, "File")],
                &duration_text,
                "../",
            ));
        }
        if count == 0 {
            return Ok(());
        }
        self.sub_page("Profile Music", None, &entries, "lists/profile_music.html")?;
        self.add_section(
            PROFILE_MUSIC_PRIORITY,
            "Profile Music",
            "music",
            count,
            "lists/profile_music.html",
        );
        Ok(())
    }

    fn contacts(&mut self, list: &ContactsList) -> io::Result<()> {
        let base = "../";
        if !list.contacts.is_empty() {
            let mut sorted: Vec<&Contact> = list.contacts.iter().collect();
            sorted.sort_by_key(|c| format!("{} {}", c.first_name, c.last_name).to_lowercase());
            let mut entries = String::new();
            for c in sorted {
                let u = Userpic {
                    color: color_index_for(string_bare_peer_id(&c.phone)),
                    size: ENTRY_USERPIC,
                    first: c.first_name.clone(),
                    last: c.last_name.clone(),
                    tooltip: String::new(),
                };
                entries.push_str(&generic_entry(
                    "",
                    &u,
                    "",
                    &joined_name(&c.first_name, &c.last_name, "Deleted Account"),
                    "",
                    &[format_phone(&c.phone)],
                    &date_time(c.date, false),
                    base,
                ));
            }
            self.sub_page(
                "Contacts",
                Some(about::ABOUT_CONTACTS),
                &entries,
                "lists/contacts.html",
            )?;
            self.add_section(
                CONTACTS_PRIORITY,
                "Contacts",
                "contacts",
                list.contacts.len(),
                "lists/contacts.html",
            );
        }

        let size = list.correspondents.len() + list.inline_bots.len() + list.phone_calls.len();
        if size > 0 {
            let mut frequent = String::new();
            for (category, group) in [
                ("people", &list.correspondents),
                ("inline bots", &list.inline_bots),
                ("calls", &list.phone_calls),
            ] {
                for t in group {
                    let is_user = matches!(t.id, PeerId::User(_));
                    let first = if !is_user {
                        t.name.clone()
                    } else if t.is_self {
                        "Saved messages".to_string()
                    } else {
                        t.first_name.clone()
                    };
                    let last = if is_user && !t.is_self {
                        t.last_name.clone()
                    } else {
                        String::new()
                    };
                    let u = Userpic {
                        color: t.id.color_index(),
                        size: ENTRY_USERPIC,
                        first,
                        last,
                        tooltip: String::new(),
                    };
                    frequent.push_str(&generic_entry(
                        "",
                        &u,
                        "",
                        &compose_name(&u, "Deleted Account"),
                        "",
                        &[format!("Rating: {:.6}", t.rating)],
                        category,
                        base,
                    ));
                }
            }
            self.sub_page(
                "Frequent contacts",
                Some(about::ABOUT_FREQUENT),
                &frequent,
                "lists/frequent.html",
            )?;
            self.add_section(
                FREQUENT_PRIORITY,
                "Frequent contacts",
                "frequent",
                size,
                "lists/frequent.html",
            );
        }
        Ok(())
    }

    fn sessions(&mut self, list: &SessionsList) -> io::Result<()> {
        let base = "../";
        if !list.list.is_empty() {
            let mut sessions = String::new();
            for x in &list.list {
                let app = if x.app_name.is_empty() {
                    "Unknown"
                } else {
                    &x.app_name
                };
                let name = format!("{app} {}", x.app_version);
                let u = Userpic {
                    color: application_color_index(x.application_id),
                    size: ENTRY_USERPIC,
                    first: name.clone(),
                    last: String::new(),
                    tooltip: String::new(),
                };
                let place = if x.region.is_empty() || x.country.is_empty() {
                    format!("{}{}", x.region, x.country)
                } else {
                    format!("{}, {}", x.region, x.country)
                };
                let details = [
                    format!("{} \u{2013} {}", x.ip, place),
                    format!("Last active: {}", date_time(x.last_active, false)),
                    format!("Created: {}", date_time(x.created, false)),
                ];
                sessions.push_str(&generic_entry(
                    "",
                    &u,
                    "",
                    &name,
                    &format!("{}, {} {}", x.device_model, x.platform, x.system_version),
                    &details,
                    "",
                    base,
                ));
            }
            self.sub_page(
                "Sessions",
                Some(about::ABOUT_SESSIONS),
                &sessions,
                "lists/sessions.html",
            )?;
            self.add_section(
                SESSIONS_PRIORITY,
                "Sessions",
                "sessions",
                list.list.len(),
                "lists/sessions.html",
            );
        }

        if !list.web_list.is_empty() {
            let mut web = String::new();
            for x in &list.web_list {
                let name = if x.domain.is_empty() {
                    "Unknown"
                } else {
                    &x.domain
                };
                let u = Userpic {
                    color: application_color_index(domain_application_id(&x.domain)),
                    size: ENTRY_USERPIC,
                    first: name.to_string(),
                    last: String::new(),
                    tooltip: String::new(),
                };
                let details = [
                    format!("{} \u{2013} {}", x.ip, x.region),
                    format!("Last active: {}", date_time(x.last_active, false)),
                    format!("Created: {}", date_time(x.created, false)),
                ];
                let info = if x.bot_username.is_empty() {
                    String::new()
                } else {
                    format!("@{}", x.bot_username)
                };
                web.push_str(&generic_entry(
                    "",
                    &u,
                    "",
                    name,
                    &format!("{}, {}", x.platform, x.browser),
                    &details,
                    &info,
                    base,
                ));
            }
            self.sub_page(
                "Web sessions",
                Some(about::ABOUT_WEB_SESSIONS),
                &web,
                "lists/web_sessions.html",
            )?;
            self.add_section(
                WEB_SESSIONS_PRIORITY,
                "Web sessions",
                "web",
                list.web_list.len(),
                "lists/web_sessions.html",
            );
        }
        Ok(())
    }

    fn other_data(&mut self, file: &FileRef) -> io::Result<()> {
        if !file.relative_path.is_empty() {
            self.add_section(OTHER_PRIORITY, "Other data", "other", 1, &file.relative_path);
        }
        Ok(())
    }

    fn dialogs_start(&mut self, left: bool) -> io::Result<()> {
        self.in_left = left;
        Ok(())
    }

    fn dialog_start(&mut self, d: &DialogInfo, _peers: &Peers) -> io::Result<()> {
        let rel = if self.single {
            String::new()
        } else {
            d.relative_path.clone()
        };
        let dir = self.root.join(&rel);
        fs::create_dir_all(&dir)?;
        let depth = if rel.is_empty() {
            0
        } else {
            rel.trim_end_matches('/').matches('/').count() + 1
        };
        let title = joined_name(&d.name, &d.last_name, "Deleted Account");
        let out = BufWriter::new(File::create(dir.join(messages_file(0)))?);
        let mut chat = ChatState {
            dir,
            rel_dir: rel,
            out,
            file_index: 0,
            total: 0,
            prev: None,
            last_day: 0,
            date_id: 0,
            base: base_for(depth),
            title,
            dialog: d.clone(),
            last_date: 0,
            last_ids: Vec::new(),
            last_id: 0,
        };
        HtmlWriter::open_chat_file(&mut chat)?;
        self.chat = Some(chat);
        Ok(())
    }

    fn messages(&mut self, d: &DialogInfo, msgs: &[Message], peers: &Peers) -> io::Result<()> {
        let Some(chat) = self.chat.as_mut() else {
            return Ok(());
        };
        for m in msgs {
            let new_index = chat.total / MESSAGES_IN_FILE;
            if new_index != chat.file_index {
                chat.out.write_all(
                    format!(
                        "<a class=\"pagination block_link\" href=\"{}\">\nNext messages\n</a>\n",
                        messages_file(new_index)
                    )
                    .as_bytes(),
                )?;
                chat.out.write_all(PAGE_END.as_bytes())?;
                chat.out.flush()?;
                chat.last_ids.push(chat.last_id);
                chat.file_index = new_index;
                chat.prev = None;
                chat.last_day = 0;
                HtmlWriter::open_chat_file(chat)?;
            }
            let day = day_key(m.date);
            if chat.last_day == 0 || day != chat.last_day {
                chat.date_id -= 1;
                chat.out.write_all(
                    service_block(
                        chat.date_id,
                        &date_text(m.date),
                        None,
                        None,
                        &self.root,
                        &chat.base,
                    )
                    .as_bytes(),
                )?;
                chat.last_day = day;
            }
            let (block, info) = message_block(
                m,
                chat.prev.as_ref(),
                d,
                peers,
                &self.root,
                &chat.base,
                &chat.last_ids,
            );
            chat.out.write_all(block.as_bytes())?;
            chat.prev = Some(info);
            chat.total += 1;
            chat.last_id = m.id;
            chat.last_date = chat.last_date.max(m.date);
        }
        Ok(())
    }

    fn dialog_end(&mut self) -> io::Result<()> {
        let Some(mut chat) = self.chat.take() else {
            return Ok(());
        };
        // Only a single exported chat says so; in the chat list the count is enough.
        if chat.total == 0 && self.single {
            chat.out.write_all(
                service_block(
                    -1,
                    "No exported messages",
                    None,
                    None,
                    &self.root,
                    &chat.base,
                )
                .as_bytes(),
            )?;
        }
        chat.out.write_all(PAGE_END.as_bytes())?;
        chat.out.flush()?;
        let entry = ChatEntry {
            dialog: chat.dialog.clone(),
            count: chat.total,
        };
        let _ = &chat.dir;
        if self.single {
            return Ok(());
        }
        if self.in_left {
            self.left_entries.push(entry)
        } else {
            self.entries.push(entry)
        }
        Ok(())
    }

    fn dialogs_end(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn finish(mut self: Box<Self>) -> io::Result<()> {
        if self.single {
            return Ok(());
        }
        let chats = self.entries.len() + self.left_entries.len();
        if chats > 0 {
            self.chats_page()?;
            self.add_section(CHATS_PRIORITY, "Chats", "chats", chats, "lists/chats.html");
        }
        let mut o = page_start("");
        o.push_str(&header("Exported Data", None));
        o.push_str("<div class=\"page_body\">\n");
        let has_personal = match self.personal.take() {
            Some(info) => {
                o.push_str(&self.personal_block(&info));
                true
            }
            None => false,
        };
        self.sections.sort_by_key(|s| s.priority);
        let has_sections = !self.sections.is_empty();
        if has_sections {
            o.push_str(if has_personal {
                "<div class=\"sections with_divider\">\n"
            } else {
                "<div class=\"sections\">\n"
            });
            for s in &self.sections {
                o.push_str(&format!(
                    "<a class=\"section block_link {}\" href=\"{}#allow_back\">\n<div class=\"counter details\">\n{}\n</div>\n<div class=\"label bold\">\n{}\n</div>\n</a>\n",
                    s.kind,
                    esc(&s.path),
                    s.count,
                    s.label
                ));
            }
            o.push_str("</div>\n");
        }
        let divider = has_sections || has_personal;
        o.push_str(&format!(
            "<div class=\"{}\">\n{}\n</div>\n",
            if divider {
                "page_about details with_divider"
            } else {
                "page_about details"
            },
            esc_br(about::ABOUT_TELEGRAM)
        ));
        o.push_str("</div>\n</div>\n</body>\n</html>\n");
        write_file(&self.root.join("export_results.html"), &o)
    }
}
