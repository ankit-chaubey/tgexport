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

//! Rich messages: the block based content some messages carry. The structures
//! and the parsing follow Telegram Desktop's export, including the order in
//! which the files of a rich message are visited.

use std::collections::BTreeMap;

use ferogram::tl;
use tl::enums as E;

use crate::model::*;
use crate::parse::{self, MediaCtx};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum RichTextKind {
    #[default]
    Empty,
    Plain,
    Concat,
    Bold,
    Italic,
    Underline,
    Strike,
    Fixed,
    Url,
    Email,
    Phone,
    Subscript,
    Superscript,
    Marked,
    Anchor,
    Math,
    CustomEmoji,
    Spoiler,
    Mention,
    Hashtag,
    BotCommand,
    Cashtag,
    AutoUrl,
    AutoEmail,
    AutoPhone,
    BankCard,
    TonAddress,
    MentionName,
    FormattedDate,
    InlineImage,
    Diff,
    Button,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InlinePeerType {
    SameBotPm,
    Pm,
    Chat,
    Megagroup,
    Broadcast,
    BotPm,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InlineActionKind {
    Url,
    Auth,
    WebView,
    Callback,
    CallbackWithPassword,
    Game,
    Buy,
    SwitchInline,
    SwitchInlineSame,
    UserProfile,
    CopyText,
    Disabled,
}

#[derive(Clone, Debug)]
pub struct InlineAction {
    pub kind: InlineActionKind,
    pub url: String,
    pub forward_text: Option<String>,
    pub callback_data: Vec<u8>,
    pub query: String,
    pub peer_types: Option<Vec<InlinePeerType>>,
    pub copy_text: String,
    pub user_id: u64,
    pub button_id: i32,
    pub requires_password: bool,
    pub same_peer: bool,
}

impl InlineAction {
    fn new(kind: InlineActionKind) -> Self {
        InlineAction {
            kind,
            url: String::new(),
            forward_text: None,
            callback_data: Vec::new(),
            query: String::new(),
            peer_types: None,
            copy_text: String::new(),
            user_id: 0,
            button_id: 0,
            requires_password: false,
            same_peer: false,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ButtonStyle {
    Default,
    Primary,
    Success,
    Danger,
    Link,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ButtonAlignment {
    Stretch,
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug)]
pub struct ButtonPayload {
    pub action: InlineAction,
    pub style: Option<ButtonStyle>,
}

#[derive(Clone, Debug, Default)]
pub struct RichText {
    pub kind: RichTextKind,
    pub text: String,
    pub data: String,
    /// For custom emoji: the document id, later the path of the saved file.
    pub custom_emoji_data: String,
    pub children: Vec<RichText>,
    pub old_children: Vec<RichText>,
    pub button: Option<Box<ButtonPayload>>,
    pub id: u64,
    pub date: i32,
    pub width: i32,
    pub height: i32,
    pub relative: bool,
    pub short_time: bool,
    pub long_time: bool,
    pub short_date: bool,
    pub long_date: bool,
    pub day_of_week: bool,
    pub unsupported: bool,
}

#[derive(Clone, Debug, Default)]
pub struct RichCaption {
    pub text: RichText,
    pub credit: RichText,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TaskState {
    None,
    Unchecked,
    Checked,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ListKind {
    Bullet,
    Ordered,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ItemContent {
    Text,
    Blocks,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum QuoteContent {
    Text,
    Blocks,
}

#[derive(Clone, Debug, Default)]
pub struct OrderedList {
    pub kind: Option<String>,
    pub start: Option<i32>,
    pub reversed: bool,
}

#[derive(Clone, Debug)]
pub struct RichListItem {
    pub text: Option<RichText>,
    pub blocks: Vec<RichBlock>,
    pub num: Option<String>,
    pub kind: Option<String>,
    pub value: Option<i32>,
    pub task_state: TaskState,
    pub content: ItemContent,
}

impl RichListItem {
    fn new(content: ItemContent) -> Self {
        RichListItem {
            text: None,
            blocks: Vec::new(),
            num: None,
            kind: None,
            value: None,
            task_state: TaskState::None,
            content,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TableAlign {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TableValign {
    Top,
    Middle,
    Bottom,
}

#[derive(Clone, Debug)]
pub struct TableCell {
    pub text: Option<RichText>,
    pub colspan: Option<i32>,
    pub rowspan: Option<i32>,
    pub align: TableAlign,
    pub valign: TableValign,
    pub header: bool,
}

#[derive(Clone, Debug)]
pub struct TableRow {
    pub cells: Vec<TableCell>,
}

#[derive(Clone, Debug)]
pub struct RelatedArticle {
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    pub photo_id: Option<u64>,
    pub published_date: Option<i32>,
    pub webpage_id: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChannelSource {
    ChatEmpty,
    Chat,
    ChatForbidden,
    Channel,
    ChannelForbidden,
    Community,
    CommunityForbidden,
}

#[derive(Clone, Debug)]
pub struct RichChannel {
    pub title: Option<String>,
    pub username: Option<String>,
    pub access_hash: Option<i64>,
    pub id: u64,
    pub source: ChannelSource,
    pub broadcast: bool,
    pub megagroup: bool,
    pub monoforum: bool,
}

impl Default for RichChannel {
    fn default() -> Self {
        RichChannel {
            title: None,
            username: None,
            access_hash: None,
            id: 0,
            source: ChannelSource::ChatEmpty,
            broadcast: false,
            megagroup: false,
            monoforum: false,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MapSource {
    GeoPointEmpty,
    GeoPoint,
    InputGeoPointEmpty,
    InputGeoPoint,
}

#[derive(Clone, Debug)]
pub struct MapPoint {
    pub access_hash: Option<i64>,
    pub accuracy_radius: Option<i32>,
    pub latitude: f64,
    pub longitude: f64,
    pub source: MapSource,
}

impl Default for MapPoint {
    fn default() -> Self {
        MapPoint {
            access_hash: None,
            accuracy_radius: None,
            latitude: 0.0,
            longitude: 0.0,
            source: MapSource::GeoPointEmpty,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BlockKind {
    Unsupported,
    Heading,
    Paragraph,
    Footer,
    Thinking,
    AuthorDate,
    Code,
    Divider,
    Anchor,
    List,
    Quote,
    Photo,
    Video,
    Cover,
    Embed,
    EmbedPost,
    Collage,
    Slideshow,
    Channel,
    Audio,
    File,
    Math,
    Table,
    Details,
    RelatedArticles,
    Map,
    InputMap,
    ButtonRow,
    Unknown,
}

#[derive(Clone, Debug)]
pub struct RichBlock {
    pub kind: BlockKind,
    pub text: RichText,
    pub quote_caption: RichText,
    pub caption: RichCaption,
    pub ordered_list: OrderedList,
    pub channel: RichChannel,
    pub map_point: MapPoint,
    pub language: String,
    pub formula: String,
    pub name: String,
    pub url: String,
    pub author: String,
    pub optional_url: Option<String>,
    pub html: Option<String>,
    pub blocks: Vec<RichBlock>,
    pub list_items: Vec<RichListItem>,
    pub table_rows: Vec<TableRow>,
    pub related_articles: Vec<RelatedArticle>,
    pub buttons: Vec<RichText>,
    pub optional_webpage_id: Option<u64>,
    pub poster_photo_id: Option<u64>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub photo_id: u64,
    pub document_id: u64,
    pub webpage_id: u64,
    pub author_photo_id: u64,
    pub date: i32,
    pub heading_level: i32,
    pub zoom: i32,
    pub map_width: i32,
    pub map_height: i32,
    pub list_kind: ListKind,
    pub quote_content: QuoteContent,
    pub button_alignment: ButtonAlignment,
    pub unsupported: bool,
    pub full_width: bool,
    pub allow_scrolling: bool,
    pub autoplay: bool,
    pub looped: bool,
    pub spoiler: bool,
    pub open: bool,
    pub bordered: bool,
    pub striped: bool,
    pub compact: bool,
    pub pullquote: bool,
}

impl RichBlock {
    fn new(kind: BlockKind) -> Self {
        RichBlock {
            kind,
            text: RichText::default(),
            quote_caption: RichText::default(),
            caption: RichCaption::default(),
            ordered_list: OrderedList::default(),
            channel: RichChannel::default(),
            map_point: MapPoint::default(),
            language: String::new(),
            formula: String::new(),
            name: String::new(),
            url: String::new(),
            author: String::new(),
            optional_url: None,
            html: None,
            blocks: Vec::new(),
            list_items: Vec::new(),
            table_rows: Vec::new(),
            related_articles: Vec::new(),
            buttons: Vec::new(),
            optional_webpage_id: None,
            poster_photo_id: None,
            width: None,
            height: None,
            photo_id: 0,
            document_id: 0,
            webpage_id: 0,
            author_photo_id: 0,
            date: 0,
            heading_level: 0,
            zoom: 0,
            map_width: 0,
            map_height: 0,
            list_kind: ListKind::Bullet,
            quote_content: QuoteContent::Text,
            button_alignment: ButtonAlignment::Stretch,
            unsupported: false,
            full_width: false,
            allow_scrolling: false,
            autoplay: false,
            looped: false,
            spoiler: false,
            open: false,
            bordered: false,
            striped: false,
            compact: false,
            pullquote: false,
        }
    }

    fn with_text(kind: BlockKind, text: &E::RichText) -> Self {
        let mut b = RichBlock::new(kind);
        b.text = parse_text(text);
        b
    }

    fn heading(level: i32, text: &E::RichText) -> Self {
        let mut b = RichBlock::with_text(BlockKind::Heading, text);
        b.heading_level = level;
        b
    }
}

#[derive(Clone, Debug)]
pub struct RichPhoto {
    pub file: FileRef,
    pub width: i32,
    pub height: i32,
}

#[derive(Clone, Debug)]
pub struct RichDocument {
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

#[derive(Clone, Debug, Default)]
pub struct RichMessage {
    pub blocks: Vec<RichBlock>,
    pub photos: BTreeMap<u64, RichPhoto>,
    pub documents: BTreeMap<u64, RichDocument>,
    pub rtl: bool,
    pub part: bool,
}

// Text.

fn wrapper(kind: RichTextKind, child: &E::RichText) -> RichText {
    RichText {
        kind,
        children: vec![parse_text(child)],
        ..Default::default()
    }
}

fn parse_peer_type(t: &E::InlineQueryPeerType) -> InlinePeerType {
    match t {
        E::InlineQueryPeerType::SameBotPm => InlinePeerType::SameBotPm,
        E::InlineQueryPeerType::Pm => InlinePeerType::Pm,
        E::InlineQueryPeerType::Chat => InlinePeerType::Chat,
        E::InlineQueryPeerType::Megagroup => InlinePeerType::Megagroup,
        E::InlineQueryPeerType::Broadcast => InlinePeerType::Broadcast,
        E::InlineQueryPeerType::BotPm => InlinePeerType::BotPm,
    }
}

pub fn parse_inline_action(t: &E::InlineButtonType) -> Option<InlineAction> {
    use InlineActionKind as K;
    Some(match t {
        E::InlineButtonType::Url(d) => {
            let mut a = InlineAction::new(K::Url);
            a.url = d.url.clone();
            a
        }
        E::InlineButtonType::UrlAuth(d) => {
            let mut a = InlineAction::new(K::Auth);
            a.url = d.url.clone();
            a.button_id = d.button_id;
            a.forward_text = d.fwd_text.clone();
            a
        }
        E::InlineButtonType::InputInlineButtonTypeUrlAuth(_) => return None,
        E::InlineButtonType::WebView(d) => {
            let mut a = InlineAction::new(K::WebView);
            a.url = d.url.clone();
            a
        }
        E::InlineButtonType::Callback(d) => {
            let mut a = InlineAction::new(if d.requires_password {
                K::CallbackWithPassword
            } else {
                K::Callback
            });
            a.requires_password = d.requires_password;
            a.callback_data = d.data.clone();
            a
        }
        E::InlineButtonType::Game => InlineAction::new(K::Game),
        E::InlineButtonType::Buy => InlineAction::new(K::Buy),
        E::InlineButtonType::SwitchInline(d) => {
            let mut a = InlineAction::new(if d.same_peer {
                K::SwitchInlineSame
            } else {
                K::SwitchInline
            });
            a.same_peer = d.same_peer;
            a.query = d.query.clone();
            a.peer_types = d
                .peer_types
                .as_ref()
                .map(|list| list.iter().map(parse_peer_type).collect());
            a
        }
        E::InlineButtonType::UserProfile(d) => {
            let mut a = InlineAction::new(K::UserProfile);
            a.user_id = d.user_id as u64;
            a
        }
        E::InlineButtonType::InputInlineButtonTypeUserProfile(_) => return None,
        E::InlineButtonType::Copy(d) => {
            let mut a = InlineAction::new(K::CopyText);
            a.copy_text = d.copy_text.clone();
            a
        }
        E::InlineButtonType::Disabled => InlineAction::new(K::Disabled),
    })
}

fn parse_button_style(
    style: &Option<E::RichButtonStyle>,
    action: InlineActionKind,
    inline_button: bool,
) -> Option<ButtonStyle> {
    let E::RichButtonStyle::RichButtonStyle(d) = style.as_ref()?;
    let link_allowed = matches!(
        action,
        InlineActionKind::Callback
            | InlineActionKind::CallbackWithPassword
            | InlineActionKind::Disabled
    );
    if inline_button && d.link && link_allowed {
        return Some(ButtonStyle::Link);
    }
    Some(if d.bg_danger {
        ButtonStyle::Danger
    } else if d.bg_primary {
        ButtonStyle::Primary
    } else if d.bg_success {
        ButtonStyle::Success
    } else {
        ButtonStyle::Default
    })
}

fn parse_button(
    text: &E::RichText,
    kind: &E::InlineButtonType,
    style: &Option<E::RichButtonStyle>,
    inline_button: bool,
) -> Option<RichText> {
    let action = parse_inline_action(kind)?;
    let style = parse_button_style(style, action.kind, inline_button);
    Some(RichText {
        kind: RichTextKind::Button,
        children: vec![parse_text(text)],
        button: Some(Box::new(ButtonPayload { action, style })),
        ..Default::default()
    })
}

pub fn parse_text(text: &E::RichText) -> RichText {
    use RichTextKind as K;
    match text {
        E::RichText::TextEmpty => RichText::default(),
        E::RichText::TextPlain(d) => RichText {
            kind: K::Plain,
            text: d.text.clone(),
            ..Default::default()
        },
        E::RichText::TextBold(d) => wrapper(K::Bold, &d.text),
        E::RichText::TextItalic(d) => wrapper(K::Italic, &d.text),
        E::RichText::TextUnderline(d) => wrapper(K::Underline, &d.text),
        E::RichText::TextStrike(d) => wrapper(K::Strike, &d.text),
        E::RichText::TextFixed(d) => wrapper(K::Fixed, &d.text),
        E::RichText::TextUrl(d) => {
            let mut r = wrapper(K::Url, &d.text);
            r.data = d.url.clone();
            r.id = d.webpage_id as u64;
            r
        }
        E::RichText::TextEmail(d) => {
            let mut r = wrapper(K::Email, &d.text);
            r.data = d.email.clone();
            r
        }
        E::RichText::TextConcat(d) => RichText {
            kind: K::Concat,
            children: d.texts.iter().map(parse_text).collect(),
            ..Default::default()
        },
        E::RichText::TextSubscript(d) => wrapper(K::Subscript, &d.text),
        E::RichText::TextSuperscript(d) => wrapper(K::Superscript, &d.text),
        E::RichText::TextMarked(d) => wrapper(K::Marked, &d.text),
        E::RichText::TextPhone(d) => {
            let mut r = wrapper(K::Phone, &d.text);
            r.data = d.phone.clone();
            r
        }
        E::RichText::TextImage(d) => RichText {
            kind: K::InlineImage,
            id: d.document_id as u64,
            width: d.w,
            height: d.h,
            ..Default::default()
        },
        E::RichText::TextAnchor(d) => {
            let mut r = wrapper(K::Anchor, &d.text);
            r.data = d.name.clone();
            r
        }
        E::RichText::TextMath(d) => RichText {
            kind: K::Math,
            data: d.source.clone(),
            ..Default::default()
        },
        E::RichText::TextCustomEmoji(d) => {
            let id = d.document_id as u64;
            RichText {
                kind: K::CustomEmoji,
                text: d.alt.clone(),
                id,
                custom_emoji_data: id.to_string(),
                ..Default::default()
            }
        }
        E::RichText::TextSpoiler(d) => wrapper(K::Spoiler, &d.text),
        E::RichText::TextMention(d) => wrapper(K::Mention, &d.text),
        E::RichText::TextHashtag(d) => wrapper(K::Hashtag, &d.text),
        E::RichText::TextBotCommand(d) => wrapper(K::BotCommand, &d.text),
        E::RichText::TextCashtag(d) => wrapper(K::Cashtag, &d.text),
        E::RichText::TextAutoUrl(d) => wrapper(K::AutoUrl, &d.text),
        E::RichText::TextAutoEmail(d) => wrapper(K::AutoEmail, &d.text),
        E::RichText::TextAutoPhone(d) => wrapper(K::AutoPhone, &d.text),
        E::RichText::TextBankCard(d) => wrapper(K::BankCard, &d.text),
        E::RichText::TextTonAddress(d) => wrapper(K::TonAddress, &d.text),
        E::RichText::TextMentionName(d) => {
            let mut r = wrapper(K::MentionName, &d.text);
            r.id = d.user_id as u64;
            r
        }
        E::RichText::TextDate(d) => {
            let mut r = wrapper(K::FormattedDate, &d.text);
            r.date = d.date;
            r.relative = d.relative;
            r.short_time = d.short_time;
            r.long_time = d.long_time;
            r.short_date = d.short_date;
            r.long_date = d.long_date;
            r.day_of_week = d.day_of_week;
            r
        }
        E::RichText::TextDiff(d) => RichText {
            kind: K::Diff,
            unsupported: true,
            children: vec![parse_text(&d.text)],
            old_children: vec![parse_text(&d.old_text)],
            ..Default::default()
        },
        E::RichText::TextButton(d) => {
            parse_button(&d.text, &d.r#type, &d.style, true).unwrap_or_else(|| parse_text(&d.text))
        }
    }
}

// Blocks.

fn parse_caption(c: &E::PageCaption) -> RichCaption {
    let E::PageCaption::PageCaption(d) = c;
    RichCaption {
        text: parse_text(&d.text),
        credit: parse_text(&d.credit),
    }
}

fn task_state(checkbox: bool, checked: bool) -> TaskState {
    if !checkbox {
        TaskState::None
    } else if checked {
        TaskState::Checked
    } else {
        TaskState::Unchecked
    }
}

fn parse_list_item(item: &E::PageListItem) -> RichListItem {
    match item {
        E::PageListItem::Text(d) => {
            let mut r = RichListItem::new(ItemContent::Text);
            r.task_state = task_state(d.checkbox, d.checked);
            r.text = Some(parse_text(&d.text));
            r
        }
        E::PageListItem::Blocks(d) => {
            let mut r = RichListItem::new(ItemContent::Blocks);
            r.task_state = task_state(d.checkbox, d.checked);
            r.blocks = parse_blocks(&d.blocks);
            r
        }
    }
}

fn parse_ordered_item(item: &E::PageListOrderedItem) -> RichListItem {
    match item {
        E::PageListOrderedItem::Text(d) => {
            let mut r = RichListItem::new(ItemContent::Text);
            r.task_state = task_state(d.checkbox, d.checked);
            r.num = d.num.clone();
            r.value = d.value;
            r.kind = d.r#type.clone();
            r.text = Some(parse_text(&d.text));
            r
        }
        E::PageListOrderedItem::Blocks(d) => {
            let mut r = RichListItem::new(ItemContent::Blocks);
            r.task_state = task_state(d.checkbox, d.checked);
            r.num = d.num.clone();
            r.value = d.value;
            r.kind = d.r#type.clone();
            r.blocks = parse_blocks(&d.blocks);
            r
        }
    }
}

fn parse_table_cell(cell: &E::PageTableCell) -> TableCell {
    let E::PageTableCell::PageTableCell(d) = cell;
    TableCell {
        text: d.text.as_ref().map(parse_text),
        colspan: d.colspan,
        rowspan: d.rowspan,
        align: if d.align_right {
            TableAlign::Right
        } else if d.align_center {
            TableAlign::Center
        } else {
            TableAlign::Left
        },
        valign: if d.valign_bottom {
            TableValign::Bottom
        } else if d.valign_middle {
            TableValign::Middle
        } else {
            TableValign::Top
        },
        header: d.header,
    }
}

fn parse_table_row(row: &E::PageTableRow) -> TableRow {
    let E::PageTableRow::PageTableRow(d) = row;
    TableRow {
        cells: d.cells.iter().map(parse_table_cell).collect(),
    }
}

fn parse_related(article: &E::PageRelatedArticle) -> RelatedArticle {
    let E::PageRelatedArticle::PageRelatedArticle(d) = article;
    RelatedArticle {
        url: d.url.clone(),
        title: d.title.clone(),
        description: d.description.clone(),
        author: d.author.clone(),
        photo_id: d.photo_id.map(|x| x as u64),
        published_date: d.published_date,
        webpage_id: d.webpage_id as u64,
    }
}

fn parse_channel(chat: &E::Chat) -> RichChannel {
    let mut r = RichChannel::default();
    match chat {
        E::Chat::Empty(d) => {
            r.source = ChannelSource::ChatEmpty;
            r.id = d.id as u64;
        }
        E::Chat::Chat(d) => {
            r.source = ChannelSource::Chat;
            r.id = d.id as u64;
            r.title = Some(d.title.clone());
        }
        E::Chat::Forbidden(d) => {
            r.source = ChannelSource::ChatForbidden;
            r.id = d.id as u64;
            r.title = Some(d.title.clone());
        }
        E::Chat::Channel(d) => {
            r.source = ChannelSource::Channel;
            r.id = d.id as u64;
            r.title = Some(d.title.clone());
            r.broadcast = d.broadcast;
            r.megagroup = d.megagroup;
            r.monoforum = d.monoforum;
            r.access_hash = d.access_hash;
            r.username = d.username.clone();
        }
        E::Chat::ChannelForbidden(d) => {
            r.source = ChannelSource::ChannelForbidden;
            r.id = d.id as u64;
            r.title = Some(d.title.clone());
            r.access_hash = Some(d.access_hash);
            r.broadcast = d.broadcast;
            r.megagroup = d.megagroup;
            r.monoforum = d.monoforum;
        }
        E::Chat::Community(d) => {
            r.source = ChannelSource::Community;
            r.id = d.id as u64;
            r.title = Some(d.title.clone());
            r.access_hash = d.access_hash;
        }
        E::Chat::CommunityForbidden(d) => {
            r.source = ChannelSource::CommunityForbidden;
            r.id = d.id as u64;
            r.title = Some(d.title.clone());
            r.access_hash = d.access_hash;
        }
    }
    r
}

fn parse_geo(point: &E::GeoPoint) -> MapPoint {
    match point {
        E::GeoPoint::Empty => MapPoint::default(),
        E::GeoPoint::GeoPoint(d) => MapPoint {
            source: MapSource::GeoPoint,
            latitude: d.lat,
            longitude: d.long,
            access_hash: Some(d.access_hash),
            accuracy_radius: d.accuracy_radius,
        },
    }
}

fn parse_input_geo(point: &E::InputGeoPoint) -> MapPoint {
    match point {
        E::InputGeoPoint::Empty => MapPoint {
            source: MapSource::InputGeoPointEmpty,
            ..Default::default()
        },
        E::InputGeoPoint::InputGeoPoint(d) => MapPoint {
            source: MapSource::InputGeoPoint,
            latitude: d.lat,
            longitude: d.long,
            access_hash: None,
            accuracy_radius: d.accuracy_radius,
        },
    }
}

pub fn parse_blocks(blocks: &[E::PageBlock]) -> Vec<RichBlock> {
    blocks.iter().map(parse_block).collect()
}

pub fn parse_block(block: &E::PageBlock) -> RichBlock {
    use BlockKind as K;
    match block {
        E::PageBlock::Unsupported => {
            let mut b = RichBlock::new(K::Unsupported);
            b.unsupported = true;
            b
        }
        E::PageBlock::Title(d) => RichBlock::heading(1, &d.text),
        E::PageBlock::Subtitle(d) => RichBlock::heading(2, &d.text),
        E::PageBlock::AuthorDate(d) => {
            let mut b = RichBlock::with_text(K::AuthorDate, &d.author);
            b.date = d.published_date;
            b
        }
        E::PageBlock::Header(d) => RichBlock::heading(3, &d.text),
        E::PageBlock::Subheader(d) => RichBlock::heading(4, &d.text),
        E::PageBlock::Paragraph(d) => RichBlock::with_text(K::Paragraph, &d.text),
        E::PageBlock::Preformatted(d) => {
            let mut b = RichBlock::with_text(K::Code, &d.text);
            b.language = d.language.clone();
            b
        }
        E::PageBlock::Footer(d) => RichBlock::with_text(K::Footer, &d.text),
        E::PageBlock::Divider => RichBlock::new(K::Divider),
        E::PageBlock::Anchor(d) => {
            let mut b = RichBlock::new(K::Anchor);
            b.name = d.name.clone();
            b
        }
        E::PageBlock::List(d) => {
            let mut b = RichBlock::new(K::List);
            b.list_kind = ListKind::Bullet;
            b.list_items = d.items.iter().map(parse_list_item).collect();
            b
        }
        E::PageBlock::Blockquote(d) => {
            let mut b = RichBlock::with_text(K::Quote, &d.text);
            b.quote_content = QuoteContent::Text;
            b.quote_caption = parse_text(&d.caption);
            b
        }
        E::PageBlock::Pullquote(d) => {
            let mut b = RichBlock::with_text(K::Quote, &d.text);
            b.quote_content = QuoteContent::Text;
            b.quote_caption = parse_text(&d.caption);
            b.pullquote = true;
            b
        }
        E::PageBlock::Photo(d) => {
            let mut b = RichBlock::new(K::Photo);
            b.photo_id = d.photo_id as u64;
            b.caption = parse_caption(&d.caption);
            b.spoiler = d.spoiler;
            b.optional_url = d.url.clone();
            b.optional_webpage_id = d.webpage_id.map(|x| x as u64);
            b
        }
        E::PageBlock::Video(d) => {
            let mut b = RichBlock::new(K::Video);
            b.document_id = d.video_id as u64;
            b.caption = parse_caption(&d.caption);
            b.autoplay = d.autoplay;
            b.looped = d.r#loop;
            b.spoiler = d.spoiler;
            b
        }
        E::PageBlock::Cover(d) => {
            let mut b = RichBlock::new(K::Cover);
            b.blocks = vec![parse_block(&d.cover)];
            b
        }
        E::PageBlock::Embed(d) => {
            let mut b = RichBlock::new(K::Embed);
            b.full_width = d.full_width;
            b.allow_scrolling = d.allow_scrolling;
            b.caption = parse_caption(&d.caption);
            b.optional_url = d.url.clone();
            b.html = d.html.clone();
            b.poster_photo_id = d.poster_photo_id.map(|x| x as u64);
            b.width = d.w;
            b.height = d.h;
            b
        }
        E::PageBlock::EmbedPost(d) => {
            let mut b = RichBlock::new(K::EmbedPost);
            b.url = d.url.clone();
            b.webpage_id = d.webpage_id as u64;
            b.author_photo_id = d.author_photo_id as u64;
            b.author = d.author.clone();
            b.date = d.date;
            b.blocks = parse_blocks(&d.blocks);
            b.caption = parse_caption(&d.caption);
            b
        }
        E::PageBlock::Collage(d) => {
            let mut b = RichBlock::new(K::Collage);
            b.blocks = parse_blocks(&d.items);
            b.caption = parse_caption(&d.caption);
            b
        }
        E::PageBlock::Slideshow(d) => {
            let mut b = RichBlock::new(K::Slideshow);
            b.blocks = parse_blocks(&d.items);
            b.caption = parse_caption(&d.caption);
            b
        }
        E::PageBlock::Channel(d) => {
            let mut b = RichBlock::new(K::Channel);
            b.channel = parse_channel(&d.channel);
            b
        }
        E::PageBlock::Audio(d) => {
            let mut b = RichBlock::new(K::Audio);
            b.document_id = d.audio_id as u64;
            b.caption = parse_caption(&d.caption);
            b
        }
        E::PageBlock::Kicker(d) => RichBlock::heading(5, &d.text),
        E::PageBlock::Table(d) => {
            let mut b = RichBlock::with_text(K::Table, &d.title);
            b.bordered = d.bordered;
            b.striped = d.striped;
            b.compact = d.compact;
            b.table_rows = d.rows.iter().map(parse_table_row).collect();
            b
        }
        E::PageBlock::OrderedList(d) => {
            let mut b = RichBlock::new(K::List);
            b.list_kind = ListKind::Ordered;
            b.ordered_list.reversed = d.reversed;
            b.ordered_list.start = d.start;
            b.ordered_list.kind = d.r#type.clone();
            b.list_items = d.items.iter().map(parse_ordered_item).collect();
            b
        }
        E::PageBlock::Details(d) => {
            let mut b = RichBlock::with_text(K::Details, &d.title);
            b.open = d.open;
            b.blocks = parse_blocks(&d.blocks);
            b
        }
        E::PageBlock::RelatedArticles(d) => {
            let mut b = RichBlock::new(K::RelatedArticles);
            b.text = parse_text(&d.title);
            b.related_articles = d.articles.iter().map(parse_related).collect();
            b
        }
        E::PageBlock::Map(d) => {
            let mut b = RichBlock::new(K::Map);
            b.map_point = parse_geo(&d.geo);
            b.zoom = d.zoom;
            b.map_width = d.w;
            b.map_height = d.h;
            b.caption = parse_caption(&d.caption);
            b
        }
        E::PageBlock::Heading1(d) => RichBlock::heading(1, &d.text),
        E::PageBlock::Heading2(d) => RichBlock::heading(2, &d.text),
        E::PageBlock::Heading3(d) => RichBlock::heading(3, &d.text),
        E::PageBlock::Heading4(d) => RichBlock::heading(4, &d.text),
        E::PageBlock::Heading5(d) => RichBlock::heading(5, &d.text),
        E::PageBlock::Heading6(d) => RichBlock::heading(6, &d.text),
        E::PageBlock::Math(d) => {
            let mut b = RichBlock::new(K::Math);
            b.formula = d.source.clone();
            b
        }
        E::PageBlock::Thinking(d) => RichBlock::with_text(K::Thinking, &d.text),
        E::PageBlock::InputPageBlockMap(d) => {
            let mut b = RichBlock::new(K::InputMap);
            b.map_point = parse_input_geo(&d.geo);
            b.zoom = d.zoom;
            b.map_width = d.w;
            b.map_height = d.h;
            b.caption = parse_caption(&d.caption);
            b
        }
        E::PageBlock::BlockquoteBlocks(d) => {
            let mut b = RichBlock::new(K::Quote);
            b.quote_content = QuoteContent::Blocks;
            b.blocks = parse_blocks(&d.blocks);
            b.quote_caption = parse_text(&d.caption);
            b
        }
        E::PageBlock::ButtonRow(d) => {
            let mut b = RichBlock::new(K::ButtonRow);
            b.button_alignment = if d.align_left {
                ButtonAlignment::Left
            } else if d.align_center {
                ButtonAlignment::Center
            } else if d.align_right {
                ButtonAlignment::Right
            } else {
                ButtonAlignment::Stretch
            };
            for button in &d.buttons {
                let E::PageButton::PageButton(f) = button;
                if let Some(parsed) = parse_button(&f.text, &f.r#type, &f.style, false) {
                    b.buttons.push(parsed);
                }
            }
            b
        }
        E::PageBlock::Document(d) => {
            let mut b = RichBlock::new(K::File);
            b.document_id = d.document_id as u64;
            b.caption = parse_caption(&d.caption);
            b
        }
    }
}

// The message itself.

/// Parses a rich message, numbering and naming its files within the chat.
pub fn parse_rich_message(ctx: &mut MediaCtx, m: &E::RichMessage, date: i32) -> RichMessage {
    let E::RichMessage::RichMessage(d) = m;
    let mut out = RichMessage {
        rtl: d.rtl,
        part: d.part,
        ..Default::default()
    };
    for photo in &d.photos {
        ctx.photos += 1;
        let name = format!("photo_{}{}.jpg", ctx.photos, parse::file_date_part(date));
        let path = format!("{}photos/{}", ctx.folder, name);
        match photo {
            E::Photo::Photo(p) => {
                let (file, width, height) =
                    parse::photo_file(photo, path).unwrap_or((FileRef::none(), 0, 0));
                out.photos.insert(
                    p.id as u64,
                    RichPhoto {
                        file,
                        width,
                        height,
                    },
                );
            }
            E::Photo::Empty(p) => {
                out.photos.insert(
                    p.id as u64,
                    RichPhoto {
                        file: FileRef::none(),
                        width: 0,
                        height: 0,
                    },
                );
            }
        }
    }
    for doc in &d.documents {
        match doc {
            E::Document::Document(p) => {
                if let Some(info) = parse::parse_document(ctx, doc, date) {
                    out.documents.insert(
                        p.id as u64,
                        RichDocument {
                            file: info.file,
                            name: info.name,
                            mime: info.mime,
                            duration: info.duration,
                            width: info.width,
                            height: info.height,
                            flags: info.flags,
                            sticker_emoji: info.sticker_emoji,
                            performer: info.performer,
                            title: info.title,
                            thumb: info.thumb,
                        },
                    );
                }
            }
            E::Document::Empty(p) => {
                out.documents.insert(
                    p.id as u64,
                    RichDocument {
                        file: FileRef::none(),
                        name: String::new(),
                        mime: String::new(),
                        duration: 0,
                        width: 0,
                        height: 0,
                        flags: DocFlags::default(),
                        sticker_emoji: String::new(),
                        performer: String::new(),
                        title: String::new(),
                        thumb: None,
                    },
                );
            }
        }
    }
    out.blocks = parse_blocks(&d.blocks);
    out
}

// Traversal.

/// What a visit of a rich message reports, in tdesktop's order.
pub enum Visit<'a> {
    Text(&'a RichText),
    Photo(u64),
    Document(u64),
}

fn visit_text(t: &RichText, f: &mut impl FnMut(Visit)) {
    f(Visit::Text(t));
    if t.kind == RichTextKind::InlineImage {
        f(Visit::Document(t.id));
    }
    for c in &t.children {
        visit_text(c, f);
    }
    for c in &t.old_children {
        visit_text(c, f);
    }
}

fn visit_caption(c: &RichCaption, f: &mut impl FnMut(Visit)) {
    visit_text(&c.text, f);
    visit_text(&c.credit, f);
}

fn visit_blocks(blocks: &[RichBlock], f: &mut impl FnMut(Visit)) {
    for b in blocks {
        visit_block(b, f);
    }
}

fn visit_block(b: &RichBlock, f: &mut impl FnMut(Visit)) {
    use BlockKind as K;
    match b.kind {
        K::Heading | K::Paragraph | K::Footer | K::Thinking | K::AuthorDate | K::Code => {
            visit_text(&b.text, f)
        }
        K::List => {
            for item in &b.list_items {
                match item.content {
                    ItemContent::Text => {
                        if let Some(t) = &item.text {
                            visit_text(t, f);
                        }
                    }
                    ItemContent::Blocks => visit_blocks(&item.blocks, f),
                }
            }
        }
        K::Quote => {
            match b.quote_content {
                QuoteContent::Text => visit_text(&b.text, f),
                QuoteContent::Blocks => visit_blocks(&b.blocks, f),
            }
            visit_text(&b.quote_caption, f);
        }
        K::Photo => {
            f(Visit::Photo(b.photo_id));
            visit_caption(&b.caption, f);
        }
        K::Video | K::Audio | K::File => {
            f(Visit::Document(b.document_id));
            visit_caption(&b.caption, f);
        }
        K::Cover => {
            if let Some(first) = b.blocks.first() {
                visit_block(first, f);
            }
        }
        K::Embed => {
            if let Some(id) = b.poster_photo_id {
                f(Visit::Photo(id));
            }
            visit_caption(&b.caption, f);
        }
        K::EmbedPost => {
            f(Visit::Photo(b.author_photo_id));
            visit_blocks(&b.blocks, f);
            visit_caption(&b.caption, f);
        }
        K::Collage | K::Slideshow => {
            visit_blocks(&b.blocks, f);
            visit_caption(&b.caption, f);
        }
        K::Table => {
            visit_text(&b.text, f);
            for row in &b.table_rows {
                for cell in &row.cells {
                    if let Some(t) = &cell.text {
                        visit_text(t, f);
                    }
                }
            }
        }
        K::Details => {
            visit_text(&b.text, f);
            visit_blocks(&b.blocks, f);
        }
        K::RelatedArticles => {
            visit_text(&b.text, f);
            for a in &b.related_articles {
                if let Some(id) = a.photo_id {
                    f(Visit::Photo(id));
                }
            }
        }
        K::Map | K::InputMap => visit_caption(&b.caption, f),
        K::ButtonRow => {
            for button in &b.buttons {
                visit_text(button, f);
            }
        }
        K::Unsupported | K::Divider | K::Anchor | K::Channel | K::Math | K::Unknown => {}
    }
}

/// Walks the message and reports texts, photos and documents that are
/// actually used by its blocks.
pub fn visit(message: &RichMessage, f: &mut impl FnMut(Visit)) {
    visit_blocks(&message.blocks, f);
}

// Mutable walk over the texts only, used to fill in custom emoji paths.

fn texts_mut_text(t: &mut RichText, f: &mut impl FnMut(&mut RichText)) {
    f(t);
    for c in &mut t.children {
        texts_mut_text(c, f);
    }
    for c in &mut t.old_children {
        texts_mut_text(c, f);
    }
}

fn texts_mut_caption(c: &mut RichCaption, f: &mut impl FnMut(&mut RichText)) {
    texts_mut_text(&mut c.text, f);
    texts_mut_text(&mut c.credit, f);
}

fn texts_mut_blocks(blocks: &mut [RichBlock], f: &mut impl FnMut(&mut RichText)) {
    for b in blocks {
        texts_mut_block(b, f);
    }
}

fn texts_mut_block(b: &mut RichBlock, f: &mut impl FnMut(&mut RichText)) {
    use BlockKind as K;
    match b.kind {
        K::Heading | K::Paragraph | K::Footer | K::Thinking | K::AuthorDate | K::Code => {
            texts_mut_text(&mut b.text, f)
        }
        K::List => {
            for item in &mut b.list_items {
                match item.content {
                    ItemContent::Text => {
                        if let Some(t) = &mut item.text {
                            texts_mut_text(t, f);
                        }
                    }
                    ItemContent::Blocks => texts_mut_blocks(&mut item.blocks, f),
                }
            }
        }
        K::Quote => {
            match b.quote_content {
                QuoteContent::Text => texts_mut_text(&mut b.text, f),
                QuoteContent::Blocks => texts_mut_blocks(&mut b.blocks, f),
            }
            texts_mut_text(&mut b.quote_caption, f);
        }
        K::Photo | K::Video | K::Audio | K::File | K::Map | K::InputMap => {
            texts_mut_caption(&mut b.caption, f)
        }
        K::Cover => {
            if let Some(first) = b.blocks.first_mut() {
                texts_mut_block(first, f);
            }
        }
        K::Embed => texts_mut_caption(&mut b.caption, f),
        K::EmbedPost | K::Collage | K::Slideshow => {
            texts_mut_blocks(&mut b.blocks, f);
            texts_mut_caption(&mut b.caption, f);
        }
        K::Table => {
            texts_mut_text(&mut b.text, f);
            for row in &mut b.table_rows {
                for cell in &mut row.cells {
                    if let Some(t) = &mut cell.text {
                        texts_mut_text(t, f);
                    }
                }
            }
        }
        K::Details => {
            texts_mut_text(&mut b.text, f);
            texts_mut_blocks(&mut b.blocks, f);
        }
        K::RelatedArticles => texts_mut_text(&mut b.text, f),
        K::ButtonRow => {
            for button in &mut b.buttons {
                texts_mut_text(button, f);
            }
        }
        K::Unsupported | K::Divider | K::Anchor | K::Channel | K::Math | K::Unknown => {}
    }
}

pub fn texts_mut(message: &mut RichMessage, f: &mut impl FnMut(&mut RichText)) {
    texts_mut_blocks(&mut message.blocks, f);
}
