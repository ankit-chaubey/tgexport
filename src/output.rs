//! The interface both writers implement, and the JSON writer.

use std::fs::File;
use std::io::{self, BufWriter};
use std::path::Path;

use crate::about;
use crate::json_out::*;
use crate::model::*;

pub trait Writer {
    fn personal(&mut self, info: &PersonalInfo, userpics: &[Userpic]) -> io::Result<()>;
    fn contacts(&mut self, list: &[Contact]) -> io::Result<()>;
    fn dialogs_start(&mut self, left: bool) -> io::Result<()>;
    fn dialog_start(&mut self, d: &DialogInfo, peers: &Peers) -> io::Result<()>;
    fn messages(&mut self, d: &DialogInfo, msgs: &[Message], peers: &Peers) -> io::Result<()>;
    fn dialog_end(&mut self) -> io::Result<()>;
    fn dialogs_end(&mut self) -> io::Result<()>;
    fn finish(self: Box<Self>) -> io::Result<()>;
}

pub struct JsonWriter {
    stream: JsonStream<BufWriter<File>>,
}

impl JsonWriter {
    pub fn create(root: &Path) -> io::Result<Self> {
        std::fs::create_dir_all(root)?;
        let file = File::create(root.join("result.json"))?;
        let mut stream = JsonStream::new(BufWriter::new(file));
        stream.begin_object()?;
        stream.field("about", &s(about::ABOUT_TELEGRAM))?;
        Ok(JsonWriter { stream })
    }
}

impl Writer for JsonWriter {
    fn personal(&mut self, info: &PersonalInfo, userpics: &[Userpic]) -> io::Result<()> {
        let mut o = vec![
            ("user_id", n(info.user_id)),
            ("first_name", s(info.first_name.clone())),
            ("last_name", s(info.last_name.clone())),
            ("phone_number", s(info.phone.clone())),
            ("username", s(info.username.clone())),
            ("bio", s(info.bio.clone())),
        ];
        o.retain(|(_, v)| !matches!(v, J::Str(x) if x.is_empty()));
        self.stream.field("personal_information", &J::Obj(o))?;
        if !userpics.is_empty() {
            let list = userpics
                .iter()
                .map(|u| {
                    let mut o = vec![("date", s(iso_date(u.date)))];
                    o.push(("date_unixtime", s(u.date.to_string())));
                    if !u.file.relative_path.is_empty() {
                        o.push(("photo", s(u.file.relative_path.clone())));
                    }
                    J::Obj(o)
                })
                .collect();
            self.stream.field("profile_pictures", &J::Arr(list))?;
        }
        Ok(())
    }

    fn contacts(&mut self, list: &[Contact]) -> io::Result<()> {
        self.stream.key("contacts")?;
        self.stream.begin_object()?;
        self.stream.field("about", &s(about::ABOUT_CONTACTS))?;
        self.stream.key("list")?;
        self.stream.begin_array()?;
        for c in list {
            let mut o = vec![
                ("first_name", s(c.first_name.clone())),
                ("last_name", s(c.last_name.clone())),
                ("phone_number", s(c.phone.clone())),
            ];
            if c.date != 0 {
                o.push(("date", s(iso_date(c.date))));
                o.push(("date_unixtime", s(c.date.to_string())));
            }
            self.stream.array_item(&J::Obj(o))?;
        }
        self.stream.end()?;
        self.stream.end()
    }

    fn dialogs_start(&mut self, left: bool) -> io::Result<()> {
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
        self.stream.item()?;
        self.stream.begin_object()?;
        self.stream.field(
            "name",
            &if d.name.is_empty() {
                J::Null
            } else {
                s(d.name.clone())
            },
        )?;
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
        self.stream.end()?; // close the list array
        self.stream.end() // close the chats object
    }

    fn finish(mut self: Box<Self>) -> io::Result<()> {
        self.stream.end()?;
        self.stream.finish()
    }
}
