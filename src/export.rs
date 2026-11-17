//! Runs the export: personal info, profile pictures, contacts, then chats.
//! Everything is fetched through the takeout session and passed on to the writers.

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::path::Path;

use ferogram::{Client, tl};
use tl::enums as E;

use crate::api::{self, Api, Download, TakeoutFlags};
use crate::model::*;
use crate::output::{JsonWriter, Writer};
use crate::parse::{self, MediaCtx};
use crate::settings::Settings;

type BoxErr = Box<dyn Error + Send + Sync>;

struct Downloader {
    out: std::path::PathBuf,
    used: HashSet<String>,
    cache: HashMap<(bool, i64), String>,
    downloaded: u64,
    files: u64,
}

impl Downloader {
    fn reserve(&mut self, suggested: &str) -> String {
        let mut candidate = suggested.to_string();
        let mut n = 0;
        while self.used.contains(&candidate) || self.out.join(&candidate).exists() {
            n += 1;
            let p = Path::new(suggested);
            let stem = p.file_stem().and_then(|x| x.to_str()).unwrap_or("file");
            let ext = p.extension().and_then(|x| x.to_str());
            let dir = p.parent().and_then(|x| x.to_str()).unwrap_or("");
            let name = match ext {
                Some(e) => format!("{stem} ({n}).{e}"),
                None => format!("{stem} ({n})"),
            };
            candidate = if dir.is_empty() {
                name
            } else {
                format!("{dir}/{name}")
            };
        }
        self.used.insert(candidate.clone());
        candidate
    }

    async fn process(
        &mut self,
        api: &Api,
        st: &Settings,
        file: &mut FileRef,
    ) -> Result<(), BoxErr> {
        if !st.apply_policy(file) {
            return Ok(());
        }
        let key = match &file.source {
            Source::Photo(p) => Some((true, p.id)),
            Source::Document(d) => Some((false, d.id)),
            _ => None,
        };
        if let Some(k) = key {
            if let Some(path) = self.cache.get(&k) {
                file.relative_path = path.clone();
                return Ok(());
            }
        }
        let rel = self.reserve(&file.suggested);
        match api.download(file, &self.out.join(&rel)).await? {
            Download::Saved(bytes) => {
                self.downloaded += bytes;
                self.files += 1;
                file.relative_path = rel.clone();
                if let Some(k) = key {
                    self.cache.insert(k, rel);
                }
            }
            Download::Unavailable => {
                self.used.remove(&rel);
                file.skip = Some(SkipReason::Unavailable);
            }
        }
        Ok(())
    }
}

fn files_of(m: &mut Message) -> Vec<&mut FileRef> {
    let mut v = Vec::new();
    match &mut m.media {
        Media::Photo { file, .. } | Media::Document { file, .. } => v.push(file),
        Media::Contact { vcard: Some(f), .. } => v.push(f),
        _ => {}
    }
    if let Some(Action::ChatEditPhoto { photo, .. }) = &mut m.action {
        v.push(photo);
    }
    v
}

fn make_writers(st: &Settings) -> Result<Vec<Box<dyn Writer>>, BoxErr> {
    let mut w: Vec<Box<dyn Writer>> = Vec::new();
    if st.format.json() {
        w.push(Box::new(JsonWriter::create(&st.path)?));
    }
    if st.format.html() {
        w.push(Box::new(crate::html_out::HtmlWriter::create(&st.path)?));
    }
    Ok(w)
}

pub async fn run(client: Client, st: Settings) -> Result<(), BoxErr> {
    std::fs::create_dir_all(&st.path)?;
    // Takeout only allows what it was opened for. A single chat can be of any
    // type, so in that case every message permission is requested.
    let single = st.only_peer.is_some();
    let files = st.wants_files() || st.userpics;
    let flags = TakeoutFlags {
        contacts: st.contacts,
        message_users: single || st.personal_chats || st.bot_chats || st.saved_messages,
        message_chats: single || st.private_groups,
        message_megagroups: single || st.private_groups || st.public_groups || st.left_channels,
        message_channels: single || st.private_channels || st.public_channels || st.left_channels,
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
    let mut dl = Downloader {
        out: st.path.clone(),
        used: HashSet::new(),
        cache: HashMap::new(),
        downloaded: 0,
        files: 0,
    };

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
    if st.personal_info || st.userpics {
        eprintln!("Personal information...");
        let (info, pics) = personal(api, st, dl).await?;
        for w in writers.iter_mut() {
            w.personal(&info, &pics)?;
        }
    }
    if st.contacts {
        eprintln!("Contacts...");
        let list = contacts(api).await?;
        for w in writers.iter_mut() {
            w.contacts(&list)?;
        }
    }
    chats(api, st, writers, dl).await
}

async fn personal(
    api: &mut Api,
    st: &Settings,
    dl: &mut Downloader,
) -> Result<(PersonalInfo, Vec<Userpic>), BoxErr> {
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
    let info = PersonalInfo {
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
    };

    let mut pics = Vec::new();
    if st.userpics {
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
                    dl.process(api, st, &mut file).await?;
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
    }
    Ok((info, pics))
}

async fn contacts(api: &mut Api) -> Result<Vec<Contact>, BoxErr> {
    let mut out: Vec<Contact> = Vec::new();
    let mut seen_phones: HashSet<String> = HashSet::new();
    match api.main(tl::functions::contacts::GetSaved {}).await {
        Ok(list) => {
            for c in list {
                let E::SavedContact::SavedPhoneContact(c) = c;
                seen_phones.insert(c.phone.trim_start_matches('+').to_string());
                out.push(Contact {
                    user_id: None,
                    first_name: c.first_name,
                    last_name: c.last_name,
                    phone: if c.phone.starts_with('+') {
                        c.phone
                    } else {
                        format!("+{}", c.phone)
                    },
                    date: c.date,
                });
            }
        }
        Err(e) => eprintln!("Could not load saved contacts: {e}"),
    }
    if let E::contacts::Contacts::Contacts(c) = api
        .main(tl::functions::contacts::GetContacts { hash: 0 })
        .await?
    {
        api.peers.absorb_users(&c.users);
        for contact in c.contacts {
            let E::Contact::Contact(contact) = contact;
            if let Some(p) = api.peers.get(PeerId::User(contact.user_id)) {
                let phone = p.phone.trim_start_matches('+').to_string();
                if !phone.is_empty() && seen_phones.contains(&phone) {
                    continue;
                }
                out.push(Contact {
                    user_id: Some(contact.user_id),
                    first_name: p.first_name.clone(),
                    last_name: p.last_name.clone(),
                    phone: if phone.is_empty() {
                        String::new()
                    } else {
                        format!("+{phone}")
                    },
                    date: 0,
                });
            }
        }
    }
    Ok(out)
}

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
    let want_any = st.personal_chats
        || st.bot_chats
        || st.private_groups
        || st.public_groups
        || st.private_channels
        || st.public_channels
        || st.saved_messages
        || st.only_peer.is_some();
    if !want_any {
        return Ok(());
    }
    eprintln!("Loading chat list...");
    let only_last = !st.any_group_chats() && st.only_peer.is_none();
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
        .filter(|d| match &st.only_peer {
            Some(q) => peer_matches(api, d, q),
            None => st.wants_dialog(d.dtype),
        })
        .collect();
    for d in &mut list {
        d.only_my_messages = !st.full_for(d.dtype) && st.only_peer.is_none();
    }

    let mut left: Vec<DialogInfo> = Vec::new();
    if st.left_channels && st.only_peer.is_none() && st.any_channels() {
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
            export_dialog(api, st, writers, dl, d, sources).await?;
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
        let splits = if splits.is_empty() {
            vec![api.splits.len() - 1]
        } else {
            splits
        };
        for split in splits {
            let mut offset_id = 1;
            loop {
                let page = match api.history_page(peer, split, offset_id, only_mine).await {
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
                    for f in files_of(&mut m) {
                        dl.process(api, st, f).await?;
                    }
                    batch.push(m);
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
