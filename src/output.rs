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

//! The interface both writers implement, and the JSON writer.

use std::fs::File;
use std::io::{self, BufWriter};
use std::path::Path;

use crate::about;
use crate::json_out::*;
use crate::model::*;

pub trait Writer {
    fn personal(&mut self, info: &PersonalInfo) -> io::Result<()>;
    fn userpics(&mut self, list: &[Userpic]) -> io::Result<()>;
    fn stories(&mut self, list: &[Story]) -> io::Result<()>;
    fn profile_music(&mut self, list: &[Message], peers: &Peers) -> io::Result<()>;
    fn contacts(&mut self, list: &ContactsList) -> io::Result<()>;
    fn sessions(&mut self, list: &SessionsList) -> io::Result<()>;
    /// `file` is the downloaded "other data" file, already saved under the export folder.
    fn other_data(&mut self, file: &FileRef) -> io::Result<()>;
    fn dialogs_start(&mut self, left: bool) -> io::Result<()>;
    fn dialog_start(&mut self, d: &DialogInfo, peers: &Peers) -> io::Result<()>;
    fn messages(&mut self, d: &DialogInfo, msgs: &[Message], peers: &Peers) -> io::Result<()>;
    fn dialog_end(&mut self) -> io::Result<()>;
    fn dialogs_end(&mut self) -> io::Result<()>;
    fn finish(self: Box<Self>) -> io::Result<()>;
}

/// Text shown instead of a path when a file was not saved.
fn skipped_text(reason: SkipReason, what: &str) -> String {
    match reason {
        SkipReason::Unavailable => format!("({what} unavailable, please try again later)"),
        SkipReason::FileSize => {
            format!("({what} exceeds maximum size. Change data exporting settings to download.)")
        }
        SkipReason::FileType => {
            format!("({what} not included. Change data exporting settings to download.)")
        }
    }
}

fn file_text(file: &FileRef, what: &str) -> String {
    if !file.relative_path.is_empty() {
        return file.relative_path.clone();
    }
    skipped_text(file.skip.unwrap_or(SkipReason::Unavailable), what)
}

fn dated(key: &'static str, unix_key: &'static str, ts: i32, o: &mut Vec<(&'static str, J)>) {
    if ts != 0 {
        o.push((key, s(iso_date(ts))));
        o.push((unix_key, s(ts.to_string())));
    }
}

pub struct JsonWriter {
    stream: JsonStream<BufWriter<File>>,
    root: std::path::PathBuf,
    /// A single chat is written as the top level object, without the usual wrapper.
    single: bool,
}

impl JsonWriter {
    pub fn create(root: &Path, single: bool) -> io::Result<Self> {
        std::fs::create_dir_all(root)?;
        let file = File::create(root.join("result.json"))?;
        let mut stream = JsonStream::new(BufWriter::new(file));
        if !single {
            stream.begin_object()?;
            stream.field("about", &s(about::ABOUT_TELEGRAM))?;
        }
        Ok(JsonWriter {
            stream,
            root: root.to_path_buf(),
            single,
        })
    }

    /// `{ "about": ..., "list": [ ... ] }` under `key`.
    fn list_section(&mut self, key: &str, about_text: &str, items: Vec<J>) -> io::Result<()> {
        self.stream.key(key)?;
        self.stream.begin_object()?;
        self.stream.field("about", &s(about_text))?;
        self.stream.key("list")?;
        self.stream.begin_array()?;
        for item in &items {
            self.stream.array_item(item)?;
        }
        self.stream.end()?;
        self.stream.end()
    }
}

fn name_or_null(name: &str) -> J {
    if name.is_empty() { J::Null } else { s(name) }
}

fn top_peers_json(list: &[TopPeerEntry], category: &str) -> Vec<J> {
    list.iter()
        .map(|t| {
            J::Obj(vec![
                ("id", n(t.id.bare())),
                ("category", s(category)),
                ("type", s(t.kind)),
                ("name", name_or_null(&t.name)),
                ("rating", float_allow_null(t.rating)),
            ])
        })
        .collect()
}

impl Writer for JsonWriter {
    fn personal(&mut self, info: &PersonalInfo) -> io::Result<()> {
        let mut o = vec![
            ("user_id", n(info.user_id)),
            ("first_name", s(info.first_name.clone())),
            ("last_name", s(info.last_name.clone())),
            ("phone_number", s(info.phone.clone())),
            ("username", s(info.username.clone())),
            ("bio", s(info.bio.clone())),
        ];
        o.retain(|(_, v)| !matches!(v, J::Str(x) if x.is_empty()));
        self.stream.field("personal_information", &J::Obj(o))
    }

    fn userpics(&mut self, list: &[Userpic]) -> io::Result<()> {
        let items = list
            .iter()
            .map(|u| {
                let mut o = Vec::new();
                dated("date", "date_unixtime", u.date, &mut o);
                o.push(("photo", s(file_text(&u.file, "Photo"))));
                J::Obj(o)
            })
            .collect();
        self.stream.field("profile_pictures", &J::Arr(items))
    }

    fn stories(&mut self, list: &[Story]) -> io::Result<()> {
        let items = list
            .iter()
            .map(|st| {
                let mut o = Vec::new();
                dated("date", "date_unixtime", st.date, &mut o);
                dated("expires", "expires_unixtime", st.expires, &mut o);
                o.push(("pinned", b(st.pinned)));
                o.push(("media", s(file_text(&st.file, "Photo"))));
                J::Obj(o)
            })
            .collect();
        self.stream.field("stories", &J::Arr(items))
    }

    fn profile_music(&mut self, list: &[Message], peers: &Peers) -> io::Result<()> {
        let items = list.iter().map(|m| message_json(m, peers)).collect();
        self.stream.field("profile_music", &J::Arr(items))
    }

    fn contacts(&mut self, list: &ContactsList) -> io::Result<()> {
        let mut sorted: Vec<&Contact> = list.contacts.iter().collect();
        sorted.sort_by_key(|c| (c.first_name.to_lowercase(), c.last_name.to_lowercase()));
        let items = sorted
            .into_iter()
            .map(|c| {
                let mut o = vec![
                    ("first_name", s(c.first_name.clone())),
                    ("last_name", s(c.last_name.clone())),
                    ("phone_number", s(c.phone.clone())),
                ];
                dated("date", "date_unixtime", c.date, &mut o);
                J::Obj(o)
            })
            .collect();
        self.list_section("contacts", about::ABOUT_CONTACTS, items)?;

        let mut frequent = top_peers_json(&list.correspondents, "people");
        frequent.extend(top_peers_json(&list.inline_bots, "inline_bots"));
        frequent.extend(top_peers_json(&list.phone_calls, "calls"));
        self.list_section("frequent_contacts", about::ABOUT_FREQUENT, frequent)
    }

    fn sessions(&mut self, list: &SessionsList) -> io::Result<()> {
        let sessions = list
            .list
            .iter()
            .map(|x| {
                let mut o = Vec::new();
                dated("last_active", "last_active_unixtime", x.last_active, &mut o);
                o.push(("last_ip", s(x.ip.clone())));
                o.push(("last_country", s(x.country.clone())));
                o.push(("last_region", s(x.region.clone())));
                o.push(("application_name", s(x.app_name.clone())));
                o.push(("application_version", s(x.app_version.clone())));
                o.push(("device_model", s(x.device_model.clone())));
                o.push(("platform", s(x.platform.clone())));
                o.push(("system_version", s(x.system_version.clone())));
                dated("created", "created_unixtime", x.created, &mut o);
                J::Obj(o)
            })
            .collect();
        self.list_section("sessions", about::ABOUT_SESSIONS, sessions)?;

        let web = list
            .web_list
            .iter()
            .map(|x| {
                let mut o = Vec::new();
                dated("last_active", "last_active_unixtime", x.last_active, &mut o);
                o.push(("last_ip", s(x.ip.clone())));
                o.push(("last_region", s(x.region.clone())));
                o.push(("bot_username", name_or_null(&x.bot_username)));
                o.push(("domain_name", name_or_null(&x.domain)));
                o.push(("browser", s(x.browser.clone())));
                o.push(("platform", s(x.platform.clone())));
                dated("created", "created_unixtime", x.created, &mut o);
                J::Obj(o)
            })
            .collect();
        self.list_section("web_sessions", about::ABOUT_WEB_SESSIONS, web)
    }

    fn other_data(&mut self, file: &FileRef) -> io::Result<()> {
        if file.relative_path.is_empty() {
            return Ok(());
        }
        let content = std::fs::read(self.root.join(&file.relative_path))?;
        if content.is_empty() {
            return Ok(());
        }
        let value: serde_json::Value = serde_json::from_slice(&content)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        self.stream.field("other_data", &from_value(&value))
    }

    fn dialogs_start(&mut self, left: bool) -> io::Result<()> {
        if self.single {
            return Ok(());
        }
        self.stream.key(if left { "left_chats" } else { "chats" })?;
        self.stream.begin_object()?;
        self.stream.field(
            "about",
            &s(if left {
                about::ABOUT_LEFT_CHATS
            } else {
                about::ABOUT_CHATS
            }),
        )?;
        self.stream.key("list")?;
        self.stream.begin_array()
    }

    fn dialog_start(&mut self, d: &DialogInfo, _peers: &Peers) -> io::Result<()> {
        if !self.single {
            self.stream.item()?;
        }
        self.stream.begin_object()?;
        if !matches!(
            d.dtype,
            DialogType::Saved | DialogType::Replies | DialogType::VerifyCodes
        ) {
            self.stream.field("name", &name_or_null(&d.name))?;
        }
        self.stream.field("type", &s(d.dtype.json_name()))?;
        self.stream.field("id", &n(d.peer.bare()))?;
        self.stream.key("messages")?;
        self.stream.begin_array()
    }

    fn messages(&mut self, _d: &DialogInfo, msgs: &[Message], peers: &Peers) -> io::Result<()> {
        for m in msgs {
            self.stream.array_item(&message_json(m, peers))?;
        }
        Ok(())
    }

    fn dialog_end(&mut self) -> io::Result<()> {
        self.stream.end()?; // close the messages array
        self.stream.end() // close the chat object
    }

    fn dialogs_end(&mut self) -> io::Result<()> {
        if self.single {
            return Ok(());
        }
        self.stream.end()?; // close the list array
        self.stream.end() // close the chats object
    }

    fn finish(mut self: Box<Self>) -> io::Result<()> {
        if !self.single {
            self.stream.end()?;
        }
        self.stream.finish()
    }
}
