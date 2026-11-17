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
        document_id: None,
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
    let msgs = vec![m1, m2, m3, m4];

    let mut ws: Vec<Box<dyn Writer>> = vec![
        Box::new(JsonWriter::create(&out).unwrap()),
        Box::new(HtmlWriter::create(&out).unwrap()),
    ];
    for w in ws.iter_mut() {
        w.personal(
            &PersonalInfo {
                user_id: 1,
                first_name: "Ann".into(),
                last_name: "Lee".into(),
                phone: "+100".into(),
                username: "ann".into(),
                bio: "hi".into(),
            },
            &[],
        )
        .unwrap();
        w.contacts(&[Contact {
            user_id: Some(2),
            first_name: "Bob".into(),
            last_name: "".into(),
            phone: "+200".into(),
            date: 1_700_000_000,
        }])
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
    assert!(out.join("chats/chat_1/messages.html").exists());
}
