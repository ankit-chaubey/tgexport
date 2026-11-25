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

//! The media boxes of the HTML pages: photos, stickers, videos, files and the
//! generic title / description / status box. They follow Telegram Desktop's
//! page writer, and also serve the media inside rich messages.

use std::path::Path;

use image::ImageFormat;

use crate::actions::CallState;
use crate::model::*;
use crate::rich::*;
use crate::rich_html::{
    Html, MediaData, MediaRenderer, attrs, escape, file_view, no_file_description,
};
use crate::thumbs::{self, ENTRY_USERPIC_SIZE, PHOTO_MAX, STICKER_MAX};

/// A file as the page sees it: where it is, or why it is missing.
#[derive(Clone, Debug, Default)]
pub struct FileV {
    pub path: String,
    pub skip: Option<SkipReason>,
    pub size: i64,
}

impl FileV {
    /// The file exactly as it was saved or skipped.
    pub fn plain(f: &FileRef) -> Self {
        FileV {
            path: f.relative_path.clone(),
            skip: f.skip,
            size: f.size,
        }
    }

    /// The way rich messages show files: anything not saved and not skipped
    /// on purpose counts as unavailable.
    pub fn rich(f: Option<&FileRef>) -> Self {
        let v = file_view(f);
        FileV {
            path: v.path,
            skip: v.skip,
            size: v.size,
        }
    }
}

pub struct PhotoV {
    pub file: FileV,
    pub width: i32,
    pub height: i32,
}

#[derive(Default)]
pub struct DocV {
    pub file: FileV,
    pub thumb: FileV,
    pub name: String,
    pub duration: i32,
    pub width: i32,
    pub height: i32,
    pub sticker_emoji: String,
    pub performer: String,
    pub title: String,
}

pub fn format_file_size(size: i64) -> String {
    let tenths = |v: i64| format!("{}.{}", v / 10, v % 10);
    if size >= 1024 * 1024 {
        format!("{} MB", tenths(size * 10 / (1024 * 1024)))
    } else if size >= 1024 {
        format!("{} KB", tenths(size * 10 / 1024))
    } else {
        format!("{size} B")
    }
}

pub fn format_duration(seconds: i32) -> String {
    let (h, m, s) = (seconds / 3600, (seconds % 3600) / 60, seconds % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

fn format_image_size(w: i32, h: i32) -> String {
    format!("{w}\u{D7}{h}")
}

fn is_global_link(link: &str) -> bool {
    let l = link.to_lowercase();
    l.starts_with("http://") || l.starts_with("https://")
}

pub fn audio_data(d: &DocV) -> MediaData {
    let has_file = !d.file.path.is_empty();
    let title = if !d.performer.is_empty() && !d.title.is_empty() {
        format!("{} \u{2013} {}", d.performer, d.title)
    } else if !d.name.is_empty() {
        d.name.clone()
    } else {
        "Audio file".to_string()
    };
    let mut status = format_duration(d.duration);
    if !has_file {
        status.push_str(&format!(", {}", format_file_size(d.file.size)));
    }
    MediaData {
        title,
        status,
        classes: "media_audio_file".into(),
        link: d.file.path.clone(),
        description: no_file_description(d.file.skip).into(),
        ..Default::default()
    }
}

pub fn file_data(d: &DocV) -> MediaData {
    MediaData {
        title: if d.name.is_empty() {
            "File".into()
        } else {
            d.name.clone()
        },
        status: format_file_size(d.file.size),
        classes: "media_file".into(),
        link: d.file.path.clone(),
        description: no_file_description(d.file.skip).into(),
        ..Default::default()
    }
}

pub struct MediaPage<'a> {
    pub root: &'a Path,
    /// Prefix that leads from the page to the export folder.
    pub base: &'a str,
}

impl MediaPage<'_> {
    fn relative(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }

    pub fn generic_box(&self, html: &mut Html, data: &MediaData) -> String {
        let mut r = html.push_tag("div", attrs(&[("class", "media_wrap clearfix")]));
        if data.link.is_empty() {
            r += &html.push_tag(
                "div",
                attrs(&[(
                    "class",
                    &format!("media clearfix pull_left {}", data.classes),
                )]),
            );
        } else {
            let href = if is_global_link(&data.link) {
                data.link.clone()
            } else {
                self.relative(&data.link)
            };
            r += &html.push_tag(
                "a",
                attrs(&[
                    (
                        "class",
                        &format!("media clearfix pull_left block_link {}", data.classes),
                    ),
                    ("href", &href),
                ]),
            );
        }
        if data.thumb.is_empty() {
            r += &html.push_tag("div", attrs(&[("class", "fill pull_left")]));
            r += &html.pop_tag();
        } else {
            r += &html.push_tag(
                "img",
                attrs(&[
                    ("class", "thumb pull_left"),
                    ("src", &self.relative(&data.thumb)),
                    ("empty", ""),
                ]),
            );
        }
        r += &html.push_tag("div", attrs(&[("class", "body")]));
        for (class, text) in [
            ("title bold", &data.title),
            ("description", &data.description),
            ("status details", &data.status),
        ] {
            if !text.is_empty() {
                r += &html.push_tag("div", attrs(&[("class", class)]));
                r += &escape(text);
                r += &html.pop_tag();
            }
        }
        r += &html.pop_tag();
        r += &html.pop_tag();
        r += &html.pop_tag();
        r
    }

    fn image_box(
        &self,
        html: &mut Html,
        wrap_class: &str,
        image_class: &str,
        href_path: &str,
        src: &str,
        size: (i32, i32),
    ) -> String {
        let mut r = html.push_tag("div", attrs(&[("class", "media_wrap clearfix")]));
        r += &html.push_tag(
            "a",
            attrs(&[
                ("class", &format!("{wrap_class} clearfix pull_left")),
                ("href", &self.relative(href_path)),
            ]),
        );
        let style = format!("width: {}px; height: {}px", size.0 / 2, size.1 / 2);
        r += &html.push_tag(
            "img",
            attrs(&[
                ("class", image_class),
                ("style", &style),
                ("src", &self.relative(src)),
                ("empty", ""),
            ]),
        );
        r += &html.pop_tag();
        r += &html.pop_tag();
        r
    }

    pub fn photo_box(&self, html: &mut Html, p: &PhotoV) -> String {
        let convert = thumbs::calculate_thumb_size(PHOTO_MAX, PHOTO_MAX, false);
        if let Some((thumb, size)) =
            thumbs::write_image_thumb(self.root, &p.file.path, &convert, None, "_thumb")
        {
            return self.image_box(html, "photo_wrap", "photo", &p.file.path, &thumb, size);
        }
        let mut generic = MediaData {
            title: "Photo".into(),
            status: format_image_size(p.width, p.height),
            description: no_file_description(p.file.skip).into(),
            classes: "media_photo".into(),
            ..Default::default()
        };
        if p.file.path.is_empty() {
            generic
                .status
                .push_str(&format!(", {}", format_file_size(p.file.size)));
        } else {
            generic.link = p.file.path.clone();
        }
        self.generic_box(html, &generic)
    }

    pub fn sticker_box(&self, html: &mut Html, d: &DocV) -> String {
        let convert = thumbs::calculate_thumb_size(STICKER_MAX, STICKER_MAX, false);
        if let Some((thumb, size)) = thumbs::write_image_thumb(
            self.root,
            &d.file.path,
            &convert,
            Some(ImageFormat::Png),
            "_thumb",
        ) {
            return self.image_box(html, "sticker_wrap", "sticker", &d.file.path, &thumb, size);
        }
        let mut generic = MediaData {
            title: "Sticker".into(),
            status: d.sticker_emoji.clone(),
            description: no_file_description(d.file.skip).into(),
            classes: "media_photo".into(),
            ..Default::default()
        };
        if d.file.path.is_empty() {
            if !generic.status.is_empty() {
                generic.status.push_str(", ");
            }
            generic.status.push_str(&format_file_size(d.file.size));
        } else {
            generic.link = d.file.path.clone();
        }
        self.generic_box(html, &generic)
    }

    fn preview_size(&self, d: &DocV) -> Option<(i32, i32)> {
        thumbs::calculate_thumb_size(PHOTO_MAX, PHOTO_MAX, true)((d.width, d.height))
    }

    pub fn animated_box(&self, html: &mut Html, d: &DocV) -> String {
        let size = self.preview_size(d);
        let Some(size) = size.filter(|_| !d.thumb.path.is_empty() && !d.file.path.is_empty())
        else {
            let generic = MediaData {
                title: "Animation".into(),
                status: format_file_size(d.file.size),
                link: d.file.path.clone(),
                description: no_file_description(d.file.skip).into(),
                classes: "media_video".into(),
                ..Default::default()
            };
            return self.generic_box(html, &generic);
        };
        let mut r = html.push_tag("div", attrs(&[("class", "media_wrap clearfix")]));
        r += &html.push_tag(
            "a",
            attrs(&[
                ("class", "animated_wrap clearfix pull_left"),
                ("href", &self.relative(&d.file.path)),
            ]),
        );
        r += &html.push_tag("div", attrs(&[("class", "video_play_bg")]));
        r += &html.push_tag("div", attrs(&[("class", "gif_play")]));
        r += "GIF";
        r += &html.pop_tag();
        r += &html.pop_tag();
        let style = format!("width: {}px; height: {}px", size.0 / 2, size.1 / 2);
        r += &html.push_tag(
            "img",
            attrs(&[
                ("class", "animated"),
                ("style", &style),
                ("src", &self.relative(&d.thumb.path)),
                ("empty", ""),
            ]),
        );
        r += &html.pop_tag();
        r += &html.pop_tag();
        r
    }

    pub fn video_file_box(&self, html: &mut Html, d: &DocV) -> String {
        let size = self.preview_size(d);
        let Some(size) = size.filter(|_| !d.thumb.path.is_empty() && !d.file.path.is_empty())
        else {
            let mut generic = MediaData {
                title: "Video file".into(),
                status: format_duration(d.duration),
                description: no_file_description(d.file.skip).into(),
                classes: "media_video".into(),
                ..Default::default()
            };
            if d.file.path.is_empty() {
                generic
                    .status
                    .push_str(&format!(", {}", format_file_size(d.file.size)));
            } else {
                generic.link = d.file.path.clone();
            }
            return self.generic_box(html, &generic);
        };
        let mut r = html.push_tag("div", attrs(&[("class", "media_wrap clearfix")]));
        r += &html.push_tag(
            "a",
            attrs(&[
                ("class", "video_file_wrap clearfix pull_left"),
                ("href", &self.relative(&d.file.path)),
            ]),
        );
        r += &html.push_tag("div", attrs(&[("class", "video_play_bg")]));
        r += &html.push_tag("div", attrs(&[("class", "video_play")]));
        r += &html.pop_tag();
        r += &html.pop_tag();
        r += &html.push_tag("div", attrs(&[("class", "video_duration")]));
        r += &format_duration(d.duration);
        r += &html.pop_tag();
        let style = format!("width: {}px; height: {}px", size.0 / 2, size.1 / 2);
        r += &html.push_tag(
            "img",
            attrs(&[
                ("class", "video_file"),
                ("style", &style),
                ("src", &self.relative(&d.thumb.path)),
                ("empty", ""),
            ]),
        );
        r += &html.pop_tag();
        r += &html.pop_tag();
        r
    }
}

// Rich messages: the same boxes, with files presented the rich way.

fn rich_photo(p: Option<&RichPhoto>) -> PhotoV {
    PhotoV {
        file: FileV::rich(p.map(|p| &p.file)),
        width: p.map(|p| p.width).unwrap_or(0),
        height: p.map(|p| p.height).unwrap_or(0),
    }
}

fn rich_document(d: Option<&RichDocument>) -> DocV {
    let Some(d) = d else {
        return DocV {
            file: FileV::rich(None),
            thumb: FileV::rich(None),
            ..Default::default()
        };
    };
    DocV {
        file: FileV::rich(Some(&d.file)),
        thumb: FileV::rich(d.thumb.as_ref().map(|t| &t.file)),
        name: d.name.clone(),
        duration: d.duration,
        width: d.width,
        height: d.height,
        sticker_emoji: d.sticker_emoji.clone(),
        performer: d.performer.clone(),
        title: d.title.clone(),
    }
}

impl MediaRenderer for MediaPage<'_> {
    fn photo(&self, html: &mut Html, photo: Option<&RichPhoto>) -> String {
        self.photo_box(html, &rich_photo(photo))
    }

    fn video(&self, html: &mut Html, document: Option<&RichDocument>) -> String {
        let mut d = rich_document(document);
        if !d.thumb.path.is_empty() && !thumbs::can_read_image(self.root, &d.thumb.path) {
            d.thumb.path.clear();
        }
        self.video_file_box(html, &d)
    }

    fn audio(&self, html: &mut Html, document: Option<&RichDocument>) -> String {
        self.generic_box(html, &audio_data(&rich_document(document)))
    }

    fn file(&self, html: &mut Html, document: Option<&RichDocument>) -> String {
        self.generic_box(html, &file_data(&rich_document(document)))
    }

    fn generic(&self, html: &mut Html, data: &MediaData) -> String {
        self.generic_box(html, data)
    }

    fn photo_card(&self, html: &mut Html, data: &MediaData, photo: Option<&RichPhoto>) -> String {
        let presentation = rich_photo(photo);
        let path = presentation.file.path;
        let mut data = data.clone();
        data.link = path.clone();
        data.thumb = if path.is_empty() {
            String::new()
        } else {
            let side = ENTRY_USERPIC_SIZE * 2;
            thumbs::write_image_thumb(
                self.root,
                &path,
                &|_| Some((side, side)),
                None,
                "_rich_card_thumb",
            )
            .map(|(p, _)| p)
            .unwrap_or_default()
        };
        self.generic_box(html, &data)
    }
}

// Media of ordinary messages.

use crate::html_out::format_text;

fn money(amount: i64, currency: &str) -> String {
    format!("{:.2} {}", amount as f64 / 100.0, currency)
}

use crate::phone::format_phone as phone;

fn maps_link(lat: f64, lon: f64) -> String {
    let coords = format!("{lat:.6},{lon:.6}");
    format!("https://maps.google.com/maps?q={coords}&ll={coords}&z=16")
}

fn doc_view(file: &FileRef, thumb: &Option<Thumb>, d: &DocParts) -> DocV {
    DocV {
        file: FileV::plain(file),
        thumb: thumb
            .as_ref()
            .map(|t| FileV::plain(&t.file))
            .unwrap_or_default(),
        name: d.name.to_string(),
        duration: d.duration,
        width: d.width,
        height: d.height,
        sticker_emoji: d.sticker_emoji.to_string(),
        performer: d.performer.to_string(),
        title: d.title.to_string(),
    }
}

struct DocParts<'a> {
    name: &'a str,
    duration: i32,
    width: i32,
    height: i32,
    sticker_emoji: &'a str,
    performer: &'a str,
    title: &'a str,
}

/// The title / description / status box of a message, when its media is
/// shown that way. An empty `classes` means the media has its own markup.
fn prepare_media_data(m: &Message, dialog: &DialogInfo, peers: &Peers) -> MediaData {
    let mut r = MediaData::default();
    if let Some(Action::PhoneCall {
        state, duration, ..
    }) = &m.action
    {
        r.classes = "media_call".into();
        let who = if m.out {
            Some(dialog.peer)
        } else {
            peers.map.iter().find(|(_, p)| p.is_self).map(|(id, _)| *id)
        };
        r.title = who.map(|w| peers.name(w)).unwrap_or_default();
        r.status = match state {
            CallState::Invitation => "Invitation",
            CallState::Active => "Ongoing",
            _ if m.out => {
                if *state == CallState::Missed {
                    "Cancelled"
                } else {
                    "Outgoing"
                }
            }
            CallState::Missed => "Missed",
            CallState::Busy => "Declined",
            _ => "Incoming",
        }
        .to_string();
        if *duration > 0 {
            r.classes.push_str(" success");
            r.status.push_str(&format!(" ({duration} seconds)"));
        }
        return r;
    }
    match &m.media {
        Media::Photo { file, ttl, .. } => {
            if *ttl != 0 {
                r.title = "Self-destructing photo".into();
                r.status = if matches!(file.source, Source::Photo(_)) {
                    "Please view it on your mobile"
                } else {
                    "Expired"
                }
                .into();
                r.classes = "media_photo".into();
            }
        }
        Media::Document {
            file,
            name,
            duration,
            width,
            height,
            flags,
            sticker_emoji,
            performer,
            title,
            ttl,
            thumb,
            ..
        } => {
            if *ttl != 0 {
                r.title = "Self-destructing video".into();
                r.status = if matches!(file.source, Source::Document(_)) {
                    "Please view it on your mobile"
                } else {
                    "Expired"
                }
                .into();
                r.classes = "media_video".into();
                return r;
            }
            let has_file = !file.relative_path.is_empty();
            let parts = DocParts {
                name,
                duration: *duration,
                width: *width,
                height: *height,
                sticker_emoji,
                performer,
                title,
            };
            let view = doc_view(file, thumb, &parts);
            r.link = file.relative_path.clone();
            r.description = no_file_description(file.skip).into();
            if flags.sticker {
                // Shown as a sticker image, if it can be.
            } else if flags.video_message {
                r.title = "Video message".into();
                r.status = format_duration(*duration);
                if !has_file {
                    r.status
                        .push_str(&format!(", {}", format_file_size(file.size)));
                }
                r.thumb = view.thumb.path.clone();
                r.classes = "media_video".into();
            } else if flags.voice {
                r.title = "Voice message".into();
                r.status = format_duration(*duration);
                if !has_file {
                    r.status
                        .push_str(&format!(", {}", format_file_size(file.size)));
                }
                r.classes = "media_voice_message".into();
            } else if flags.animated || flags.video_file {
                // Shown as a preview, if there is one.
            } else if flags.audio {
                r = audio_data(&view);
            } else {
                r = file_data(&view);
            }
        }
        Media::Contact {
            first_name,
            last_name,
            phone: number,
            vcard,
        } => {
            r.title = format!("{first_name} {last_name}");
            r.classes = "media_contact".into();
            r.status = phone(number);
            if let Some(v) = vcard {
                r.status.push_str(" - vCard");
                r.link = v.relative_path.clone();
            }
        }
        Media::Geo { point, live_period } => {
            if *live_period != 0 {
                r.classes = "media_live_location".into();
                r.title = "Live location".into();
            } else {
                r.classes = "media_location".into();
                r.title = "Location".into();
            }
            if let Some((lat, lon)) = point {
                r.status = format!("{lat:.6}, {lon:.6}");
                r.link = maps_link(*lat, *lon);
            }
        }
        Media::Venue {
            point,
            title,
            address,
        } => {
            r.classes = "media_venue".into();
            r.title = title.clone();
            r.description = address.clone();
            if let Some((lat, lon)) = point {
                r.link = maps_link(*lat, *lon);
            }
        }
        Media::Game {
            title,
            description,
            bot_id,
            short_name,
        } => {
            r.classes = "media_game".into();
            r.title = title.clone();
            r.description = description.clone();
            if *bot_id != 0 && !short_name.is_empty() {
                if let Some(bot) = peers.get(PeerId::User(*bot_id)) {
                    if bot.is_bot && !bot.username.is_empty() {
                        let link = format!("https://t.me/{}?game={}", bot.username, short_name);
                        r.link = link.clone();
                        r.status = link;
                    }
                }
            }
        }
        Media::Invoice {
            title,
            description,
            amount,
            currency,
            ..
        } => {
            r.classes = "media_invoice".into();
            r.title = title.clone();
            r.description = description.clone();
            r.status = money(*amount, currency);
        }
        Media::Paid { stars } => {
            r.classes = "media_invoice".into();
            r.status = format!("{stars} XTR");
        }
        Media::None
        | Media::Poll(_)
        | Media::Todo { .. }
        | Media::GiveawayStart { .. }
        | Media::GiveawayResults { .. }
        | Media::Unsupported => {}
    }
    r
}

fn poll_html(html: &mut Html, p: &Poll, base: &str) -> String {
    let votes = |count: i32| -> String {
        if count > 1 {
            format!("{count} votes")
        } else if count > 0 {
            format!("{count} vote")
        } else {
            "No votes".to_string()
        }
    };
    let mut r = html.push_tag("div", attrs(&[("class", "media_wrap clearfix")]));
    r += &html.push_tag("div", attrs(&[("class", "media_poll")]));
    r += &html.push_tag("div", attrs(&[("class", "question bold")]));
    r += &format_text(&p.question, base);
    r += &html.pop_tag();
    r += &html.push_tag("div", attrs(&[("class", "details")]));
    r += if p.closed {
        "Final results"
    } else {
        "Anonymous poll"
    };
    r += &html.pop_tag();
    for (text, count, chosen) in &p.answers {
        r += &html.push_tag("div", attrs(&[("class", "answer")]));
        r += "- ";
        r += &format_text(text, base);
        if *count > 0 {
            r += &format!(
                " <span class=\"details\">{}{}</span>",
                votes(*count),
                if *chosen { ", chosen vote" } else { "" }
            );
        }
        r += &html.pop_tag();
    }
    r += &html.push_tag("div", attrs(&[("class", "total details ")]));
    r += &votes(p.total_votes);
    r += &html.pop_tag();
    r += &html.pop_tag();
    r += &html.pop_tag();
    r
}

/// Markup for the media of a message (`pushMedia`).
pub fn message_media(
    m: &Message,
    dialog: &DialogInfo,
    peers: &Peers,
    root: &Path,
    base: &str,
    depth: usize,
    link: &dyn Fn(i32, &str) -> String,
) -> String {
    let mut html = Html::with_depth(depth);
    let page = MediaPage { root, base };
    let data = prepare_media_data(m, dialog, peers);
    if !data.classes.is_empty() {
        return page.generic_box(&mut html, &data);
    }
    match &m.media {
        Media::Document {
            file,
            name,
            duration,
            width,
            height,
            flags,
            sticker_emoji,
            performer,
            title,
            thumb,
            ..
        } => {
            let parts = DocParts {
                name,
                duration: *duration,
                width: *width,
                height: *height,
                sticker_emoji,
                performer,
                title,
            };
            let view = doc_view(file, thumb, &parts);
            if flags.sticker {
                page.sticker_box(&mut html, &view)
            } else if flags.animated {
                page.animated_box(&mut html, &view)
            } else if flags.video_file {
                page.video_file_box(&mut html, &view)
            } else {
                String::new()
            }
        }
        Media::Photo {
            file,
            width,
            height,
            ..
        } => page.photo_box(
            &mut html,
            &PhotoV {
                file: FileV::plain(file),
                width: *width,
                height: *height,
            },
        ),
        Media::Poll(p) => poll_html(&mut html, p, base),
        Media::Todo { title, items, .. } => todo_html(&mut html, title, items, base),
        Media::GiveawayStart {
            countries,
            channels,
            additional_prize,
            until_date,
            credits,
            quantity,
            months,
            all,
        } => giveaway_start_html(
            &mut html,
            peers,
            countries,
            channels,
            additional_prize,
            *until_date,
            *credits,
            *quantity,
            *months,
            *all,
        ),
        Media::GiveawayResults {
            winners,
            winners_count,
            launch_id,
            credits,
            unclaimed_count,
            ..
        } => giveaway_results_html(
            &mut html,
            peers,
            winners,
            *winners_count,
            *launch_id,
            *credits,
            *unclaimed_count,
            link,
        ),
        _ => String::new(),
    }
}

// Checklists and giveaways.

fn wrap_peer_name_html(peers: &Peers, id: PeerId) -> String {
    let name = peers.name(id);
    if name.is_empty() {
        "Deleted".into()
    } else {
        escape(&name)
    }
}

fn todo_html(
    html: &mut Html,
    title: &Text,
    items: &[crate::actions::TodoItem],
    base: &str,
) -> String {
    let mut r = html.push_tag("div", attrs(&[("class", "media_wrap clearfix")]));
    r += &html.push_tag("div", attrs(&[("class", "media_poll")]));
    r += &html.push_tag("div", attrs(&[("class", "question bold")]));
    r += &format_text(title, base);
    r += &html.pop_tag();
    r += &html.push_tag("div", attrs(&[("class", "details")]));
    r += "To-do List";
    r += &html.pop_tag();
    for item in items {
        r += &html.push_tag("div", attrs(&[("class", "answer")]));
        r += "- ";
        r += &format_text(&item.text, base);
        r += &html.pop_tag();
    }
    r += &html.pop_tag();
    r += &html.pop_tag();
    r
}

#[allow(clippy::too_many_arguments)]
fn giveaway_start_html(
    html: &mut Html,
    peers: &Peers,
    countries: &[String],
    channels: &[i64],
    additional_prize: &str,
    until_date: i32,
    credits: i64,
    quantity: i32,
    months: i32,
    all: bool,
) -> String {
    let mut r = html.push_tag("div", attrs(&[("class", "media_wrap clearfix")]));
    r += &html.push_tag("div", attrs(&[("class", "media_giveaway")]));
    r += &html.push_tag("div", attrs(&[("class", "section_title bold")]));
    r += if quantity > 1 { "Giveaway Prizes" } else { "Giveaway Prize" };
    r += &html.pop_tag();
    r += &html.push_tag("div", attrs(&[("class", "section_body")]));
    r += &format!("<b>{quantity}</b> {}", escape(additional_prize));
    r += &html.pop_tag();
    r += &html.push_tag("div", attrs(&[("class", "section_title bold")]));
    r += "with";
    r += &html.pop_tag();
    r += &html.push_tag("div", attrs(&[("class", "section_body")]));
    if credits > 0 {
        r += &format!(
            "<b>{credits}{}</b> will be distributed ",
            if credits == 1 { " Star" } else { " Stars" }
        );
        if quantity == 1 {
            r += &format!("to <b>{quantity}</b> winner.");
        } else {
            r += &format!("among <b>{quantity}</b> winners.");
        }
    } else {
        r += &format!(
            "<b>{quantity}</b> {} for <b>{months}</b> {}",
            if quantity > 1 {
                "Telegram Premium Subscriptions"
            } else {
                "Telegram Premium Subscription"
            },
            if months > 1 { "months." } else { "month." }
        );
    }
    r += &html.pop_tag();
    r += &html.push_tag("div", attrs(&[("class", "section_title bold")]));
    r += "Participants";
    r += &html.pop_tag();
    r += &html.push_tag("div", attrs(&[("class", "section_body")]));
    let mut names: Vec<String> = Vec::new();
    let (mut any_channel, mut any_group) = (false, false);
    for id in channels {
        let peer = PeerId::Channel(*id);
        if let Some(info) = peers.get(peer) {
            if info.is_broadcast {
                any_channel = true;
            } else if info.is_megagroup {
                any_group = true;
            }
        }
        names.push(format!("<b>{}</b>", wrap_peer_name_html(peers, peer)));
    }
    let many = names.len() > 1;
    let participants = match (all, any_group, any_channel) {
        (true, false, true) => {
            if many { "All subscribers of the channels:" } else { "All subscribers of the channel:" }
        }
        (true, true, false) => {
            if many { "All members of the groups:" } else { "All members of the group:" }
        }
        (true, true, true) => {
            if many { "All members of the groups and channels:" } else { "All members of the group:" }
        }
        (false, false, true) => {
            if many {
                "All users who joined the channels below after this date:"
            } else {
                "All users who joined the channel below after this date:"
            }
        }
        (false, true, false) => {
            if many {
                "All users who joined the groups below after this date:"
            } else {
                "All users who joined the group below after this date:"
            }
        }
        (false, true, true) => {
            if many {
                "All users who joined the groups and channels below after this date:"
            } else {
                "All users who joined the group below after this date:"
            }
        }
        _ => "",
    };
    r += &escape(participants);
    r += &names.join(", ");
    r += &html.pop_tag();
    let list: Vec<String> = countries
        .iter()
        .map(|c| {
            format!(
                "{}\u{A0}{}",
                crate::countries::flag_emoji(c),
                crate::countries::name_by_iso2(c)
            )
        })
        .collect();
    if !list.is_empty() {
        let mut united = list[0].clone();
        for (i, item) in list.iter().enumerate().skip(1) {
            united = if i + 1 == list.len() {
                format!("{united} and {item}")
            } else {
                format!("{united}, {item}")
            };
        }
        r += &html.push_tag("div", attrs(&[("class", "section_body")]));
        r += &escape(&format!("from {united}"));
        r += &html.pop_tag();
    }
    r += &html.push_tag("div", attrs(&[("class", "section_title bold")]));
    r += "Winners Selection Date";
    r += &html.pop_tag();
    r += &html.push_tag("div", attrs(&[("class", "section_body")]));
    r += &crate::html_out::date_time(until_date, false);
    r += &html.pop_tag();
    r += &html.pop_tag();
    r += &html.pop_tag();
    r
}

fn giveaway_results_html(
    html: &mut Html,
    peers: &Peers,
    winners: &[i64],
    winners_count: i32,
    launch_id: i32,
    credits: i64,
    unclaimed_count: i32,
    link: &dyn Fn(i32, &str) -> String,
) -> String {
    let mut r = html.push_tag("div", attrs(&[("class", "media_wrap clearfix")]));
    r += &html.push_tag("div", attrs(&[("class", "media_giveaway")]));
    r += &html.push_tag("div", attrs(&[("class", "section_title bold")]));
    r += if winners_count > 1 { "Winners Selected!" } else { "Winner Selected!" };
    r += &html.pop_tag();
    r += &html.push_tag("div", attrs(&[("class", "section_body")]));
    r += &format!(
        "<b>{winners_count}</b> {} of the {} was randomly selected by Telegram.",
        if winners_count > 1 { "winners" } else { "winner" },
        link(launch_id, "Giveaway")
    );
    r += &html.pop_tag();
    r += &html.push_tag("div", attrs(&[("class", "section_title bold")]));
    r += if winners_count > 1 { "Winners" } else { "Winner" };
    r += &html.pop_tag();
    r += &html.push_tag("div", attrs(&[("class", "section_body")]));
    let names: Vec<String> = winners
        .iter()
        .map(|w| format!("<b>{}</b>", wrap_peer_name_html(peers, PeerId::User(*w))))
        .collect();
    let and_more = if winners_count as usize > winners.len() {
        format!(" and {} more!", winners_count as usize - winners.len())
    } else {
        String::new()
    };
    r += &format!("{}{}", names.join(", "), and_more);
    r += &html.pop_tag();
    r += &html.push_tag("div", attrs(&[("class", "section_body")]));
    let single = credits == 1;
    r += &if credits != 0 && winners_count == 1 {
        format!(
            "The winner received <b>{credits}</b>{}",
            if single { " Star." } else { " Stars." }
        )
    } else if credits != 0 && winners_count > 1 {
        format!(
            "All winners received <b>{credits}</b>{}",
            if single { " Star in total." } else { " Stars in total." }
        )
    } else if unclaimed_count != 0 {
        "Some winners couldn't be selected.".to_string()
    } else if winners_count == 1 {
        "The winner received their gift link in a private message.".to_string()
    } else if winners_count > 1 {
        "All winners received gift links in private messages.".to_string()
    } else {
        String::new()
    };
    r += &html.pop_tag();
    r += &html.pop_tag();
    r += &html.pop_tag();
    r
}
