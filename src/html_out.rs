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

const MESSAGES_IN_FILE: usize = 1000;
const JOIN_WITHIN_SECONDS: i32 = 900;
const HISTORY_USERPIC: u32 = 42;
const ENTRY_USERPIC: u32 = 48;
const PERSONAL_USERPIC: u32 = 90;

pub fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            c => o.push(c),
        }
    }
    o
}

fn esc_br(s: &str) -> String {
    esc(s).replace('\n', "<br>")
}

fn local(ts: i32) -> Option<chrono::DateTime<Local>> {
    Local.timestamp_opt(ts as i64, 0).single()
}

fn date_text(ts: i32) -> String {
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

fn time_text(ts: i32) -> String {
    match local(ts) {
        Some(d) => format!("{:02}:{:02}", d.hour(), d.minute()),
        None => String::new(),
    }
}

fn date_time(ts: i32, seconds: bool) -> String {
    match local(ts) {
        Some(d) => {
            if seconds {
                d.format("%d.%m.%Y %H:%M:%S").to_string()
            } else {
                d.format("%d.%m.%Y %H:%M").to_string()
            }
        }
        None => String::new(),
    }
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

fn no_file(r: Option<SkipReason>) -> &'static str {
    match r {
        Some(SkipReason::Unavailable) => "Unavailable, please try again later.",
        Some(SkipReason::FileSize) => {
            "Exceeds maximum size, change data exporting settings to download."
        }
        Some(SkipReason::FileType) => "Not included, change data exporting settings to download.",
        None => "",
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

fn display_name(id: Option<PeerId>, peers: &Peers) -> String {
    let n = id.map(|i| peers.name(i)).unwrap_or_default();
    if n.is_empty() {
        "Deleted Account".into()
    } else {
        n
    }
}

fn wrap_name(id: PeerId, peers: &Peers) -> String {
    esc(&display_name(Some(id), peers))
}

fn wrap_user_names(ids: &[i64], peers: &Peers) -> String {
    ids.iter()
        .map(|u| wrap_name(PeerId::User(*u), peers))
        .collect::<Vec<_>>()
        .join(", ")
}

fn safe_href(url: &str) -> Option<String> {
    let u = url.trim();
    let lower = u.to_lowercase();
    if lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("tg://")
        || lower.starts_with("mailto:")
        || lower.starts_with("tel:")
        || lower.starts_with("ftp://")
    {
        Some(u.to_string())
    } else if u.contains('.') && !u.contains(' ') && !u.contains(':') {
        Some(format!("http://{u}"))
    } else {
        None
    }
}

fn link(text: &str, target: &str) -> String {
    match safe_href(target) {
        Some(h) => format!("<a href=\"{}\">{}</a>", esc(&h), text),
        None => text.to_string(),
    }
}

fn js_str(s: &str) -> String {
    format!(
        "'{}'",
        s.replace('\\', "\\\\")
            .replace('\'', "\\'")
            .replace('\n', " ")
    )
}

pub fn format_text(parts: &Text) -> String {
    let mut out = String::new();
    for p in parts {
        let t = esc_br(&p.text);
        let raw = &p.text;
        let s = match p.kind {
            TextKind::Text | TextKind::Unknown | TextKind::TonAddress => t,
            TextKind::Mention => {
                let user = raw.trim_start_matches('@');
                link(&t, &format!("https://t.me/{user}"))
            }
            TextKind::Hashtag => format!(
                "<a href=\"\" onclick=\"return ShowHashtag({})\">{}</a>",
                esc(&js_str(raw.trim_start_matches('#'))),
                t
            ),
            TextKind::BotCommand => format!(
                "<a href=\"\" onclick=\"return ShowBotCommand({})\">{}</a>",
                esc(&js_str(raw.trim_start_matches('/'))),
                t
            ),
            TextKind::Url => link(&t, raw),
            TextKind::Email => link(&t, &format!("mailto:{raw}")),
            TextKind::Phone => link(&t, &format!("tel:{raw}")),
            TextKind::Bold => format!("<strong>{t}</strong>"),
            TextKind::Italic => format!("<em>{t}</em>"),
            TextKind::Code => format!("<code>{t}</code>"),
            TextKind::Pre => format!("<pre>{t}</pre>"),
            TextKind::TextUrl => link(&t, &p.additional),
            TextKind::MentionName => {
                format!("<a href=\"\" onclick=\"return ShowMentionName()\">{t}</a>")
            }
            TextKind::Cashtag => format!(
                "<a href=\"\" onclick=\"return ShowCashtag({})\">{}</a>",
                esc(&js_str(raw.trim_start_matches('$'))),
                t
            ),
            TextKind::Underline => format!("<u>{t}</u>"),
            TextKind::Strike => format!("<s>{t}</s>"),
            TextKind::Blockquote => format!("<blockquote>{t}</blockquote>"),
            TextKind::BankCard => {
                format!("<a href=\"\" onclick=\"return ShowToast('Bank card')\">{t}</a>")
            }
            TextKind::Spoiler => format!(
                "<span class=\"spoiler hidden\" onclick=\"ShowSpoiler(this)\"><span aria-hidden=\"true\">{t}</span></span>"
            ),
            TextKind::CustomEmoji => {
                format!("<a href=\"\" onclick=\"return ShowNotLoadedEmoji();\">{t}</a>")
            }
        };
        out.push_str(&s);
    }
    out
}

struct Generic {
    classes: String,
    title: String,
    description: String,
    status: String,
    link: String,
}

fn generic_html(g: &Generic, base: &str) -> String {
    let mut o = String::new();
    o.push_str("<div class=\"media_wrap clearfix\">\n");
    if g.link.is_empty() {
        o.push_str(&format!(
            "<div class=\"media clearfix pull_left {}\">\n",
            g.classes
        ));
    } else {
        let href = if g.link.starts_with("http") {
            g.link.clone()
        } else {
            format!("{base}{}", g.link)
        };
        o.push_str(&format!(
            "<a class=\"media clearfix pull_left block_link {}\" href=\"{}\">\n",
            g.classes,
            esc(&href)
        ));
    }
    o.push_str("<div class=\"fill pull_left\">\n</div>\n<div class=\"body\">\n");
    if !g.title.is_empty() {
        o.push_str(&format!(
            "<div class=\"title bold\">\n{}\n</div>\n",
            esc(&g.title)
        ));
    }
    if !g.description.is_empty() {
        o.push_str(&format!(
            "<div class=\"description\">\n{}\n</div>\n",
            esc(&g.description)
        ));
    }
    if !g.status.is_empty() {
        o.push_str(&format!(
            "<div class=\"status details\">\n{}\n</div>\n",
            esc(&g.status)
        ));
    }
    o.push_str("</div>\n");
    o.push_str(if g.link.is_empty() {
        "</div>\n"
    } else {
        "</a>\n"
    });
    o.push_str("</div>\n");
    o
}

fn scaled(w: i32, h: i32, max: i32) -> (i32, i32) {
    if w <= 0 || h <= 0 {
        return (max, max);
    }
    let k = (max as f64 / w.max(h) as f64).min(1.0);
    (
        ((w as f64) * k).round() as i32,
        ((h as f64) * k).round() as i32,
    )
}

fn image_block(
    class_wrap: &str,
    class_img: &str,
    path: &str,
    w: i32,
    h: i32,
    max: i32,
    base: &str,
) -> String {
    let (sw, sh) = scaled(w, h, max);
    format!(
        "<a class=\"{} clearfix pull_left\" href=\"{base}{p}\">\n<img class=\"{}\" src=\"{base}{p}\" style=\"width: {}px; height: {}px\"/>\n</a>\n",
        class_wrap,
        class_img,
        sw,
        sh,
        p = esc(path),
    )
}

fn is_web_image(path: &str) -> bool {
    let l = path.to_lowercase();
    l.ends_with(".jpg")
        || l.ends_with(".jpeg")
        || l.ends_with(".png")
        || l.ends_with(".webp")
        || l.ends_with(".gif")
}

fn maps_link(la: f64, lo: f64) -> String {
    format!("https://maps.google.com/maps?q={la:.6},{lo:.6}&ll={la:.6},{lo:.6}&z=16")
}

fn simple(classes: &str, title: &str, description: &str, status: &str, link: &str) -> Generic {
    Generic {
        classes: classes.into(),
        title: title.into(),
        description: description.into(),
        status: status.into(),
        link: link.into(),
    }
}

fn media_html(m: &Message, dialog: &DialogInfo, base: &str) -> String {
    if let Some(Action::PhoneCall {
        duration: dur,
        discard_reason,
    }) = &m.action
    {
        let status = match (discard_reason.as_str(), m.out) {
            ("missed", true) => "Cancelled",
            ("missed", false) => "Missed",
            ("busy", false) => "Declined",
            (_, true) => "Outgoing",
            (_, false) => "Incoming",
        };
        let mut classes = "media_call".to_string();
        let mut st = status.to_string();
        if *dur > 0 {
            classes.push_str(" success");
            st.push_str(&format!(" ({dur} seconds)"));
        }
        return generic_html(&simple(&classes, &dialog.name, "", &st, ""), base);
    }
    match &m.media {
        Media::None | Media::Unsupported => String::new(),
        Media::Photo {
            file,
            width,
            height,
            ttl,
            ..
        } => {
            if *ttl != 0 {
                return generic_html(
                    &simple(
                        "media_photo",
                        "Self-destructing photo",
                        "",
                        "Please view it on your mobile",
                        "",
                    ),
                    base,
                );
            }
            if !file.relative_path.is_empty() {
                return image_block(
                    "photo_wrap",
                    "photo",
                    &file.relative_path,
                    *width,
                    *height,
                    260,
                    base,
                );
            }
            generic_html(
                &simple(
                    "media_photo",
                    "Photo",
                    no_file(file.skip),
                    &format!("{}x{}, {}", width, height, file_size(file.size)),
                    "",
                ),
                base,
            )
        }
        Media::Document {
            file,
            name,
            duration: dur,
            width,
            height,
            flags,
            sticker_emoji,
            performer,
            title,
            ttl,
            ..
        } => {
            if *ttl != 0 {
                return generic_html(
                    &simple(
                        "media_video",
                        "Self-destructing video",
                        "",
                        "Please view it on your mobile",
                        "",
                    ),
                    base,
                );
            }
            let has = !file.relative_path.is_empty();
            if flags.sticker && has && is_web_image(&file.relative_path) {
                return image_block(
                    "sticker_wrap",
                    "sticker",
                    &file.relative_path,
                    *width,
                    *height,
                    192,
                    base,
                );
            }
            let missing = if has {
                String::new()
            } else {
                format!(", {}", file_size(file.size))
            };
            let desc = no_file(file.skip);
            let link = file.relative_path.as_str();
            let g = if flags.sticker {
                simple("media_file", "Sticker", desc, sticker_emoji, link)
            } else if flags.video_message {
                simple(
                    "media_video",
                    "Video message",
                    desc,
                    &format!("{}{}", duration(*dur), missing),
                    link,
                )
            } else if flags.voice {
                simple(
                    "media_voice_message",
                    "Voice message",
                    desc,
                    &format!("{}{}", duration(*dur), missing),
                    link,
                )
            } else if flags.animated {
                simple(
                    "media_video",
                    "Animation",
                    desc,
                    &file_size(file.size),
                    link,
                )
            } else if flags.video_file {
                simple(
                    "media_video",
                    "Video file",
                    desc,
                    &format!("{}, {}", duration(*dur), file_size(file.size)),
                    link,
                )
            } else if flags.audio {
                let t = if !performer.is_empty() && !title.is_empty() {
                    format!("{performer} - {title}")
                } else if !title.is_empty() {
                    title.clone()
                } else if !name.is_empty() {
                    name.clone()
                } else {
                    "Audio file".to_string()
                };
                simple(
                    "media_audio_file",
                    &t,
                    desc,
                    &format!("{}, {}", duration(*dur), file_size(file.size)),
                    link,
                )
            } else {
                let t = if name.is_empty() {
                    "File"
                } else {
                    name.as_str()
                };
                simple("media_file", t, desc, &file_size(file.size), link)
            };
            generic_html(&g, base)
        }
        Media::Contact {
            first_name,
            last_name,
            phone,
            vcard,
        } => {
            let mut status = if phone.is_empty() || phone.starts_with('+') {
                phone.clone()
            } else {
                format!("+{phone}")
            };
            let mut link = String::new();
            if let Some(v) = vcard {
                status.push_str(" - vCard");
                link = v.relative_path.clone();
            }
            generic_html(
                &simple(
                    "media_contact",
                    &format!("{first_name} {last_name}"),
                    "",
                    &status,
                    &link,
                ),
                base,
            )
        }
        Media::Geo { point, live_period } => {
            let live = *live_period != 0;
            let (status, link) = match point {
                Some((la, lo)) => (format!("{la:.6}, {lo:.6}"), maps_link(*la, *lo)),
                None => (String::new(), String::new()),
            };
            generic_html(
                &simple(
                    if live {
                        "media_live_location"
                    } else {
                        "media_location"
                    },
                    if live { "Live location" } else { "Location" },
                    "",
                    &status,
                    &link,
                ),
                base,
            )
        }
        Media::Venue {
            point,
            title,
            address,
        } => {
            let link = point.map(|(la, lo)| maps_link(la, lo)).unwrap_or_default();
            generic_html(&simple("media_venue", title, address, "", &link), base)
        }
        Media::Game {
            title, description, ..
        } => generic_html(&simple("media_game", title, description, "", ""), base),
        Media::Invoice {
            title,
            description,
            amount,
            currency,
            ..
        } => generic_html(
            &simple(
                "media_invoice",
                title,
                description,
                &format!("{:.2} {}", *amount as f64 / 100.0, currency),
                "",
            ),
            base,
        ),
        Media::Dice { emoji, value } => generic_html(
            &simple("media_game", &format!("{emoji} {value}"), "", "", ""),
            base,
        ),
        Media::Poll(p) => {
            let mut o = String::from("<div class=\"media_poll\">\n");
            o.push_str(&format!(
                "<div class=\"question bold\">\n{}\n</div>\n",
                format_text(&p.question)
            ));
            o.push_str(&format!(
                "<div class=\"details\">\n{}\n</div>\n",
                if p.closed { "Closed poll" } else { "Poll" }
            ));
            for (t, votes, chosen) in &p.answers {
                o.push_str(&format!(
                    "<div class=\"answer details\">\n- {}{} <span class=\"details\">{} votes</span>\n</div>\n",
                    format_text(t),
                    if *chosen { " (chosen)" } else { "" },
                    votes
                ));
            }
            o.push_str(&format!(
                "<div class=\"total details\">\n{} voters\n</div>\n</div>\n",
                p.total_votes
            ));
            o
        }
    }
}

fn service_text(m: &Message, dialog: &DialogInfo, peers: &Peers) -> Option<String> {
    let channel = dialog.dtype.is_channel();
    let from = m
        .from
        .map(|f| wrap_name(f, peers))
        .unwrap_or_else(|| "Deleted Account".into());
    let reply_link = |text: &str| -> String {
        if m.reply_to != 0 {
            format!(
                "<a href=\"#go_to_message{id}\" onclick=\"return GoToMessage({id})\">{text}</a>",
                id = m.reply_to
            )
        } else {
            text.to_string()
        }
    };
    let a = m.action.as_ref()?;
    Some(match a {
        Action::ChatCreate { title, users } => {
            let mut s = format!("{from} created group &laquo;{}&raquo;", esc(title));
            if !users.is_empty() {
                s.push_str(&format!(" with members {}", wrap_user_names(users, peers)));
            }
            s
        }
        Action::ChatEditTitle { title } => {
            if channel {
                format!("Channel title changed to &laquo;{}&raquo;", esc(title))
            } else {
                format!("{from} changed group title to &laquo;{}&raquo;", esc(title))
            }
        }
        Action::ChatEditPhoto { .. } => {
            if channel {
                "Channel photo changed".into()
            } else {
                format!("{from} changed group photo")
            }
        }
        Action::ChatDeletePhoto => {
            if channel {
                "Channel photo removed".into()
            } else {
                format!("{from} removed group photo")
            }
        }
        Action::ChatAddUser { users } => {
            format!("{from} invited {}", wrap_user_names(users, peers))
        }
        Action::ChatDeleteUser { user } => {
            format!("{from} removed {}", wrap_name(PeerId::User(*user), peers))
        }
        Action::ChatJoinedByLink { inviter } => {
            format!(
                "{from} joined group by link from {}",
                wrap_name(PeerId::User(*inviter), peers)
            )
        }
        Action::ChannelCreate { title } => format!("Channel &laquo;{}&raquo; created", esc(title)),
        Action::ChatMigrateTo => format!("{from} converted this group to a supergroup"),
        Action::ChannelMigrateFrom { title } => {
            format!(
                "{from} converted a basic group to this supergroup &laquo;{}&raquo;",
                esc(title)
            )
        }
        Action::PinMessage => format!("{from} pinned {}", reply_link("this message")),
        Action::HistoryClear => "History cleared".into(),
        Action::GameScore { score } => {
            format!("{from} scored {score} in {}", reply_link("this game"))
        }
        Action::PaymentSent { amount, currency } => format!(
            "You have successfully transferred {:.2} {} for {}",
            *amount as f64 / 100.0,
            esc(currency),
            reply_link("this invoice")
        ),
        Action::PhoneCall { .. } => return None,
        Action::ScreenshotTaken => format!("{from} took a screenshot"),
        Action::CustomAction { message } => esc(message),
        Action::BotAllowed { domain } => {
            format!(
                "You allowed this bot to message you when you logged in on {}",
                esc(domain)
            )
        }
        Action::ContactSignUp => format!("{from} joined Telegram"),
        Action::GroupCall { duration } => {
            let d = if *duration != 0 {
                format!(" ({duration} seconds)")
            } else {
                String::new()
            };
            if channel {
                format!("Voice chat{d}")
            } else {
                format!("{from} started voice chat{d}")
            }
        }
        Action::InviteToGroupCall { users } => {
            format!(
                "{from} invited {} to the voice chat",
                wrap_user_names(users, peers)
            )
        }
        Action::SetMessagesTtl { period } => {
            let text = match *period {
                604800 => "7 days",
                86400 => "24 hours",
                _ => "",
            };
            if channel {
                if *period != 0 {
                    format!("New messages will auto-delete in {text}")
                } else {
                    "New messages will not auto-delete".into()
                }
            } else if *period != 0 {
                format!("{from} has set messages to auto-delete in {text}")
            } else {
                format!("{from} has set messages not to auto-delete")
            }
        }
        Action::ChatJoinedByRequest => format!("{from} joined group by request"),
        Action::TopicCreate { title } => {
            format!("{from} created topic &laquo;{}&raquo;", esc(title))
        }
        Action::TopicEdit { title } => {
            if title.is_empty() {
                format!("{from} changed topic")
            } else {
                format!("{from} changed topic title to &laquo;{}&raquo;", esc(title))
            }
        }
        Action::BoostApply { boosts } => format!("{from} boosted the group {boosts} time(s)"),
        Action::Other(name) => format!("{from}: {}", esc(&name.replace('_', " "))),
    })
}

fn service_block(id: i64, text: &str, photo: Option<(&str, u32)>, base: &str) -> String {
    let mut o = format!(
        "<div class=\"message service\" id=\"message{id}\">\n<div class=\"body details\">\n{text}\n</div>\n"
    );
    if let Some((path, size)) = photo {
        o.push_str("<div class=\"userpic_wrap\">\n");
        o.push_str(&format!(
            "<a class=\"userpic_link\" href=\"{base}{0}\">\n<img class=\"userpic\" style=\"width: {1}px; height: {1}px\" src=\"{base}{0}\"/>\n</a>\n",
            esc(path),
            size
        ));
        o.push_str("</div>\n");
    }
    o.push_str("</div>\n");
    o
}

#[derive(Clone)]
struct Prev {
    from: Option<PeerId>,
    date: i32,
    default: bool,
    fwd_from: Option<PeerId>,
    fwd_date: i32,
}

fn needs_wrap(m: &Message, prev: Option<&Prev>) -> bool {
    match prev {
        None => true,
        Some(p) => {
            !p.default
                || m.from.is_none()
                || p.from != m.from
                || (m.date - p.date).abs() > JOIN_WITHIN_SECONDS
        }
    }
}

fn forwarded_needs_wrap(m: &Message, prev: Option<&Prev>) -> bool {
    if needs_wrap(m, prev) {
        return true;
    }
    let p = prev.unwrap();
    m.fwd_from.is_none()
        || m.fwd_from != p.fwd_from
        || !matches!(m.fwd_from, Some(PeerId::User(_)))
        || (m.fwd_date - p.fwd_date).abs() > JOIN_WITHIN_SECONDS
}

fn message_block(
    m: &Message,
    prev: Option<&Prev>,
    dialog: &DialogInfo,
    peers: &Peers,
    base: &str,
) -> (String, Prev) {
    let info = |default: bool| Prev {
        from: m.from,
        date: m.date,
        default,
        fwd_from: m.fwd_from,
        fwd_date: m.fwd_date,
    };
    if m.unsupported {
        let t = "This message is not supported by this version of the export";
        return (service_block(m.id as i64, t, None, base), info(false));
    }
    if m.action.is_some() {
        if let Some(text) = service_text(m, dialog, peers) {
            let photo = match &m.action {
                Some(Action::ChatEditPhoto { photo, .. }) if !photo.relative_path.is_empty() => {
                    Some((photo.relative_path.as_str(), 60u32))
                }
                _ => None,
            };
            return (service_block(m.id as i64, &text, photo, base), info(false));
        }
    }

    let wrap = needs_wrap(m, prev);
    let mut o = format!(
        "<div class=\"message default clearfix{}\" id=\"message{}\">\n",
        if wrap { "" } else { " joined" },
        m.id
    );
    if wrap {
        let u = peer_userpic(m.from, peers, HISTORY_USERPIC, (m.id % 8) as u8);
        o.push_str("<div class=\"pull_left userpic_wrap\">\n");
        o.push_str(&userpic_html(&u));
        o.push_str("</div>\n");
    }
    o.push_str("<div class=\"body\">\n");
    o.push_str(&format!(
        "<div class=\"pull_right date details\" title=\"{}\">\n{}\n</div>\n",
        date_time(m.date, true),
        time_text(m.date)
    ));
    let via = m
        .via_bot
        .and_then(|b| peers.get(PeerId::User(b)))
        .map(|p| p.username.clone())
        .filter(|u| !u.is_empty());
    if wrap {
        let via_text = match (&via, m.forwarded) {
            (Some(v), false) => format!(" via @{}", esc(v)),
            _ => String::new(),
        };
        o.push_str(&format!(
            "<div class=\"from_name\">\n{}{}\n</div>\n",
            esc(&display_name(m.from, peers)),
            via_text
        ));
    }
    if m.forwarded {
        let fwrap = forwarded_needs_wrap(m, prev);
        if fwrap {
            let fu = match m.fwd_from {
                Some(p) => peer_userpic(Some(p), peers, HISTORY_USERPIC, 0),
                None => Userpic {
                    color: (m.id % 8) as u8,
                    size: HISTORY_USERPIC,
                    first: m.fwd_name.clone(),
                    last: String::new(),
                    tooltip: String::new(),
                },
            };
            o.push_str("<div class=\"pull_left forwarded userpic_wrap\">\n");
            o.push_str(&userpic_html(&fu));
            o.push_str("</div>\n");
        }
        o.push_str("<div class=\"forwarded body\">\n");
        if fwrap {
            let name = match m.fwd_from {
                Some(p) => display_name(Some(p), peers),
                None if !m.fwd_name.is_empty() => m.fwd_name.clone(),
                None => "Deleted Account".into(),
            };
            o.push_str(&format!(
                "<div class=\"from_name\">\n{}{}<span class=\"date details\" title=\"{}\"> {}</span>\n</div>\n",
                esc(&name),
                via.as_ref().map(|v| format!(" via @{}", esc(v))).unwrap_or_default(),
                date_time(m.fwd_date, true),
                date_time(m.fwd_date, false)
            ));
        }
    }
    if m.reply_to != 0 {
        o.push_str("<div class=\"reply_to details\">\n");
        if m.reply_to_peer.is_some() {
            o.push_str("In reply to a message in another chat");
        } else {
            o.push_str(&format!(
                "In reply to <a href=\"#go_to_message{id}\" onclick=\"return GoToMessage({id})\">this message</a>",
                id = m.reply_to
            ));
        }
        o.push_str("\n</div>\n");
    }
    o.push_str(&media_html(m, dialog, base));
    let text = format_text(&m.text);
    if !text.is_empty() {
        o.push_str(&format!("<div class=\"text\">\n{text}\n</div>\n"));
    }
    if !m.buttons.is_empty() {
        o.push_str("<table class=\"bot_buttons_table\">\n<tbody>\n");
        for row in &m.buttons {
            o.push_str("<tr>\n<td class=\"bot_button_row\">\n");
            for (i, b) in row.iter().enumerate() {
                let href = if b.kind == "url" {
                    safe_href(&b.url)
                } else {
                    None
                };
                o.push_str("<div class=\"bot_button\">\n");
                match href {
                    Some(h) => o.push_str(&format!("<a href=\"{}\">\n", esc(&h))),
                    None => o.push_str(&format!(
                        "<a onclick=\"return ShowTextCopied({})\">\n",
                        esc(&js_str(&format!("Type: {}", b.kind)))
                    )),
                }
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
    if m.forwarded {
        o.push_str("</div>\n");
    }
    if !m.reactions.is_empty() {
        o.push_str("<span class=\"reactions\">\n");
        for r in &m.reactions {
            let emoji = if r.document_id.is_some() {
                "\u{1F44B}".to_string()
            } else {
                r.emoji.clone()
            };
            o.push_str(&format!(
                "<span class=\"reaction\">\n<span class=\"emoji\">\n{}\n</span>\n<span class=\"count\">\n{}\n</span>\n</span>\n",
                esc(&emoji),
                r.count
            ));
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
    userpic: Userpic,
    name: String,
    count: usize,
    last_date: i32,
    link: String,
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
    last_date: i32,
}

pub struct HtmlWriter {
    root: PathBuf,
    personal: Option<String>,
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

fn entry_html(
    u: &Userpic,
    name: &str,
    details: &[String],
    info: &str,
    link: &str,
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
    o.push_str(&userpic_html(u));
    o.push_str("</div>\n<div class=\"body\">\n");
    if !info.is_empty() {
        o.push_str(&format!(
            "<div class=\"pull_right info details\">\n{}\n</div>\n",
            esc(info)
        ));
    }
    if !name.is_empty() {
        o.push_str(&format!(
            "<div class=\"name bold\">\n{}\n</div>\n",
            esc(name)
        ));
    }
    for d in details {
        o.push_str(&format!(
            "<div class=\"details_entry details\">\n{}\n</div>\n",
            esc(d)
        ));
    }
    o.push_str("</div>\n");
    o.push_str(if link.is_empty() {
        "</div>\n"
    } else {
        "</a>\n"
    });
    o
}

impl HtmlWriter {
    pub fn create(root: &Path) -> io::Result<Self> {
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
            personal: None,
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
        out.write_all(
            header(&chat.title, Some(&format!("{}lists/chats.html", chat.base))).as_bytes(),
        )?;
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

    fn list_page(
        &self,
        title: &str,
        about_text: &str,
        entries: &[ChatEntry],
        file: &str,
    ) -> io::Result<()> {
        let base = "../";
        let mut o = page_start(base);
        o.push_str(&header(title, Some("../export_results.html")));
        o.push_str("<div class=\"page_body list_page\">\n");
        o.push_str(&format!(
            "<div class=\"page_about details\">\n{}\n</div>\n",
            esc_br(about_text)
        ));
        o.push_str("<div class=\"entry_list\">\n");
        for e in entries {
            o.push_str(&entry_html(
                &e.userpic,
                &e.name,
                &[format!("{} messages", e.count)],
                &date_text(e.last_date),
                &e.link,
                base,
            ));
        }
        o.push_str(PAGE_END);
        write_file(&self.root.join(file), &o)
    }
}

impl Writer for HtmlWriter {
    fn personal(
        &mut self,
        info: &PersonalInfo,
        userpics: &[crate::model::Userpic],
    ) -> io::Result<()> {
        let u = Userpic {
            color: PeerId::User(info.user_id).color_index(),
            size: PERSONAL_USERPIC,
            first: info.first_name.clone(),
            last: info.last_name.clone(),
            tooltip: String::new(),
        };
        let mut o = String::from(
            "<div class=\"personal_info clearfix\">\n<div class=\"pull_right userpic_wrap\">\n",
        );
        o.push_str(&userpic_html(&u));
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
                ("Phone number", info.phone.clone()),
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
        self.personal = Some(o);

        if !userpics.is_empty() {
            let base = "../";
            let mut p = page_start(base);
            p.push_str(&header("Profile pictures", Some("../export_results.html")));
            p.push_str("<div class=\"page_body list_page\">\n<div class=\"entry_list\">\n");
            for pic in userpics {
                p.push_str(
                    "<div class=\"entry clearfix\">\n<div class=\"pull_left userpic_wrap\">\n",
                );
                if pic.file.relative_path.is_empty() {
                    p.push_str(&userpic_html(&Userpic {
                        color: 0,
                        size: ENTRY_USERPIC,
                        first: String::new(),
                        last: String::new(),
                        tooltip: String::new(),
                    }));
                } else {
                    p.push_str(&format!(
                        "<a class=\"userpic_link\" href=\"{base}{0}\">\n<img class=\"userpic\" style=\"width: {1}px; height: {1}px\" src=\"{base}{0}\"/>\n</a>\n",
                        esc(&pic.file.relative_path),
                        ENTRY_USERPIC
                    ));
                }
                p.push_str(&format!(
                    "</div>\n<div class=\"body\">\n<div class=\"details_entry details\">\n{}\n</div>\n</div>\n</div>\n",
                    date_time(pic.date, false)
                ));
            }
            p.push_str(PAGE_END);
            write_file(&self.root.join("lists/profile_pictures.html"), &p)?;
            self.sections.push(Section {
                priority: 0,
                label: "Profile pictures",
                kind: "photos",
                count: userpics.len(),
                path: "lists/profile_pictures.html".into(),
            });
        }
        Ok(())
    }

    fn contacts(&mut self, list: &[Contact]) -> io::Result<()> {
        if list.is_empty() {
            return Ok(());
        }
        let base = "../";
        let mut o = page_start(base);
        o.push_str(&header("Contacts", Some("../export_results.html")));
        o.push_str("<div class=\"page_body list_page\">\n");
        o.push_str(&format!(
            "<div class=\"page_about details\">\n{}\n</div>\n",
            esc_br(about::ABOUT_CONTACTS)
        ));
        o.push_str("<div class=\"entry_list\">\n");
        let mut sorted: Vec<&Contact> = list.iter().collect();
        sorted.sort_by_key(|c| (c.first_name.to_lowercase(), c.last_name.to_lowercase()));
        for c in sorted {
            let color = c
                .user_id
                .map(|u| PeerId::User(u).color_index())
                .unwrap_or(0);
            let u = Userpic {
                color,
                size: ENTRY_USERPIC,
                first: c.first_name.clone(),
                last: c.last_name.clone(),
                tooltip: c.user_id.map(|u| format!("ID: {u}")).unwrap_or_default(),
            };
            let name = format!("{} {}", c.first_name, c.last_name);
            let name = if name.trim().is_empty() {
                "Deleted Account".to_string()
            } else {
                name.trim().to_string()
            };
            let info = if c.date != 0 {
                date_time(c.date, false)
            } else {
                String::new()
            };
            o.push_str(&entry_html(&u, &name, &[c.phone.clone()], &info, "", base));
        }
        o.push_str(PAGE_END);
        write_file(&self.root.join("lists/contacts.html"), &o)?;
        self.sections.push(Section {
            priority: 1,
            label: "Contacts",
            kind: "contacts",
            count: list.len(),
            path: "lists/contacts.html".into(),
        });
        Ok(())
    }

    fn dialogs_start(&mut self, left: bool) -> io::Result<()> {
        self.in_left = left;
        Ok(())
    }

    fn dialog_start(&mut self, d: &DialogInfo, _peers: &Peers) -> io::Result<()> {
        let rel = if d.relative_path.is_empty() {
            "chats/chat/".to_string()
        } else {
            d.relative_path.clone()
        };
        let dir = self.root.join(&rel);
        fs::create_dir_all(&dir)?;
        let depth = rel.trim_end_matches('/').matches('/').count() + 1;
        let title = if d.name.is_empty() {
            "Deleted Account".to_string()
        } else {
            d.name.clone()
        };
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
            last_date: 0,
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
                chat.file_index = new_index;
                chat.prev = None;
                chat.last_day = 0;
                HtmlWriter::open_chat_file(chat)?;
            }
            let day = day_key(m.date);
            if chat.last_day == 0 || day != chat.last_day {
                chat.date_id -= 1;
                chat.out.write_all(
                    service_block(chat.date_id, &date_text(m.date), None, &chat.base).as_bytes(),
                )?;
                chat.last_day = day;
            }
            let (block, info) = message_block(m, chat.prev.as_ref(), d, peers, &chat.base);
            chat.out.write_all(block.as_bytes())?;
            chat.prev = Some(info);
            chat.total += 1;
            chat.last_date = chat.last_date.max(m.date);
        }
        Ok(())
    }

    fn dialog_end(&mut self) -> io::Result<()> {
        let Some(mut chat) = self.chat.take() else {
            return Ok(());
        };
        if chat.total == 0 {
            chat.out.write_all(
                service_block(-1, "No exported messages", None, &chat.base).as_bytes(),
            )?;
        }
        chat.out.write_all(PAGE_END.as_bytes())?;
        chat.out.flush()?;
        let entry = ChatEntry {
            userpic: Userpic {
                color: 0,
                size: ENTRY_USERPIC,
                first: chat.title.clone(),
                last: String::new(),
                tooltip: String::new(),
            },
            name: chat.title.clone(),
            count: chat.total,
            last_date: chat.last_date,
            link: format!("{}messages.html", chat.rel_dir),
        };
        let _ = &chat.dir;
        if self.in_left {
            self.left_entries.push(entry)
        } else {
            self.entries.push(entry)
        }
        Ok(())
    }

    fn dialogs_end(&mut self) -> io::Result<()> {
        if self.in_left {
            if !self.left_entries.is_empty() {
                self.list_page(
                    "Left chats",
                    about::ABOUT_LEFT_CHATS,
                    &self.left_entries,
                    "lists/left_chats.html",
                )?;
                self.sections.push(Section {
                    priority: 3,
                    label: "Left chats",
                    kind: "chats",
                    count: self.left_entries.len(),
                    path: "lists/left_chats.html".into(),
                });
            }
        } else if !self.entries.is_empty() {
            self.list_page(
                "Chats",
                about::ABOUT_CHATS,
                &self.entries,
                "lists/chats.html",
            )?;
            self.sections.push(Section {
                priority: 2,
                label: "Chats",
                kind: "chats",
                count: self.entries.len(),
                path: "lists/chats.html".into(),
            });
        }
        Ok(())
    }

    fn finish(mut self: Box<Self>) -> io::Result<()> {
        let mut o = page_start("");
        o.push_str(&header("Exported Data", None));
        o.push_str("<div class=\"page_body\">\n");
        let has_personal = self.personal.is_some();
        if let Some(p) = &self.personal {
            o.push_str(p);
        }
        self.sections.sort_by_key(|s| s.priority);
        if !self.sections.is_empty() {
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
        o.push_str(&format!(
            "<div class=\"page_about details with_divider\">\n{}\n</div>\n",
            esc_br(about::ABOUT_TELEGRAM)
        ));
        o.push_str("</div>\n</div>\n</body>\n</html>\n");
        write_file(&self.root.join("export_results.html"), &o)
    }
}
