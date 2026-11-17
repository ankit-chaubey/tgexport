//! Writer for `result.json`. It follows the layout Telegram Desktop produces:
//! one space of indentation per level, empty values left out, coordinates with
//! six decimals and dates in local time.

use std::io::{self, Write};

use crate::model::*;

pub enum J {
    Raw(String),
    Str(String),
    Null,
    Arr(Vec<J>),
    Obj(Vec<(&'static str, J)>),
}

pub fn s(v: impl Into<String>) -> J {
    J::Str(v.into())
}

pub fn n(v: i64) -> J {
    J::Raw(v.to_string())
}

pub fn b(v: bool) -> J {
    J::Raw(if v { "true" } else { "false" }.to_string())
}

pub fn f(v: f64) -> J {
    J::Raw(format!("{v:.6}"))
}

fn string_allow_null(v: String) -> J {
    if v.is_empty() { J::Null } else { J::Str(v) }
}

pub fn escape(value: &str, out: &mut String) {
    out.push('"');
    for c in value.chars() {
        match c {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            c if (c as u32) < 32 => out.push_str(&format!("\\u{:04X}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn indent(n: usize, out: &mut String) {
    for _ in 0..n {
        out.push(' ');
    }
}

/// Writes `j` as it appears at nesting level `depth`.
pub fn ser(j: &J, depth: usize, out: &mut String) {
    match j {
        J::Raw(r) => out.push_str(r),
        J::Str(v) => escape(v, out),
        J::Null => out.push_str("null"),
        J::Arr(items) => {
            out.push('[');
            let mut first = true;
            for it in items {
                if !first {
                    out.push(',');
                }
                first = false;
                out.push('\n');
                indent(depth + 1, out);
                ser(it, depth + 1, out);
            }
            out.push('\n');
            indent(depth, out);
            out.push(']');
        }
        J::Obj(pairs) => {
            out.push('{');
            let mut first = true;
            for (k, v) in pairs {
                if !first {
                    out.push(',');
                }
                first = false;
                out.push('\n');
                indent(depth + 1, out);
                escape(k, out);
                out.push_str(": ");
                ser(v, depth + 1, out);
            }
            out.push('\n');
            indent(depth, out);
            out.push('}');
        }
    }
}

/// Writes JSON piece by piece, so a huge chat never has to fit in memory.
/// It keeps track of nesting to place commas and indentation.
pub struct JsonStream<W: Write> {
    out: W,
    stack: Vec<(bool, bool)>, // (is an object, already has an item)
}

impl<W: Write> JsonStream<W> {
    pub fn new(out: W) -> Self {
        JsonStream {
            out,
            stack: Vec::new(),
        }
    }

    fn sep(&mut self) -> io::Result<()> {
        let depth = self.stack.len();
        if let Some(top) = self.stack.last_mut() {
            if top.1 {
                self.out.write_all(b",")?;
            }
            top.1 = true;
        }
        let mut pad = String::from("\n");
        indent(depth, &mut pad);
        self.out.write_all(pad.as_bytes())
    }

    pub fn begin_object(&mut self) -> io::Result<()> {
        self.out.write_all(b"{")?;
        self.stack.push((true, false));
        Ok(())
    }

    pub fn begin_array(&mut self) -> io::Result<()> {
        self.out.write_all(b"[")?;
        self.stack.push((false, false));
        Ok(())
    }

    pub fn end(&mut self) -> io::Result<()> {
        let (is_obj, _) = self.stack.pop().unwrap_or((true, false));
        let mut pad = String::from("\n");
        indent(self.stack.len(), &mut pad);
        pad.push(if is_obj { '}' } else { ']' });
        self.out.write_all(pad.as_bytes())
    }

    /// Writes `"key": ` and leaves the value to the caller.
    pub fn key(&mut self, key: &str) -> io::Result<()> {
        self.sep()?;
        let mut k = String::new();
        escape(key, &mut k);
        k.push_str(": ");
        self.out.write_all(k.as_bytes())
    }

    pub fn field(&mut self, key: &str, v: &J) -> io::Result<()> {
        self.key(key)?;
        self.value(v)
    }

    /// Starts a new array element; the caller writes the value next.
    pub fn item(&mut self) -> io::Result<()> {
        self.sep()
    }

    pub fn value(&mut self, v: &J) -> io::Result<()> {
        let mut buf = String::new();
        ser(v, self.stack.len(), &mut buf);
        self.out.write_all(buf.as_bytes())
    }

    pub fn array_item(&mut self, v: &J) -> io::Result<()> {
        self.item()?;
        self.value(v)
    }

    pub fn finish(mut self) -> io::Result<()> {
        self.out.write_all(b"\n")?;
        self.out.flush()
    }
}

fn kind_name(k: TextKind) -> &'static str {
    match k {
        TextKind::Unknown => "unknown",
        TextKind::Mention => "mention",
        TextKind::Hashtag => "hashtag",
        TextKind::BotCommand => "bot_command",
        TextKind::Url => "link",
        TextKind::Email => "email",
        TextKind::Bold => "bold",
        TextKind::Italic => "italic",
        TextKind::Code => "code",
        TextKind::Pre => "pre",
        TextKind::Text => "plain",
        TextKind::TextUrl => "text_link",
        TextKind::MentionName => "mention_name",
        TextKind::Phone => "phone",
        TextKind::Cashtag => "cashtag",
        TextKind::Underline => "underline",
        TextKind::Strike => "strikethrough",
        TextKind::Blockquote => "blockquote",
        TextKind::BankCard => "bank_card",
        TextKind::TonAddress => "ton_address",
        TextKind::Spoiler => "spoiler",
        TextKind::CustomEmoji => "custom_emoji",
    }
}

pub fn text_json(data: &Text, to_objects: bool) -> J {
    if data.is_empty() {
        return if to_objects {
            J::Raw("[]".into())
        } else {
            s("")
        };
    }
    let mut items = Vec::new();
    for p in data {
        if p.kind == TextKind::Text && !to_objects {
            items.push(s(p.text.clone()));
            continue;
        }
        let mut pairs = vec![("type", s(kind_name(p.kind))), ("text", s(p.text.clone()))];
        match p.kind {
            TextKind::MentionName => pairs.push(("user_id", J::Raw(p.additional.clone()))),
            TextKind::CustomEmoji => pairs.push(("document_id", s(p.additional.clone()))),
            TextKind::Pre => pairs.push(("language", s(p.additional.clone()))),
            TextKind::TextUrl => pairs.push(("href", s(p.additional.clone()))),
            TextKind::Blockquote => pairs.push(("collapsed", b(!p.additional.is_empty()))),
            _ => {}
        }
        items.push(J::Obj(pairs));
    }
    if !to_objects && data.len() == 1 && data[0].kind == TextKind::Text {
        return items.remove(0);
    }
    J::Arr(items)
}

fn path_value(file: &FileRef) -> Option<String> {
    if !file.relative_path.is_empty() {
        return Some(file.relative_path.clone());
    }
    match file.skip {
        Some(SkipReason::Unavailable) => Some("(File unavailable, please try again later)".into()),
        Some(SkipReason::FileSize) => {
            Some("(File exceeds maximum size. Change data exporting settings to download.)".into())
        }
        Some(SkipReason::FileType) => {
            Some("(File not included. Change data exporting settings to download.)".into())
        }
        None => None,
    }
}

fn format_phone(p: &str) -> String {
    if p.is_empty() || p.starts_with('+') {
        p.to_string()
    } else {
        format!("+{p}")
    }
}

pub fn message_json(m: &Message, peers: &Peers) -> J {
    if m.unsupported {
        return J::Obj(vec![("id", n(m.id as i64)), ("type", s("unsupported"))]);
    }
    let mut v: Vec<(&'static str, J)> = vec![
        ("id", n(m.id as i64)),
        (
            "type",
            s(if m.action.is_some() {
                "service"
            } else {
                "message"
            }),
        ),
        ("date", s(iso_date(m.date))),
        ("date_unixtime", s(m.date.to_string())),
    ];
    if m.edited != 0 {
        v.push(("edited", s(iso_date(m.edited))));
        v.push(("edited_unixtime", s(m.edited.to_string())));
    }
    let name_of = |p: PeerId| string_allow_null(peers.name(p));
    let user_names = |ids: &[i64]| {
        J::Arr(
            ids.iter()
                .map(|u| string_allow_null(peers.name(PeerId::User(*u))))
                .collect(),
        )
    };
    let push_path = |v: &mut Vec<(&'static str, J)>, key: &'static str, file: &FileRef| {
        if let Some(p) = path_value(file) {
            v.push((key, s(p)));
        }
    };
    let push_from =
        |v: &mut Vec<(&'static str, J)>, label: &'static str, id_label: &'static str| {
            if let Some(f) = m.from {
                v.push((label, name_of(f)));
                v.push((id_label, s(f.tag())));
            }
        };
    let reply = |v: &mut Vec<(&'static str, J)>, label: &'static str| {
        if m.reply_to != 0 {
            v.push((label, n(m.reply_to as i64)));
            if let Some(p) = m.reply_to_peer {
                v.push(("reply_to_peer_id", s(p.tag())));
            }
        }
    };

    if let Some(a) = &m.action {
        let actor = |v: &mut Vec<(&'static str, J)>| push_from(v, "actor", "actor_id");
        match a {
            Action::ChatCreate { title, users } => {
                actor(&mut v);
                v.push(("action", s("create_group")));
                v.push(("title", s(title.clone())));
                v.push(("members", user_names(users)));
            }
            Action::ChatEditTitle { title } => {
                actor(&mut v);
                v.push(("action", s("edit_group_title")));
                v.push(("title", s(title.clone())));
            }
            Action::ChatEditPhoto {
                photo,
                width,
                height,
            } => {
                actor(&mut v);
                v.push(("action", s("edit_group_photo")));
                push_path(&mut v, "photo", photo);
                v.push(("photo_file_size", n(photo.size)));
                if *width != 0 && *height != 0 {
                    v.push(("width", n(*width as i64)));
                    v.push(("height", n(*height as i64)));
                }
            }
            Action::ChatDeletePhoto => {
                actor(&mut v);
                v.push(("action", s("delete_group_photo")));
            }
            Action::ChatAddUser { users } => {
                actor(&mut v);
                v.push(("action", s("invite_members")));
                v.push(("members", user_names(users)));
            }
            Action::ChatDeleteUser { user } => {
                actor(&mut v);
                v.push(("action", s("remove_members")));
                v.push(("members", user_names(&[*user])));
            }
            Action::ChatJoinedByLink { inviter } => {
                actor(&mut v);
                v.push(("action", s("join_group_by_link")));
                v.push((
                    "inviter",
                    string_allow_null(peers.name(PeerId::User(*inviter))),
                ));
            }
            Action::ChannelCreate { title } => {
                actor(&mut v);
                v.push(("action", s("create_channel")));
                v.push(("title", s(title.clone())));
            }
            Action::ChatMigrateTo => {
                actor(&mut v);
                v.push(("action", s("migrate_to_supergroup")));
            }
            Action::ChannelMigrateFrom { title } => {
                actor(&mut v);
                v.push(("action", s("migrate_from_group")));
                v.push(("title", s(title.clone())));
            }
            Action::PinMessage => {
                actor(&mut v);
                v.push(("action", s("pin_message")));
                reply(&mut v, "message_id");
            }
            Action::HistoryClear => {
                actor(&mut v);
                v.push(("action", s("clear_history")));
            }
            Action::GameScore { score } => {
                actor(&mut v);
                v.push(("action", s("score_in_game")));
                reply(&mut v, "game_message_id");
                v.push(("score", n(*score as i64)));
            }
            Action::PaymentSent { amount, currency } => {
                v.push(("action", s("send_payment")));
                v.push(("amount", n(*amount)));
                v.push(("currency", s(currency.clone())));
                reply(&mut v, "invoice_message_id");
            }
            Action::PhoneCall {
                duration,
                discard_reason,
            } => {
                actor(&mut v);
                v.push(("action", s("phone_call")));
                if *duration != 0 {
                    v.push(("duration_seconds", n(*duration as i64)));
                }
                if !discard_reason.is_empty() {
                    v.push(("discard_reason", s(discard_reason.clone())));
                }
            }
            Action::ScreenshotTaken => {
                actor(&mut v);
                v.push(("action", s("take_screenshot")));
            }
            Action::CustomAction { message } => {
                actor(&mut v);
                v.push(("information_text", s(message.clone())));
            }
            Action::BotAllowed { domain } => {
                v.push(("action", s("allow_sending_messages")));
                v.push(("reason_domain", s(domain.clone())));
            }
            Action::ContactSignUp => {
                actor(&mut v);
                v.push(("action", s("joined_telegram")));
            }
            Action::GroupCall { duration } => {
                actor(&mut v);
                v.push(("action", s("group_call")));
                if *duration != 0 {
                    v.push(("duration", n(*duration as i64)));
                }
            }
            Action::InviteToGroupCall { users } => {
                actor(&mut v);
                v.push(("action", s("invite_to_group_call")));
                v.push(("members", user_names(users)));
            }
            Action::SetMessagesTtl { period } => {
                actor(&mut v);
                v.push(("action", s("set_messages_ttl")));
                v.push(("period", n(*period as i64)));
            }
            Action::ChatJoinedByRequest => {
                actor(&mut v);
                v.push(("action", s("join_group_by_request")));
            }
            Action::TopicCreate { title } => {
                actor(&mut v);
                v.push(("action", s("topic_created")));
                v.push(("title", s(title.clone())));
            }
            Action::TopicEdit { title } => {
                actor(&mut v);
                v.push(("action", s("topic_edit")));
                if !title.is_empty() {
                    v.push(("new_title", s(title.clone())));
                }
            }
            Action::BoostApply { boosts } => {
                actor(&mut v);
                v.push(("action", s("boost_apply")));
                v.push(("boosts", n(*boosts as i64)));
            }
            Action::Other(name) => {
                actor(&mut v);
                v.push(("action", s(name.clone())));
            }
        }
    } else {
        push_from(&mut v, "from", "from_id");
        if !m.signature.is_empty() {
            v.push(("author", s(m.signature.clone())));
        }
        if let Some(p) = m.fwd_from {
            v.push(("forwarded_from", name_of(p)));
            v.push(("forwarded_from_id", s(p.tag())));
        } else if !m.fwd_name.is_empty() {
            v.push(("forwarded_from", s(m.fwd_name.clone())));
        }
        if let Some(p) = m.saved_from {
            v.push(("saved_from", name_of(p)));
        }
        reply(&mut v, "reply_to_message_id");
        if let Some(bot) = m.via_bot {
            if let Some(info) = peers.get(PeerId::User(bot)) {
                if !info.username.is_empty() {
                    v.push(("via_bot", s(format!("@{}", info.username))));
                }
            }
        }
        match &m.media {
            Media::None | Media::Unsupported => {}
            Media::Photo {
                file,
                width,
                height,
                spoiler,
                ttl,
            } => {
                push_path(&mut v, "photo", file);
                v.push(("photo_file_size", n(file.size)));
                if *width != 0 && *height != 0 {
                    v.push(("width", n(*width as i64)));
                    v.push(("height", n(*height as i64)));
                }
                if *spoiler {
                    v.push(("media_spoiler", b(true)));
                }
                if *ttl != 0 {
                    v.push(("self_destruct_period_seconds", n(*ttl as i64)));
                }
            }
            Media::Document {
                file,
                name,
                mime,
                duration,
                width,
                height,
                flags,
                sticker_emoji,
                performer,
                title,
                spoiler,
                ttl,
            } => {
                push_path(&mut v, "file", file);
                if !name.is_empty() {
                    v.push(("file_name", s(name.clone())));
                }
                v.push(("file_size", n(file.size)));
                let ty = if flags.sticker {
                    "sticker"
                } else if flags.video_message {
                    "video_message"
                } else if flags.voice {
                    "voice_message"
                } else if flags.animated {
                    "animation"
                } else if flags.video_file {
                    "video_file"
                } else if flags.audio {
                    "audio_file"
                } else {
                    ""
                };
                if !ty.is_empty() {
                    v.push(("media_type", s(ty)));
                }
                if flags.sticker && !sticker_emoji.is_empty() {
                    v.push(("sticker_emoji", s(sticker_emoji.clone())));
                }
                if flags.audio {
                    if !performer.is_empty() {
                        v.push(("performer", s(performer.clone())));
                    }
                    if !title.is_empty() {
                        v.push(("title", s(title.clone())));
                    }
                }
                if !mime.is_empty() {
                    v.push(("mime_type", s(mime.clone())));
                }
                if *duration != 0 {
                    v.push(("duration_seconds", n(*duration as i64)));
                }
                if *width != 0 && *height != 0 {
                    v.push(("width", n(*width as i64)));
                    v.push(("height", n(*height as i64)));
                }
                if *spoiler {
                    v.push(("media_spoiler", b(true)));
                }
                if *ttl != 0 {
                    v.push(("self_destruct_period_seconds", n(*ttl as i64)));
                }
            }
            Media::Contact {
                first_name,
                last_name,
                phone,
                vcard,
            } => {
                v.push((
                    "contact_information",
                    J::Obj(vec![
                        ("first_name", s(first_name.clone())),
                        ("last_name", s(last_name.clone())),
                        ("phone_number", s(format_phone(phone))),
                    ]),
                ));
                if let Some(vc) = vcard {
                    push_path(&mut v, "contact_vcard", vc);
                    v.push(("contact_vcard_file_size", n(vc.size)));
                }
            }
            Media::Geo { point, live_period } => {
                v.push((
                    "location_information",
                    match point {
                        Some((la, lo)) => J::Obj(vec![("latitude", f(*la)), ("longitude", f(*lo))]),
                        None => J::Null,
                    },
                ));
                if *live_period != 0 {
                    v.push(("live_location_period_seconds", n(*live_period as i64)));
                }
            }
            Media::Venue {
                point,
                title,
                address,
            } => {
                if !title.is_empty() {
                    v.push(("place_name", s(title.clone())));
                }
                if !address.is_empty() {
                    v.push(("address", s(address.clone())));
                }
                if let Some((la, lo)) = point {
                    v.push((
                        "location_information",
                        J::Obj(vec![("latitude", f(*la)), ("longitude", f(*lo))]),
                    ));
                }
            }
            Media::Game {
                title, description, ..
            } => {
                v.push(("game_title", s(title.clone())));
                v.push(("game_description", s(description.clone())));
            }
            Media::Invoice {
                title,
                description,
                amount,
                currency,
                receipt_msg_id,
            } => {
                let mut o = vec![
                    ("title", s(title.clone())),
                    ("description", s(description.clone())),
                    ("amount", n(*amount)),
                    ("currency", s(currency.clone())),
                ];
                if *receipt_msg_id != 0 {
                    o.push(("receipt_message_id", n(*receipt_msg_id as i64)));
                }
                v.push(("invoice_information", J::Obj(o)));
            }
            Media::Poll(p) => {
                let answers = J::Arr(
                    p.answers
                        .iter()
                        .map(|(t, votes, chosen)| {
                            J::Obj(vec![
                                ("text", text_json(t, false)),
                                ("voters", n(*votes as i64)),
                                ("chosen", b(*chosen)),
                            ])
                        })
                        .collect(),
                );
                v.push((
                    "poll",
                    J::Obj(vec![
                        ("question", text_json(&p.question, false)),
                        ("closed", b(p.closed)),
                        ("total_voters", n(p.total_votes as i64)),
                        ("answers", answers),
                    ]),
                ));
            }
            Media::Dice { emoji, value } => {
                v.push(("dice_emoji", s(emoji.clone())));
                v.push(("dice_value", n(*value as i64)));
            }
        }
    }

    v.push(("text", text_json(&m.text, false)));
    v.push(("text_entities", text_json(&m.text, true)));

    if !m.buttons.is_empty() {
        let rows = m
            .buttons
            .iter()
            .map(|row| {
                J::Arr(
                    row.iter()
                        .map(|bt| {
                            let mut o =
                                vec![("type", s(bt.kind.clone())), ("text", s(bt.text.clone()))];
                            if bt.kind == "callback" {
                                use std::fmt::Write as _;
                                let mut hex = String::new();
                                for byte in &bt.data {
                                    let _ = write!(hex, "{byte:02x}");
                                }
                                o.push(("data", s(hex)));
                            } else if !bt.url.is_empty() {
                                o.push(("data", s(bt.url.clone())));
                            }
                            J::Obj(o)
                        })
                        .collect(),
                )
            })
            .collect();
        v.push(("inline_bot_buttons", J::Arr(rows)));
    }

    if !m.reactions.is_empty() {
        let list = m
            .reactions
            .iter()
            .map(|r| {
                let mut o = vec![("count", n(r.count as i64))];
                if let Some(id) = r.document_id {
                    o.insert(0, ("type", s("custom_emoji")));
                    o.push(("document_id", s(id.to_string())));
                } else {
                    o.insert(0, ("type", s("emoji")));
                    o.push(("emoji", s(r.emoji.clone())));
                }
                J::Obj(o)
            })
            .collect();
        v.push(("reactions", J::Arr(list)));
    }
    J::Obj(v)
}
