//! Runs hand-made messages through both writers, so the output can be checked
//! without logging in to Telegram.
use crate::html_out::HtmlWriter;
use crate::model::*;
use crate::output::{JsonWriter, Writer};
use crate::parse::parse_text;

fn msg(id: i32, from: Option<PeerId>, date: i32, text: &str) -> Message {
    Message {
        id,
        out: false,
        date,
        edited: 0,
        from,
        signature: String::new(),
        fwd_from: None,
        fwd_name: String::new(),
        fwd_date: 0,
        forwarded: false,
        saved_from: None,
        reply_to: 0,
        reply_to_peer: None,
        via_bot: None,
        text: parse_text(text, &[]),
        media: Media::None,
        action: None,
        buttons: vec![],
        reactions: vec![],
        rich: None,
        unsupported: false,
    }
}

#[test]
fn render_sample() {
    let out = std::path::PathBuf::from("/tmp/tgexport_selftest");
    let _ = std::fs::remove_dir_all(&out);
    let mut peers = Peers::default();
    peers.map.insert(
        PeerId::User(1),
        PeerInfo {
            first_name: "Ann".into(),
            last_name: "Lee".into(),
            ..Default::default()
        },
    );
    peers.map.insert(
        PeerId::User(2),
        PeerInfo {
            first_name: "Bob".into(),
            ..Default::default()
        },
    );
    let d = DialogInfo {
        peer: PeerId::User(2),
        dtype: DialogType::Personal,
        name: "Bob".into(),
        is_left: false,
        only_my_messages: false,
        migrated_from: None,
        relative_path: "chats/chat_1/".into(),
        splits: vec![0],
    };
    let mut m1 = msg(
        1,
        Some(PeerId::User(1)),
        1_700_000_000,
        "Hello <b>world</b> & \"quotes\"\nline2",
    );
    m1.reactions.push(Reaction {
        emoji: "👍".into(),
        custom: None,
        recent: vec![(PeerId::User(2), 1_700_000_050)],
        count: 2,
    });
    let mut m2 = msg(2, Some(PeerId::User(2)), 1_700_000_100, "reply");
    m2.reply_to = 1;
    let mut m3 = msg(3, Some(PeerId::User(2)), 1_700_100_000, "");
    m3.media = Media::Geo {
        point: Some((51.5, -0.12)),
        live_period: 0,
    };
    let mut m4 = msg(4, Some(PeerId::User(1)), 1_700_100_050, "");
    m4.action = Some(Action::ChatEditTitle {
        title: "New".into(),
    });
    let saved = |path: &str, size: i64| FileRef {
        source: Source::None,
        class: MediaClass::File,
        size,
        suggested: path.into(),
        relative_path: path.into(),
        skip: None,
    };
    let mut m5 = msg(5, Some(PeerId::User(1)), 1_700_100_100, "");
    m5.media = Media::Document {
        file: saved("chats/chat_1/video_files/video_1.mp4", 1000),
        name: String::new(),
        mime: "video/mp4".into(),
        duration: 12,
        width: 640,
        height: 360,
        flags: DocFlags {
            video_file: true,
            ..Default::default()
        },
        sticker_emoji: String::new(),
        performer: String::new(),
        title: String::new(),
        spoiler: false,
        ttl: 0,
        thumb: Some(Thumb {
            file: saved("chats/chat_1/video_files/video_1.mp4_thumb.jpg", 50),
        }),
    };
    let mut m6 = msg(6, Some(PeerId::User(2)), 1_700_100_200, "");
    m6.text = vec![
        TextPart {
            kind: TextKind::CustomEmoji,
            text: "x".into(),
            additional: "chats/chat_1/stickers/e.webp".into(),
        },
        TextPart {
            kind: TextKind::CustomEmoji,
            text: "y".into(),
            additional: "(unavailable)".into(),
        },
    ];
    let msgs = vec![m1, m2, m3, m4, m5, m6];

    let mut ws: Vec<Box<dyn Writer>> = vec![
        Box::new(JsonWriter::create(&out, false).unwrap()),
        Box::new(HtmlWriter::create(&out, false).unwrap()),
    ];
    for w in ws.iter_mut() {
        w.personal(&PersonalInfo {
            user_id: 1,
            first_name: "Ann".into(),
            last_name: "Lee".into(),
            phone: "+100".into(),
            username: "ann".into(),
            bio: "hi".into(),
        })
        .unwrap();
        w.contacts(&ContactsList {
            contacts: vec![Contact {
                first_name: "Bob".into(),
                last_name: "".into(),
                phone: "+200".into(),
                date: 1_700_000_000,
            }],
            correspondents: vec![TopPeerEntry {
                id: PeerId::User(2),
                name: "Bob".into(),
                kind: "user",
                rating: 1.5,
            }],
            ..Default::default()
        })
        .unwrap();
        w.sessions(&SessionsList {
            list: vec![Session {
                last_active: 1_700_000_000,
                ip: "1.2.3.4".into(),
                country: "DE".into(),
                region: "Berlin".into(),
                app_name: "tgexport".into(),
                app_version: "0.1".into(),
                device_model: "PC".into(),
                platform: "Linux".into(),
                system_version: "6".into(),
                created: 1_690_000_000,
            }],
            web_list: vec![],
        })
        .unwrap();
        w.dialogs_start(false).unwrap();
        w.dialog_start(&d, &peers).unwrap();
        w.messages(&d, &msgs, &peers).unwrap();
        w.dialog_end().unwrap();
        w.dialogs_end().unwrap();
    }
    for w in ws {
        w.finish().unwrap();
    }
    assert!(out.join("result.json").exists());
    assert!(out.join("export_results.html").exists());
    let json = std::fs::read_to_string(out.join("result.json")).unwrap();
    assert!(json.contains("\"thumbnail\": \"chats/chat_1/video_files/video_1.mp4_thumb.jpg\""));
    assert!(json.contains("\"thumbnail_file_size\": 50"));
    assert!(json.contains("\"recent\": ["));
    assert!(json.contains("\"document_id\": \"chats/chat_1/stickers/e.webp\""));
    let html = std::fs::read_to_string(out.join("chats/chat_1/messages.html")).unwrap();
    assert!(html.contains("video_file_wrap"));
    assert!(html.contains("src=\"../../chats/chat_1/video_files/video_1.mp4_thumb.jpg\""));
    assert!(html.contains("href=\"../../chats/chat_1/stickers/e.webp\""));
    assert!(html.contains("ShowNotAvailableEmoji"));
    assert!(out.join("chats/chat_1/messages.html").exists());
}

#[test]
fn single_chat_layout() {
    let out = std::path::PathBuf::from("/tmp/tgexport_selftest_single");
    let _ = std::fs::remove_dir_all(&out);
    let peers = Peers::default();
    let d = DialogInfo {
        peer: PeerId::User(2),
        dtype: DialogType::Personal,
        name: "Bob".into(),
        is_left: false,
        only_my_messages: false,
        migrated_from: None,
        relative_path: String::new(),
        splits: vec![0],
    };
    let msgs = vec![msg(1, Some(PeerId::User(2)), 1_700_000_000, "hi")];
    let mut ws: Vec<Box<dyn Writer>> = vec![
        Box::new(JsonWriter::create(&out, true).unwrap()),
        Box::new(HtmlWriter::create(&out, true).unwrap()),
    ];
    for w in ws.iter_mut() {
        w.dialogs_start(false).unwrap();
        w.dialog_start(&d, &peers).unwrap();
        w.messages(&d, &msgs, &peers).unwrap();
        w.dialog_end().unwrap();
        w.dialogs_end().unwrap();
    }
    for w in ws {
        w.finish().unwrap();
    }
    let json = std::fs::read_to_string(out.join("result.json")).unwrap();
    assert!(json.starts_with("{\n \"name\": \"Bob\""), "{json}");
    assert!(out.join("messages.html").exists());
    assert!(!out.join("export_results.html").exists());
}
