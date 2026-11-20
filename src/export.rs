//! Runs the export: personal info, profile pictures, contacts, then chats.
//! Everything is fetched through the takeout session and passed on to the writers.

use std::collections::HashMap;
use std::error::Error;

use ferogram::{Client, tl};
use tl::enums as E;

use crate::api::{self, Api, Download, TakeoutFlags};
use crate::download::Downloader;
use crate::model::*;
use crate::output::{JsonWriter, Writer};
use crate::parse::{self, MediaCtx};
use crate::rich;
use crate::settings::Settings;

type BoxErr = Box<dyn Error + Send + Sync>;

fn make_writers(st: &Settings) -> Result<Vec<Box<dyn Writer>>, BoxErr> {
    let single = st.only_peer.is_some();
    let mut w: Vec<Box<dyn Writer>> = Vec::new();
    if st.format.json() {
        w.push(Box::new(JsonWriter::create(&st.path, single)?));
    }
    if st.format.html() {
        w.push(Box::new(crate::html_out::HtmlWriter::create(
            &st.path, single,
        )?));
    }
    Ok(w)
}

pub async fn run(client: Client, st: Settings) -> Result<(), BoxErr> {
    std::fs::create_dir_all(&st.path)?;
    // Takeout only allows what it was opened for, so the flags follow the
    // selected sections. For a single chat every chat type is enabled.
    let files = st.needs_files();
    let flags = TakeoutFlags {
        contacts: st.contacts,
        message_users: st.personal_chats || st.bot_chats,
        message_chats: st.private_groups,
        message_megagroups: st.private_groups || st.public_groups,
        message_channels: st.private_channels || st.public_channels,
        files,
        file_max_size: if files && st.size_limit < api::FILE_MAX_SIZE {
            Some(st.size_limit.max(1))
        } else {
            None
        },
    };
    eprintln!("Starting takeout session...");
    let mut api = Api::start(client, flags).await?;
    let mut writers = make_writers(&st)?;
    let mut dl = Downloader::new(st.path.clone(), st.resume);

    let result = run_stages(&mut api, &st, &mut writers, &mut dl).await;
    let ok = result.is_ok();
    for w in writers {
        if let Err(e) = w.finish() {
            eprintln!("Could not finalize output: {e}");
        }
    }
    api.finish(ok).await;
    if ok {
        eprintln!(
            "Done. {} files, {:.1} MB downloaded. Output: {}",
            dl.files,
            dl.downloaded as f64 / 1_048_576.0,
            st.path.display()
        );
    }
    result
}

async fn run_stages(
    api: &mut Api,
    st: &Settings,
    writers: &mut [Box<dyn Writer>],
    dl: &mut Downloader,
) -> Result<(), BoxErr> {
    if st.personal_info {
        eprintln!("Personal information...");
        let info = personal_info(api).await?;
        for w in writers.iter_mut() {
            w.personal(&info)?;
        }
    }
    if st.userpics {
        eprintln!("Profile pictures...");
        let pics = userpics(api, st, dl).await?;
        for w in writers.iter_mut() {
            w.userpics(&pics)?;
        }
    }
    if st.stories {
        eprintln!("Stories...");
        let list = stories(api, st, dl).await?;
        for w in writers.iter_mut() {
            w.stories(&list)?;
        }
    }
    if st.profile_music {
        eprintln!("Profile music...");
        let list = profile_music(api, st, dl).await?;
        for w in writers.iter_mut() {
            w.profile_music(&list, &api.peers)?;
        }
    }
    if st.contacts {
        eprintln!("Contacts...");
        let mut list = ContactsList {
            contacts: api.saved_contacts().await?,
            ..Default::default()
        };
        api.top_peers(&mut list).await?;
        for w in writers.iter_mut() {
            w.contacts(&list)?;
        }
    }
    if st.sessions {
        eprintln!("Sessions...");
        let list = api.sessions().await?;
        for w in writers.iter_mut() {
            w.sessions(&list)?;
        }
    }
    if st.other_data {
        eprintln!("Other data...");
        let rel = "lists/other_data.json".to_string();
        let mut file = FileRef {
            source: Source::Takeout,
            class: MediaClass::Always,
            size: 0,
            suggested: rel.clone(),
            relative_path: String::new(),
            skip: None,
        };
        match api.download(&file, &st.path.join(&rel)).await? {
            Download::Saved(_) => file.relative_path = rel,
            Download::Unavailable | Download::Expired => file.skip = Some(SkipReason::Unavailable),
        }
        for w in writers.iter_mut() {
            w.other_data(&file)?;
        }
    }
    chats(api, st, writers, dl).await
}

// Account data.

async fn personal_info(api: &mut Api) -> Result<PersonalInfo, BoxErr> {
    let full = api
        .main(tl::functions::users::GetFullUser {
            id: E::InputUser::UserSelf,
        })
        .await?;
    let E::users::UserFull::UserFull(full) = full;
    api.peers.absorb_users(&full.users);
    let E::UserFull::UserFull(fu) = &full.full_user;
    let me = api
        .peers
        .get(PeerId::User(api.self_id))
        .cloned()
        .unwrap_or_default();
    Ok(PersonalInfo {
        user_id: api.self_id,
        first_name: me.first_name,
        last_name: me.last_name,
        phone: if me.phone.is_empty() || me.phone.starts_with('+') {
            me.phone
        } else {
            format!("+{}", me.phone)
        },
        username: me.username,
        bio: fu.about.clone().unwrap_or_default(),
    })
}

async fn userpics(
    api: &mut Api,
    st: &Settings,
    dl: &mut Downloader,
) -> Result<Vec<Userpic>, BoxErr> {
    let mut pics = Vec::new();
    let mut offset = 0;
    let mut counter = 0;
    loop {
        let res = api
            .main(tl::functions::photos::GetUserPhotos {
                user_id: E::InputUser::UserSelf,
                offset,
                max_id: 0,
                limit: api::SLICE_LIMIT,
            })
            .await?;
        let (photos, more) = match res {
            E::photos::Photos::Photos(p) => (p.photos, false),
            E::photos::Photos::Slice(p) => (p.photos, true),
        };
        if photos.is_empty() {
            break;
        }
        offset += photos.len() as i32;
        for p in &photos {
            let E::Photo::Photo(ph) = p else { continue };
            counter += 1;
            let name = format!(
                "profile_pictures/photo_{}{}.jpg",
                counter,
                parse::file_date_part(ph.date)
            );
            if let Some((mut file, _, _)) = parse::photo_file(p, name) {
                file.class = MediaClass::Always;
                dl.process(api, st, &mut file, &Origin::None).await?;
                pics.push(Userpic {
                    date: ph.date,
                    file,
                });
            }
        }
        if !more {
            break;
        }
    }
    Ok(pics)
}

async fn stories(api: &mut Api, st: &Settings, dl: &mut Downloader) -> Result<Vec<Story>, BoxErr> {
    let mut out = Vec::new();
    let mut offset_id = 0;
    let mut index = 0;
    loop {
        let (items, last) = api.stories_page(offset_id).await?;
        if items.is_empty() {
            break;
        }
        for item in &items {
            if let Some(mut story) = parse::parse_story(index + 1, item) {
                index += 1;
                dl.process(api, st, &mut story.file, &Origin::Story { id: story.id })
                    .await?;
                out.push(story);
            }
        }
        if last == 0 {
            break;
        }
        offset_id = last;
    }
    Ok(out)
}

async fn profile_music(
    api: &mut Api,
    st: &Settings,
    dl: &mut Downloader,
) -> Result<Vec<Message>, BoxErr> {
    let mut out = Vec::new();
    let mut offset = 0;
    let mut index = 0;
    loop {
        let docs = api.profile_music_page(offset).await?;
        let got = docs.len() as i32;
        for doc in &docs {
            if let Some(mut m) = parse::parse_music(index + 1, doc) {
                index += 1;
                dl.message_files(api, st, &mut m, &Origin::None).await?;
                out.push(m);
            }
        }
        if got < api::SLICE_LIMIT {
            break;
        }
        offset += got;
    }
    Ok(out)
}

// Chats.

fn pad(n: usize, digits: usize) -> String {
    format!("{n:0digits$}")
}

fn peer_matches(api: &Api, d: &DialogInfo, query: &str) -> bool {
    let q = query.trim().trim_start_matches('@');
    if q.eq_ignore_ascii_case("me") || q.eq_ignore_ascii_case("self") {
        return d.dtype == DialogType::Saved;
    }
    if let Ok(id) = q.parse::<i64>() {
        return d.peer.bare() == id.abs() || d.peer.bare() == id;
    }
    api.peers
        .get(d.peer)
        .map(|p| p.username.eq_ignore_ascii_case(q))
        .unwrap_or(false)
}

async fn chats(
    api: &mut Api,
    st: &Settings,
    writers: &mut [Box<dyn Writer>],
    dl: &mut Downloader,
) -> Result<(), BoxErr> {
    if !st.any_chats() {
        return Ok(());
    }
    eprintln!("Loading chat list...");
    let only_last = !st.non_channel_chats();
    let all = api.dialogs(only_last).await?;

    // A group that was upgraded to a supergroup shares its history with it, so fold it in.
    let mut by_peer: HashMap<PeerId, usize> = HashMap::new();
    let mut list: Vec<DialogInfo> = Vec::new();
    let mut old_chats: Vec<DialogInfo> = Vec::new();
    for d in all {
        let migrated_to = api.peers.get(d.peer).and_then(|p| p.migrated_to);
        if migrated_to.is_some() {
            old_chats.push(d);
        } else {
            by_peer.insert(d.peer, list.len());
            list.push(d);
        }
    }
    let mut extra_sources: HashMap<PeerId, Vec<(PeerId, Vec<usize>)>> = HashMap::new();
    for old in old_chats {
        let to = api
            .peers
            .get(old.peer)
            .and_then(|p| p.migrated_to)
            .map(PeerId::Channel);
        match to.and_then(|t| by_peer.get(&t).copied()) {
            Some(i) => {
                list[i].migrated_from = Some(old.peer);
                extra_sources
                    .entry(list[i].peer)
                    .or_default()
                    .push((old.peer, old.splits.clone()));
            }
            None => {
                by_peer.insert(old.peer, list.len());
                list.push(old);
            }
        }
    }

    let mut list: Vec<DialogInfo> = list
        .into_iter()
        .filter(|d| st.wants_dialog(d.dtype))
        .filter(|d| match &st.only_peer {
            Some(q) => peer_matches(api, d, q),
            None => true,
        })
        .collect();
    for d in &mut list {
        d.only_my_messages = !st.full_for(d.dtype);
    }
    if st.only_peer.is_some() && list.is_empty() {
        return Err("chat not found among your dialogs".into());
    }

    let mut left: Vec<DialogInfo> = Vec::new();
    if st.groups_or_channels() && st.only_peer.is_none() {
        eprintln!("Loading left channels...");
        left = api.left_channels().await?;
        left.retain(|d| !list.iter().any(|x| x.peer == d.peer));
    }

    let full_count = list.len() + left.len();
    let digits = full_count.saturating_sub(1).to_string().len().max(1);
    let single = st.only_peer.is_some();
    let mut index = 0;
    for d in list.iter_mut().chain(left.iter_mut()) {
        index += 1;
        d.relative_path = if single {
            String::new()
        } else {
            format!("chats/chat_{}/", pad(index, digits))
        };
    }
    eprintln!("{} chats to export.", full_count);

    let sets: [(&mut Vec<DialogInfo>, bool); 2] = [(&mut list, false), (&mut left, true)];
    for (dialogs, is_left) in sets {
        if dialogs.is_empty() {
            continue;
        }
        for w in writers.iter_mut() {
            w.dialogs_start(is_left)?;
        }
        let total = dialogs.len();
        for (i, d) in dialogs.iter().enumerate() {
            eprintln!(
                "[{}/{}] {} ({})",
                i + 1,
                total,
                if d.name.is_empty() {
                    "(no name)"
                } else {
                    &d.name
                },
                d.dtype.json_name()
            );
            let mut sources = Vec::new();
            if let Some(ex) = extra_sources.get(&d.peer) {
                sources.extend(ex.iter().cloned());
            }
            sources.push((d.peer, d.splits.clone()));
            let mut dialog = d.clone();
            if let Some(root) = st.topic {
                // A single topic is written like a public supergroup named after it.
                dialog.dtype = DialogType::PublicSupergroup;
                dialog.name = format!("{} (topic {})", d.name, root);
            }
            export_dialog(api, st, writers, dl, &dialog, sources).await?;
        }
        for w in writers.iter_mut() {
            w.dialogs_end()?;
        }
    }
    Ok(())
}

async fn export_dialog(
    api: &mut Api,
    st: &Settings,
    writers: &mut [Box<dyn Writer>],
    dl: &mut Downloader,
    d: &DialogInfo,
    sources: Vec<(PeerId, Vec<usize>)>,
) -> Result<(), BoxErr> {
    for w in writers.iter_mut() {
        w.dialog_start(d, &api.peers)?;
    }
    let mut ctx = MediaCtx {
        folder: d.relative_path.clone(),
        ..Default::default()
    };
    let mut only_mine = d.only_my_messages;
    let mut count = 0u64;

    for (peer, splits) in sources {
        let splits = if splits.is_empty() || st.topic.is_some() {
            vec![api.splits.len() - 1]
        } else {
            splits
        };
        for split in splits {
            let mut offset_id = 1;
            loop {
                let fetched = match st.topic {
                    Some(root) => api.topic_page(peer, root, offset_id).await,
                    None => api.history_page(peer, split, offset_id, only_mine).await,
                };
                let page = match fetched {
                    Ok(p) => p,
                    Err(e)
                        if api::rpc_name(&e) == Some("CHANNEL_PRIVATE")
                            && matches!(peer, PeerId::Channel(_))
                            && !only_mine =>
                    {
                        // We lost access to the channel, so keep only our own messages.
                        only_mine = true;
                        continue;
                    }
                    Err(e) => return Err(e.into()),
                };
                if page.is_empty() {
                    break;
                }
                let mut max_id = 0;
                let mut batch: Vec<Message> = Vec::new();
                for raw in &page {
                    let id = match raw {
                        E::Message::Message(m) => m.id,
                        E::Message::Service(m) => m.id,
                        E::Message::Empty(m) => m.id,
                    };
                    max_id = max_id.max(id);
                    let Some(mut m) = parse::parse_message(&mut ctx, raw) else {
                        continue;
                    };
                    if m.from.is_none() && d.dtype.is_channel() {
                        m.from = Some(peer); // posts in channels have no sender, use the channel
                    }
                    if (st.from != 0 && m.date < st.from) || (st.till != 0 && m.date > st.till) {
                        continue;
                    }
                    batch.push(m);
                }

                // Long rich messages arrive as a part first: ask for the whole thing.
                for m in batch.iter_mut() {
                    if m.rich.as_ref().map(|r| r.part).unwrap_or(false) {
                        let full = api.full_rich_message(peer, m.id).await?;
                        let parsed = full
                            .map(|f| rich::parse_rich_message(&mut ctx, &f, m.date))
                            .filter(|p| !p.part);
                        match parsed {
                            Some(p) => m.rich = Some(p),
                            None => return Err("Unexpected rich message hydration result.".into()),
                        }
                    }
                }

                dl.resolve_emoji(api, st, &mut ctx, &mut batch).await?;
                for m in batch.iter_mut() {
                    let origin = Origin::Message {
                        peer,
                        id: m.id,
                        split,
                    };
                    dl.message_files(api, st, m, &origin).await?;
                }
                count += batch.len() as u64;
                for w in writers.iter_mut() {
                    w.messages(d, &batch, &api.peers)?;
                }
                eprint!("\r    {count} messages");
                offset_id = max_id + 1;
            }
        }
    }
    eprintln!();
    for w in writers.iter_mut() {
        w.dialog_end()?;
    }
    Ok(())
}
