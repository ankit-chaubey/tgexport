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

//! Saving files into the export folder: naming, reuse of files that are
//! already there, thumbnails, fresh references and custom emoji.

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::path::{Path, PathBuf};

use crate::api::{Api, Download};
use crate::model::*;
use crate::parse::{self, MediaCtx};
use crate::refresh::rebind;
use crate::rich::{self, RichMessage, RichTextKind, Visit};
use crate::settings::Settings;

type BoxErr = Box<dyn Error + Send + Sync>;

/// What a custom emoji file turned out to be.
const EMOJI_UNAVAILABLE: &str = "(unavailable)";

/// How often a download is retried with a fresh file reference.
const REFRESH_ATTEMPTS: u32 = 2;

pub struct Downloader {
    out: PathBuf,
    resume: bool,
    used: HashSet<String>,
    /// Same photo or document seen again: reuse the file already saved.
    cache: HashMap<(u8, i64), String>,
    /// Document behind a custom emoji id, found with `GetCustomEmojiDocuments`.
    emoji_files: HashMap<i64, FileRef>,
    /// Final value for a custom emoji: path, empty (not saved) or "(unavailable)".
    emoji_results: HashMap<i64, String>,
    pub downloaded: u64,
    pub files: u64,
}

impl Downloader {
    pub fn new(out: PathBuf, resume: bool) -> Self {
        Downloader {
            out,
            resume,
            used: HashSet::new(),
            cache: HashMap::new(),
            emoji_files: HashMap::new(),
            emoji_results: HashMap::new(),
            downloaded: 0,
            files: 0,
        }
    }

    /// Picks the path for a file. Returns it together with a flag telling that
    /// a complete copy is already on disk and can be kept (resume mode only).
    fn reserve(&mut self, suggested: &str, size: i64) -> (String, bool) {
        let mut candidate = suggested.to_string();
        let mut n = 0;
        loop {
            if !self.used.contains(&candidate) {
                let on_disk = self.out.join(&candidate).metadata().ok();
                if self.resume {
                    let complete =
                        size > 0 && on_disk.map(|m| m.len() as i64 == size).unwrap_or(false);
                    self.used.insert(candidate.clone());
                    return (candidate, complete);
                }
                if on_disk.is_none() {
                    self.used.insert(candidate.clone());
                    return (candidate, false);
                }
            }
            n += 1;
            candidate = numbered(suggested, n);
        }
    }

    /// Downloads `file` unless the filters say otherwise, and records the result
    /// on it (path or skip reason).
    pub async fn process(
        &mut self,
        api: &mut Api,
        st: &Settings,
        file: &mut FileRef,
        origin: &Origin,
    ) -> Result<(), BoxErr> {
        if !st.apply_policy(file) {
            return Ok(());
        }
        let key = cache_key(&file.source);
        if let Some(k) = key {
            if let Some(path) = self.cache.get(&k) {
                file.relative_path = path.clone();
                return Ok(());
            }
        }
        let (rel, complete) = self.reserve(&file.suggested, file.size);
        if complete {
            file.relative_path = rel.clone();
            self.remember(key, rel);
            return Ok(());
        }

        let dest = self.out.join(&rel);
        let mut attempts = 0;
        loop {
            match api.download(file, &dest).await? {
                Download::Saved(bytes) => {
                    self.downloaded += bytes;
                    self.files += 1;
                    file.relative_path = rel.clone();
                    self.remember(key, rel);
                    return Ok(());
                }
                Download::Unavailable => break,
                Download::Expired => {
                    attempts += 1;
                    if attempts > REFRESH_ATTEMPTS {
                        break;
                    }
                    let Ok(fresh) = api.refresh(origin).await else {
                        break;
                    };
                    if !rebind(&mut file.source, &fresh) {
                        break;
                    }
                }
            }
        }
        self.used.remove(&rel);
        file.skip = Some(SkipReason::Unavailable);
        Ok(())
    }

    fn remember(&mut self, key: Option<(u8, i64)>, path: String) {
        if let Some(k) = key {
            self.cache.insert(k, path);
        }
    }

    /// A document and its thumbnail. The thumbnail follows the filters that
    /// applied to the file.
    pub async fn file_with_thumb(
        &mut self,
        api: &mut Api,
        st: &Settings,
        file: &mut FileRef,
        thumb: &mut Option<Thumb>,
        origin: &Origin,
    ) -> Result<(), BoxErr> {
        self.process(api, st, file, origin).await?;
        if let Some(t) = thumb {
            match file.skip {
                // A thumbnail of a file we skip would only waste time.
                Some(reason @ (SkipReason::FileType | SkipReason::FileSize)) => {
                    t.file.skip = Some(reason);
                }
                _ => self.process(api, st, &mut t.file, origin).await?,
            }
        }
        Ok(())
    }

    /// Files used by the blocks of a rich message, in the order they appear.
    async fn rich_files(
        &mut self,
        api: &mut Api,
        st: &Settings,
        rich: &mut RichMessage,
        origin: &Origin,
    ) -> Result<(), BoxErr> {
        let mut order: Vec<(bool, u64)> = Vec::new();
        rich::visit(rich, &mut |v| match v {
            Visit::Photo(id) => order.push((false, id)),
            Visit::Document(id) => order.push((true, id)),
            Visit::Text(_) => {}
        });
        let mut seen: HashSet<(bool, u64)> = HashSet::new();
        for (is_document, id) in order {
            if !seen.insert((is_document, id)) {
                continue;
            }
            if is_document {
                if let Some(d) = rich.documents.get_mut(&id) {
                    self.file_with_thumb(api, st, &mut d.file, &mut d.thumb, origin)
                        .await?;
                }
            } else if let Some(p) = rich.photos.get_mut(&id) {
                self.process(api, st, &mut p.file, origin).await?;
            }
        }
        Ok(())
    }

    /// Handles every file of a message: the media itself, and for documents the
    /// thumbnail, which follows the same filters as the file.
    pub async fn message_files(
        &mut self,
        api: &mut Api,
        st: &Settings,
        m: &mut Message,
        origin: &Origin,
    ) -> Result<(), BoxErr> {
        match &mut m.media {
            Media::Photo { file, .. } => self.process(api, st, file, origin).await?,
            Media::Document { file, thumb, .. } => {
                self.file_with_thumb(api, st, file, thumb, origin).await?
            }
            Media::Contact { vcard: Some(f), .. } => self.process(api, st, f, origin).await?,
            _ => {}
        }
        if let Some(Action::ChatEditPhoto { photo, .. }) = &mut m.action {
            self.process(api, st, photo, origin).await?;
        }
        if let Some(rich) = &mut m.rich {
            let rich_origin = match origin {
                Origin::Message { peer, id, .. } => Origin::RichMessage {
                    peer: *peer,
                    id: *id,
                },
                other => other.clone(),
            };
            self.rich_files(api, st, rich, &rich_origin).await?;
        }
        Ok(())
    }

    /// Replaces custom emoji ids in the messages of one page with the path of
    /// the saved emoji file. Anything that cannot be saved is marked as such.
    pub async fn resolve_emoji(
        &mut self,
        api: &mut Api,
        st: &Settings,
        ctx: &mut MediaCtx,
        batch: &mut [Message],
    ) -> Result<(), BoxErr> {
        let mut wanted: Vec<i64> = Vec::new();
        for m in batch.iter() {
            for id in emoji_ids(m) {
                if !self.emoji_results.contains_key(&id) && !wanted.contains(&id) {
                    wanted.push(id);
                }
            }
        }
        if wanted.is_empty() {
            return Ok(());
        }

        for chunk in wanted.chunks(100) {
            // A failed lookup leaves these emoji unavailable, nothing more.
            if let Ok(docs) = api.custom_emoji_docs(chunk).await {
                for doc in &docs {
                    let ferogram::tl::enums::Document::Document(d) = doc else {
                        continue;
                    };
                    if let Some(info) = parse::parse_document(ctx, doc, 0) {
                        self.emoji_files.insert(d.id, info.file);
                    }
                }
            }
        }

        for id in wanted {
            let result = match self.emoji_files.get(&id).cloned() {
                None => EMOJI_UNAVAILABLE.to_string(),
                Some(mut file) => {
                    self.process(api, st, &mut file, &Origin::CustomEmoji { id })
                        .await?;
                    if !file.relative_path.is_empty() {
                        file.relative_path
                    } else {
                        match file.skip {
                            Some(SkipReason::FileType | SkipReason::FileSize) => String::new(),
                            _ => EMOJI_UNAVAILABLE.to_string(),
                        }
                    }
                }
            };
            self.emoji_results.insert(id, result);
        }

        for m in batch.iter_mut() {
            for part in &mut m.text {
                if part.kind == TextKind::CustomEmoji {
                    if let Some(r) = part
                        .additional
                        .parse::<i64>()
                        .ok()
                        .and_then(|i| self.emoji_results.get(&i))
                    {
                        part.additional = r.clone();
                    }
                }
            }
            if let Some(rich) = &mut m.rich {
                let results = &self.emoji_results;
                rich::texts_mut(rich, &mut |t| {
                    if t.kind == RichTextKind::CustomEmoji && t.id != 0 {
                        if let Some(r) = results.get(&(t.id as i64)) {
                            t.custom_emoji_data = r.clone();
                        }
                    }
                });
            }
            for r in &mut m.reactions {
                if let Some(custom) = &mut r.custom {
                    if let Some(res) = custom
                        .parse::<i64>()
                        .ok()
                        .and_then(|i| self.emoji_results.get(&i))
                    {
                        *custom = res.clone();
                    }
                }
            }
        }
        Ok(())
    }
}

fn emoji_ids(m: &Message) -> Vec<i64> {
    let mut ids = Vec::new();
    for part in &m.text {
        if part.kind == TextKind::CustomEmoji {
            if let Ok(id) = part.additional.parse::<i64>() {
                ids.push(id);
            }
        }
    }
    for r in &m.reactions {
        if let Some(id) = r.custom.as_ref().and_then(|c| c.parse::<i64>().ok()) {
            ids.push(id);
        }
    }
    if let Some(rich) = &m.rich {
        rich::visit(rich, &mut |v| {
            if let Visit::Text(t) = v {
                if t.kind == RichTextKind::CustomEmoji && t.id != 0 {
                    ids.push(t.id as i64);
                }
            }
        });
    }
    ids
}

fn cache_key(source: &Source) -> Option<(u8, i64)> {
    match source {
        Source::Photo(p) => Some((0, p.id)),
        Source::Document(d) => Some((1, d.id)),
        Source::DocumentThumb(d, _) => Some((2, d.id)),
        _ => None,
    }
}

/// `name (1).ext`, `name (2).ext`, ... next to the original.
fn numbered(suggested: &str, n: u32) -> String {
    let p = Path::new(suggested);
    let stem = p.file_stem().and_then(|x| x.to_str()).unwrap_or("file");
    let ext = p.extension().and_then(|x| x.to_str());
    let dir = p.parent().and_then(|x| x.to_str()).unwrap_or("");
    let name = match ext {
        Some(e) => format!("{stem} ({n}).{e}"),
        None => format!("{stem} ({n})"),
    };
    if dir.is_empty() {
        name
    } else {
        format!("{dir}/{name}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("tgexport_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn existing_file_gets_a_new_name_without_resume() {
        let dir = temp("plain");
        std::fs::write(dir.join("a.jpg"), b"12345").unwrap();
        let mut d = Downloader::new(dir, false);
        assert_eq!(d.reserve("a.jpg", 5), ("a (1).jpg".to_string(), false));
    }

    #[test]
    fn resume_keeps_complete_files_only() {
        let dir = temp("resume");
        std::fs::write(dir.join("a.jpg"), b"12345").unwrap();
        std::fs::write(dir.join("b.jpg"), b"12").unwrap();
        let mut d = Downloader::new(dir, true);
        assert_eq!(d.reserve("a.jpg", 5), ("a.jpg".to_string(), true));
        // Truncated by an interrupted run: download it again under the same name.
        assert_eq!(d.reserve("b.jpg", 5), ("b.jpg".to_string(), false));
        // The same name twice in one run still gets a new name.
        assert_eq!(d.reserve("a.jpg", 5), ("a (1).jpg".to_string(), false));
    }
}
