//! JSON for rich messages, following Telegram Desktop's serializer.

use crate::json_out::*;
use crate::model::*;
use crate::rich::*;

type Fields = Vec<(&'static str, J)>;

/// Rich arrays are written as `[]` when empty, unlike the other arrays.
fn arr(items: Vec<J>) -> J {
    if items.is_empty() {
        J::Raw("[]".into())
    } else {
        J::Arr(items)
    }
}

fn num_string(v: u64) -> J {
    s(v.to_string())
}

/// Why a file is missing, or where it was saved.
fn availability(
    values: &mut Fields,
    file: Option<&FileRef>,
    path_key: &'static str,
    reason_key: &'static str,
) {
    if let Some(file) = file {
        match file.skip {
            Some(SkipReason::Unavailable) => {
                values.push((reason_key, s("unavailable")));
                return;
            }
            Some(SkipReason::FileType) => {
                values.push((reason_key, s("file_type")));
                return;
            }
            Some(SkipReason::FileSize) => {
                values.push((reason_key, s("file_size")));
                return;
            }
            None => {
                if !file.relative_path.is_empty() {
                    values.push((path_key, s(file.relative_path.clone())));
                    return;
                }
            }
        }
    }
    values.push((reason_key, s("unavailable")));
}

fn photo_metadata(
    values: &mut Fields,
    photo: Option<&RichPhoto>,
    keys: (
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        &'static str,
    ),
) {
    let (path_key, reason_key, size_key, width_key, height_key) = keys;
    availability(values, photo.map(|p| &p.file), path_key, reason_key);
    let Some(photo) = photo else { return };
    values.push((size_key, n(photo.file.size)));
    if photo.width != 0 && photo.height != 0 {
        values.push((width_key, n(photo.width as i64)));
        values.push((height_key, n(photo.height as i64)));
    }
}

fn document_metadata(
    values: &mut Fields,
    document: Option<&RichDocument>,
    include_dimensions: bool,
) {
    availability(
        values,
        document.map(|d| &d.file),
        "file",
        "file_skip_reason",
    );
    let Some(d) = document else { return };
    if !d.name.is_empty() {
        values.push(("file_name", s(d.name.clone())));
    }
    values.push(("file_size", n(d.file.size)));
    if let Some(t) = &d.thumb {
        availability(values, Some(&t.file), "thumbnail", "thumbnail_skip_reason");
        values.push(("thumbnail_file_size", n(t.file.size)));
    }
    if d.flags.sticker {
        values.push(("media_type", s("sticker")));
        if !d.sticker_emoji.is_empty() {
            values.push(("sticker_emoji", s(d.sticker_emoji.clone())));
        }
    } else if d.flags.video_message {
        values.push(("media_type", s("video_message")));
    } else if d.flags.voice {
        values.push(("media_type", s("voice_message")));
    } else if d.flags.animated {
        values.push(("media_type", s("animation")));
    } else if d.flags.video_file {
        values.push(("media_type", s("video_file")));
    } else if d.flags.audio {
        values.push(("media_type", s("audio_file")));
        if !d.performer.is_empty() {
            values.push(("performer", s(d.performer.clone())));
        }
        if !d.title.is_empty() {
            values.push(("title", s(d.title.clone())));
        }
    }
    if !d.mime.is_empty() {
        values.push(("mime_type", s(d.mime.clone())));
    }
    if d.duration != 0 {
        values.push(("duration_seconds", n(d.duration as i64)));
    }
    if include_dimensions && d.width != 0 && d.height != 0 {
        values.push(("width", n(d.width as i64)));
        values.push(("height", n(d.height as i64)));
    }
}

fn text_kind_name(k: RichTextKind) -> &'static str {
    use RichTextKind as K;
    match k {
        K::Empty => "empty",
        K::Plain => "plain",
        K::Concat => "concat",
        K::Bold => "bold",
        K::Italic => "italic",
        K::Underline => "underline",
        K::Strike => "strikethrough",
        K::Fixed => "code",
        K::Url => "text_link",
        K::Email => "email",
        K::Phone => "phone",
        K::Subscript => "subscript",
        K::Superscript => "superscript",
        K::Marked => "marked",
        K::Anchor => "anchor",
        K::Math => "math",
        K::CustomEmoji => "custom_emoji",
        K::Spoiler => "spoiler",
        K::Mention => "mention",
        K::Hashtag => "hashtag",
        K::BotCommand => "bot_command",
        K::Cashtag => "cashtag",
        K::AutoUrl => "link",
        K::AutoEmail => "email",
        K::AutoPhone => "phone",
        K::BankCard => "bank_card",
        K::TonAddress => "ton_address",
        K::MentionName => "mention_name",
        K::FormattedDate => "formatted_date",
        K::InlineImage => "inline_image",
        K::Diff => "diff",
        K::Button => "button",
    }
}

fn action_kind_name(k: InlineActionKind) -> &'static str {
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

/// Base64 without padding, with `-` and `_` (the URL safe alphabet).
fn base64_url(data: &[u8]) -> String {
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

pub fn button_action(a: &InlineAction) -> J {
    use InlineActionKind as K;
    let mut v: Fields = vec![("type", s(action_kind_name(a.kind)))];
    match a.kind {
        K::Url | K::WebView => v.push(("url", s(a.url.clone()))),
        K::Auth => {
            v.push(("url", s(a.url.clone())));
            if let Some(t) = &a.forward_text {
                v.push(("forward_text", s(t.clone())));
            }
            v.push(("button_id", n(a.button_id as i64)));
        }
        K::Callback | K::CallbackWithPassword => {
            v.push(("requires_password", b(a.requires_password)));
            v.push(("dataBase64", s(base64_url(&a.callback_data))));
            v.push(("data", s("")));
        }
        K::Game | K::Buy | K::Disabled => {}
        K::SwitchInline | K::SwitchInlineSame => {
            v.push(("query", s(a.query.clone())));
            v.push(("same_peer", b(a.same_peer)));
            if let Some(types) = &a.peer_types {
                v.push((
                    "peer_types",
                    arr(types.iter().map(|t| s(peer_type_name(*t))).collect()),
                ));
            }
        }
        K::UserProfile => v.push(("user_id", num_string(a.user_id))),
        K::CopyText => v.push(("copy_text", s(a.copy_text.clone()))),
    }
    J::Obj(v)
}

fn first_child(children: &[RichText], m: &RichMessage) -> J {
    match children.first() {
        Some(c) => rich_text(m, c),
        None => rich_text(m, &RichText::default()),
    }
}

fn button_fields(values: &mut Fields, m: &RichMessage, button: &RichText) {
    values.push(("text", first_child(&button.children, m)));
    if let Some(payload) = &button.button {
        values.push(("button", button_action(&payload.action)));
        if let Some(style) = payload.style {
            values.push(("style", s(style_name(style))));
        }
    }
}

pub fn rich_texts(m: &RichMessage, list: &[RichText]) -> J {
    arr(list.iter().map(|t| rich_text(m, t)).collect())
}

pub fn rich_text(m: &RichMessage, d: &RichText) -> J {
    use RichTextKind as K;
    let mut v: Fields = vec![("type", s(text_kind_name(d.kind)))];
    match d.kind {
        K::Empty => {}
        K::Plain => v.push(("text", s(d.text.clone()))),
        K::Concat => v.push(("text", rich_texts(m, &d.children))),
        K::Bold
        | K::Italic
        | K::Underline
        | K::Strike
        | K::Fixed
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
        | K::TonAddress => v.push(("text", first_child(&d.children, m))),
        K::Url => {
            v.push(("href", s(d.data.clone())));
            if d.id != 0 {
                v.push(("webpage_id", num_string(d.id)));
            }
            v.push(("text", first_child(&d.children, m)));
        }
        K::Email => {
            v.push(("email", s(d.data.clone())));
            v.push(("text", first_child(&d.children, m)));
        }
        K::Phone => {
            v.push(("phone", s(d.data.clone())));
            v.push(("text", first_child(&d.children, m)));
        }
        K::Anchor => {
            v.push(("name", s(d.data.clone())));
            v.push(("text", first_child(&d.children, m)));
        }
        K::Math => v.push(("source", s(d.data.clone()))),
        K::CustomEmoji => {
            v.push(("text", s(d.text.clone())));
            v.push(("document_id", s(d.custom_emoji_data.clone())));
        }
        K::MentionName => {
            v.push(("user_id", num_string(d.id)));
            v.push(("text", first_child(&d.children, m)));
        }
        K::FormattedDate => {
            v.push(("text", first_child(&d.children, m)));
            v.push(("date", s(iso_date(d.date))));
            v.push(("date_unixtime", s(d.date.to_string())));
            v.push(("relative", b(d.relative)));
            v.push(("short_time", b(d.short_time)));
            v.push(("long_time", b(d.long_time)));
            v.push(("short_date", b(d.short_date)));
            v.push(("long_date", b(d.long_date)));
            v.push(("day_of_week", b(d.day_of_week)));
        }
        K::InlineImage => {
            v.push(("document_id", num_string(d.id)));
            document_metadata(&mut v, m.documents.get(&d.id), false);
            v.push(("width", n(d.width as i64)));
            v.push(("height", n(d.height as i64)));
        }
        K::Diff => {
            v.push(("unsupported", J::Raw("true".into())));
            v.push(("text", first_child(&d.children, m)));
            v.push(("old_text", first_child(&d.old_children, m)));
        }
        K::Button => button_fields(&mut v, m, d),
    }
    J::Obj(v)
}

fn button_object(m: &RichMessage, button: &RichText) -> J {
    let mut v: Fields = Vec::new();
    button_fields(&mut v, m, button);
    J::Obj(v)
}

fn caption_object(m: &RichMessage, c: &RichCaption) -> J {
    J::Obj(vec![
        ("text", rich_text(m, &c.text)),
        ("credit", rich_text(m, &c.credit)),
    ])
}

fn task_state_name(t: TaskState) -> &'static str {
    match t {
        TaskState::None => "none",
        TaskState::Unchecked => "unchecked",
        TaskState::Checked => "checked",
    }
}

fn list_kind_name(k: ListKind) -> &'static str {
    match k {
        ListKind::Bullet => "bullet",
        ListKind::Ordered => "ordered",
    }
}

fn item_content_name(c: ItemContent) -> &'static str {
    match c {
        ItemContent::Text => "text",
        ItemContent::Blocks => "blocks",
    }
}

fn quote_content_name(c: QuoteContent) -> &'static str {
    match c {
        QuoteContent::Text => "text",
        QuoteContent::Blocks => "blocks",
    }
}

fn align_name(a: TableAlign) -> &'static str {
    match a {
        TableAlign::Left => "left",
        TableAlign::Center => "center",
        TableAlign::Right => "right",
    }
}

fn valign_name(a: TableValign) -> &'static str {
    match a {
        TableValign::Top => "top",
        TableValign::Middle => "middle",
        TableValign::Bottom => "bottom",
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

fn list_item(m: &RichMessage, d: &RichListItem) -> J {
    let mut v: Fields = vec![
        ("task_state", s(task_state_name(d.task_state))),
        ("content", s(item_content_name(d.content))),
    ];
    match d.content {
        ItemContent::Text => {
            let text = match &d.text {
                Some(t) => rich_text(m, t),
                None => rich_text(m, &RichText::default()),
            };
            v.push(("text", text));
        }
        ItemContent::Blocks => v.push(("blocks", rich_blocks(m, &d.blocks))),
    }
    if let Some(num) = &d.num {
        v.push(("num", s(num.clone())));
    }
    if let Some(value) = d.value {
        v.push(("value", n(value as i64)));
    }
    if let Some(kind) = &d.kind {
        v.push(("item_type", s(kind.clone())));
    }
    J::Obj(v)
}

fn table_cell(m: &RichMessage, d: &TableCell) -> J {
    let mut v: Fields = Vec::new();
    if let Some(t) = &d.text {
        v.push(("text", rich_text(m, t)));
    }
    if let Some(c) = d.colspan {
        v.push(("colspan", n(c as i64)));
    }
    if let Some(r) = d.rowspan {
        v.push(("rowspan", n(r as i64)));
    }
    v.push(("header", b(d.header)));
    v.push(("align", s(align_name(d.align))));
    v.push(("vertical_align", s(valign_name(d.valign))));
    J::Obj(v)
}

fn table_row(m: &RichMessage, d: &TableRow) -> J {
    J::Obj(vec![(
        "cells",
        arr(d.cells.iter().map(|c| table_cell(m, c)).collect()),
    )])
}

fn related_article(m: &RichMessage, d: &RelatedArticle) -> J {
    let mut v: Fields = vec![
        ("url", s(d.url.clone())),
        ("webpage_id", num_string(d.webpage_id)),
    ];
    if let Some(t) = &d.title {
        v.push(("title", s(t.clone())));
    }
    if let Some(t) = &d.description {
        v.push(("description", s(t.clone())));
    }
    if let Some(id) = d.photo_id {
        v.push(("photo_id", num_string(id)));
        photo_metadata(
            &mut v,
            m.photos.get(&id),
            (
                "photo",
                "photo_skip_reason",
                "photo_file_size",
                "photo_width",
                "photo_height",
            ),
        );
    }
    if let Some(a) = &d.author {
        v.push(("author", s(a.clone())));
    }
    if let Some(date) = d.published_date {
        v.push(("published_date", s(iso_date(date))));
        v.push(("published_date_unixtime", s(date.to_string())));
    }
    J::Obj(v)
}

fn channel_object(d: &RichChannel) -> J {
    let mut v: Fields = vec![
        ("source_type", s(channel_source_name(d.source))),
        ("id", num_string(d.id)),
    ];
    if let Some(h) = d.access_hash {
        v.push(("access_hash", s(h.to_string())));
    }
    if let Some(t) = &d.title {
        v.push(("title", s(t.clone())));
    }
    if let Some(u) = &d.username {
        v.push(("username", s(u.clone())));
    }
    if matches!(
        d.source,
        ChannelSource::Channel | ChannelSource::ChannelForbidden
    ) {
        v.push(("broadcast", b(d.broadcast)));
        v.push(("megagroup", b(d.megagroup)));
        v.push(("monoforum", b(d.monoforum)));
    }
    J::Obj(v)
}

fn map_point(d: &MapPoint) -> J {
    let mut v: Fields = vec![("source_type", s(map_source_name(d.source)))];
    match d.source {
        MapSource::GeoPointEmpty | MapSource::InputGeoPointEmpty => {}
        MapSource::GeoPoint | MapSource::InputGeoPoint => {
            v.push(("latitude", float_allow_null(d.latitude)));
            v.push(("longitude", float_allow_null(d.longitude)));
        }
    }
    if let Some(h) = d.access_hash {
        v.push(("access_hash", s(h.to_string())));
    }
    if let Some(r) = d.accuracy_radius {
        v.push(("accuracy_radius", n(r as i64)));
    }
    J::Obj(v)
}

fn block_kind_name(k: BlockKind) -> &'static str {
    use BlockKind as K;
    match k {
        K::Unsupported => "unsupported",
        K::Heading => "heading",
        K::Paragraph => "paragraph",
        K::Footer => "footer",
        K::Thinking => "thinking",
        K::AuthorDate => "author_date",
        K::Code => "code",
        K::Divider => "divider",
        K::Anchor => "anchor",
        K::List => "list",
        K::Quote => "quote",
        K::Photo => "photo",
        K::Video => "video",
        K::Cover => "cover",
        K::Embed => "embed",
        K::EmbedPost => "embed_post",
        K::Collage => "collage",
        K::Slideshow => "slideshow",
        K::Channel => "channel",
        K::Audio => "audio",
        K::File => "file",
        K::Math => "math",
        K::Table => "table",
        K::Details => "details",
        K::RelatedArticles => "related_articles",
        K::Map => "map",
        K::InputMap => "input_map",
        K::ButtonRow => "button_row",
        K::Unknown => "unsupported",
    }
}

fn alignment_name(a: ButtonAlignment) -> &'static str {
    match a {
        ButtonAlignment::Stretch => "stretch",
        ButtonAlignment::Left => "left",
        ButtonAlignment::Center => "center",
        ButtonAlignment::Right => "right",
    }
}

pub fn rich_blocks(m: &RichMessage, list: &[RichBlock]) -> J {
    arr(list.iter().map(|b| rich_block(m, b)).collect())
}

fn rich_block(m: &RichMessage, d: &RichBlock) -> J {
    use BlockKind as K;
    let mut v: Fields = vec![("type", s(block_kind_name(d.kind)))];
    match d.kind {
        K::Unsupported => {
            v.push(("source_type", s("page_block_unsupported")));
            v.push(("unsupported", J::Raw("true".into())));
        }
        K::Heading => {
            v.push(("level", n(d.heading_level as i64)));
            v.push(("text", rich_text(m, &d.text)));
        }
        K::Paragraph | K::Footer | K::Thinking => v.push(("text", rich_text(m, &d.text))),
        K::AuthorDate => {
            v.push(("author", rich_text(m, &d.text)));
            v.push(("date", s(iso_date(d.date))));
            v.push(("date_unixtime", s(d.date.to_string())));
        }
        K::Code => {
            v.push(("text", rich_text(m, &d.text)));
            v.push(("language", s(d.language.clone())));
        }
        K::Divider => {}
        K::Anchor => v.push(("name", s(d.name.clone()))),
        K::List => {
            v.push(("kind", s(list_kind_name(d.list_kind))));
            if d.list_kind == ListKind::Ordered {
                v.push(("reversed", b(d.ordered_list.reversed)));
                if let Some(start) = d.ordered_list.start {
                    v.push(("start", n(start as i64)));
                }
                if let Some(kind) = &d.ordered_list.kind {
                    v.push(("list_type", s(kind.clone())));
                }
            }
            v.push((
                "items",
                arr(d.list_items.iter().map(|i| list_item(m, i)).collect()),
            ));
        }
        K::Quote => {
            v.push(("content", s(quote_content_name(d.quote_content))));
            v.push(("pullquote", b(d.pullquote)));
            match d.quote_content {
                QuoteContent::Text => v.push(("text", rich_text(m, &d.text))),
                QuoteContent::Blocks => v.push(("blocks", rich_blocks(m, &d.blocks))),
            }
            v.push(("caption", rich_text(m, &d.quote_caption)));
        }
        K::Photo => {
            v.push(("photo_id", num_string(d.photo_id)));
            photo_metadata(
                &mut v,
                m.photos.get(&d.photo_id),
                (
                    "photo",
                    "photo_skip_reason",
                    "photo_file_size",
                    "width",
                    "height",
                ),
            );
            v.push(("spoiler", b(d.spoiler)));
            if let Some(url) = &d.optional_url {
                v.push(("url", s(url.clone())));
            }
            if let Some(id) = d.optional_webpage_id {
                v.push(("webpage_id", num_string(id)));
            }
            v.push(("caption", caption_object(m, &d.caption)));
        }
        K::Video => {
            v.push(("document_id", num_string(d.document_id)));
            document_metadata(&mut v, m.documents.get(&d.document_id), true);
            v.push(("autoplay", b(d.autoplay)));
            v.push(("loop", b(d.looped)));
            v.push(("spoiler", b(d.spoiler)));
            v.push(("caption", caption_object(m, &d.caption)));
        }
        K::Cover => {
            if let Some(first) = d.blocks.first() {
                v.push(("block", rich_block(m, first)));
            }
        }
        K::Embed => {
            if let Some(url) = &d.optional_url {
                v.push(("url", s(url.clone())));
            }
            if let Some(html) = &d.html {
                v.push(("html", s(html.clone())));
            }
            if let Some(id) = d.poster_photo_id {
                v.push(("poster_photo_id", num_string(id)));
                photo_metadata(
                    &mut v,
                    m.photos.get(&id),
                    (
                        "poster_photo",
                        "poster_photo_skip_reason",
                        "poster_photo_file_size",
                        "poster_photo_width",
                        "poster_photo_height",
                    ),
                );
            }
            if let Some(w) = d.width {
                v.push(("width", n(w as i64)));
            }
            if let Some(h) = d.height {
                v.push(("height", n(h as i64)));
            }
            v.push(("full_width", b(d.full_width)));
            v.push(("allow_scrolling", b(d.allow_scrolling)));
            v.push(("caption", caption_object(m, &d.caption)));
        }
        K::EmbedPost => {
            v.push(("url", s(d.url.clone())));
            v.push(("webpage_id", num_string(d.webpage_id)));
            v.push(("author_photo_id", num_string(d.author_photo_id)));
            photo_metadata(
                &mut v,
                m.photos.get(&d.author_photo_id),
                (
                    "author_photo",
                    "author_photo_skip_reason",
                    "author_photo_file_size",
                    "author_photo_width",
                    "author_photo_height",
                ),
            );
            v.push(("author", s(d.author.clone())));
            v.push(("date", s(iso_date(d.date))));
            v.push(("date_unixtime", s(d.date.to_string())));
            v.push(("blocks", rich_blocks(m, &d.blocks)));
            v.push(("caption", caption_object(m, &d.caption)));
        }
        K::Collage | K::Slideshow => {
            v.push(("items", rich_blocks(m, &d.blocks)));
            v.push(("caption", caption_object(m, &d.caption)));
        }
        K::Channel => v.push(("channel", channel_object(&d.channel))),
        K::Audio | K::File => {
            v.push(("document_id", num_string(d.document_id)));
            document_metadata(&mut v, m.documents.get(&d.document_id), true);
            v.push(("caption", caption_object(m, &d.caption)));
        }
        K::Math => v.push(("formula", s(d.formula.clone()))),
        K::Table => {
            v.push(("title", rich_text(m, &d.text)));
            v.push(("bordered", b(d.bordered)));
            v.push(("striped", b(d.striped)));
            v.push(("compact", b(d.compact)));
            v.push((
                "rows",
                arr(d.table_rows.iter().map(|r| table_row(m, r)).collect()),
            ));
        }
        K::Details => {
            v.push(("title", rich_text(m, &d.text)));
            v.push(("open", b(d.open)));
            v.push(("blocks", rich_blocks(m, &d.blocks)));
        }
        K::RelatedArticles => {
            v.push(("title", rich_text(m, &d.text)));
            v.push((
                "articles",
                arr(d
                    .related_articles
                    .iter()
                    .map(|a| related_article(m, a))
                    .collect()),
            ));
        }
        K::Map | K::InputMap => {
            v.push(("geo", map_point(&d.map_point)));
            v.push(("zoom", n(d.zoom as i64)));
            v.push(("width", n(d.map_width as i64)));
            v.push(("height", n(d.map_height as i64)));
            v.push(("caption", caption_object(m, &d.caption)));
        }
        K::ButtonRow => {
            v.push(("alignment", s(alignment_name(d.button_alignment))));
            v.push((
                "buttons",
                arr(d.buttons.iter().map(|bt| button_object(m, bt)).collect()),
            ));
        }
        K::Unknown => {
            v.push(("source_type", s("unknown")));
            v.push(("unsupported", J::Raw("true".into())));
        }
    }
    J::Obj(v)
}

/// The `rich_message` object of a message.
pub fn rich_message_json(m: &RichMessage) -> J {
    J::Obj(vec![
        ("rtl", b(m.rtl)),
        ("part", b(m.part)),
        ("blocks", rich_blocks(m, &m.blocks)),
    ])
}
