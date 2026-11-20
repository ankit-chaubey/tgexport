//! tgexport exports a Telegram account to JSON and HTML, like the export in
//! Telegram Desktop does.

mod about;
mod account;
mod api;
mod assets;
mod download;
mod export;
mod grouped_layout;
mod html_out;
mod json_out;
mod media_html;
mod model;
mod output;
mod parse;
mod refresh;
mod rich;
mod rich_html;
mod rich_json;
#[cfg(test)]
mod selftest;
mod settings;
mod thumbs;

use std::collections::HashSet;
use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use ferogram::Client;

use settings::{Format, MediaClassKey, Settings};

#[derive(Clone, Copy, ValueEnum, Debug)]
enum FormatArg {
    Json,
    Html,
    Both,
}

#[derive(Parser, Debug)]
#[command(
    name = "tgexport",
    version,
    about = "Export your Telegram account to JSON and HTML"
)]
struct Cli {
    /// api_id from https://my.telegram.org
    #[arg(long, env = "TG_API_ID")]
    api_id: i32,
    /// api_hash from https://my.telegram.org
    #[arg(long, env = "TG_API_HASH")]
    api_hash: String,
    /// Session file, created on first login
    #[arg(long, default_value = "tgexport.session")]
    session: PathBuf,
    /// Output directory
    #[arg(short, long, default_value = "TelegramExport")]
    out: PathBuf,
    #[arg(short, long, value_enum, default_value = "both")]
    format: FormatArg,

    /// Export everything
    #[arg(long)]
    all: bool,

    /// Your name, phone number, username and bio
    #[arg(long)]
    personal_info: bool,
    /// Your profile pictures
    #[arg(long)]
    userpics: bool,
    /// Your story archive
    #[arg(long)]
    stories: bool,
    /// Music saved on your profile
    #[arg(long)]
    profile_music: bool,
    /// Contacts and frequent contacts
    #[arg(long)]
    contacts: bool,
    /// Active sessions and websites you logged in on
    #[arg(long)]
    sessions: bool,
    /// Other data that Telegram keeps for the account
    #[arg(long)]
    other_data: bool,

    /// Private chats, including Saved Messages
    #[arg(long)]
    personal_chats: bool,
    #[arg(long)]
    bot_chats: bool,
    #[arg(long)]
    private_groups: bool,
    #[arg(long)]
    public_groups: bool,
    #[arg(long)]
    private_channels: bool,
    #[arg(long)]
    public_channels: bool,

    /// Full history of private groups (by default only your own messages)
    #[arg(long)]
    full_private_groups: bool,
    /// Full history of private channels (by default only your own messages)
    #[arg(long)]
    full_private_channels: bool,
    /// Full history of private groups and channels
    #[arg(long)]
    full: bool,

    /// Media to download: any of photos,videos,voice,round,stickers,gifs,files, or `all` / `none`
    #[arg(long, default_value = "photos")]
    media: String,
    /// Skip files larger than this many MB
    #[arg(long, default_value_t = 8)]
    size_limit: i64,
    /// Only messages from this date (YYYY-MM-DD, local time)
    #[arg(long)]
    from: Option<String>,
    /// Only messages up to and including this date (YYYY-MM-DD, local time)
    #[arg(long)]
    till: Option<String>,
    /// Reuse files that are already in the output folder instead of downloading them again
    #[arg(long)]
    resume: bool,
    /// Export a single chat: @username, numeric id, or `me` for Saved Messages
    #[arg(long)]
    chat: Option<String>,
    /// With --chat: export only the forum topic that starts with this message id
    #[arg(long, requires = "chat")]
    topic: Option<i32>,
}

fn parse_date(s: &str, end_of_day: bool) -> Result<i32, String> {
    use chrono::{Local, NaiveDate, TimeZone};
    let d = NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|e| format!("bad date '{s}': {e}"))?;
    let t = if end_of_day {
        d.and_hms_opt(23, 59, 59)
    } else {
        d.and_hms_opt(0, 0, 0)
    }
    .unwrap();
    Local
        .from_local_datetime(&t)
        .single()
        .map(|x| x.timestamp() as i32)
        .ok_or_else(|| format!("ambiguous local time for '{s}'"))
}

fn parse_media(s: &str) -> Result<HashSet<MediaClassKey>, String> {
    let mut set = HashSet::new();
    for part in s
        .split(',')
        .map(|p| p.trim().to_lowercase())
        .filter(|p| !p.is_empty())
    {
        match part.as_str() {
            "none" => return Ok(HashSet::new()),
            "all" => set.extend([
                MediaClassKey::Photo,
                MediaClassKey::Video,
                MediaClassKey::VoiceMessage,
                MediaClassKey::VideoMessage,
                MediaClassKey::Sticker,
                MediaClassKey::Gif,
                MediaClassKey::File,
            ]),
            "photos" => {
                set.insert(MediaClassKey::Photo);
            }
            "videos" => {
                set.insert(MediaClassKey::Video);
            }
            "voice" => {
                set.insert(MediaClassKey::VoiceMessage);
            }
            "round" => {
                set.insert(MediaClassKey::VideoMessage);
            }
            "stickers" => {
                set.insert(MediaClassKey::Sticker);
            }
            "gifs" => {
                set.insert(MediaClassKey::Gif);
            }
            "files" => {
                set.insert(MediaClassKey::File);
            }
            other => return Err(format!("unknown media type '{other}'")),
        }
    }
    Ok(set)
}

fn build_settings(c: &Cli) -> Result<Settings, String> {
    let any_selected = c.personal_info
        || c.userpics
        || c.stories
        || c.profile_music
        || c.contacts
        || c.sessions
        || c.other_data
        || c.personal_chats
        || c.bot_chats
        || c.private_groups
        || c.public_groups
        || c.private_channels
        || c.public_channels
        || c.chat.is_some();
    // With no selection flags, use the same defaults Telegram Desktop starts with.
    let dflt = !any_selected && !c.all;
    let all = c.all;
    // A single chat can be of any type, so nothing is filtered by type then.
    let single = c.chat.is_some();

    let from = c
        .from
        .as_deref()
        .map(|d| parse_date(d, false))
        .transpose()?;
    let till = c.till.as_deref().map(|d| parse_date(d, true)).transpose()?;
    if let (Some(f), Some(t)) = (from, till) {
        if t <= f {
            return Err("--till must be after --from".into());
        }
    }
    if !(0..=4000).contains(&c.size_limit) {
        return Err("--size-limit must be between 0 and 4000 (MB)".into());
    }

    Ok(Settings {
        path: c.out.clone(),
        format: match c.format {
            FormatArg::Json => Format::Json,
            FormatArg::Html => Format::Html,
            FormatArg::Both => Format::Both,
        },
        personal_info: all || dflt || c.personal_info,
        userpics: all || dflt || c.userpics,
        stories: all || dflt || c.stories,
        profile_music: all || dflt || c.profile_music,
        contacts: all || dflt || c.contacts,
        sessions: all || c.sessions,
        other_data: all || c.other_data,
        personal_chats: all || dflt || c.personal_chats || single,
        bot_chats: all || c.bot_chats || single,
        private_groups: all || dflt || c.private_groups || single,
        public_groups: all || c.public_groups || single,
        private_channels: all || c.private_channels || single,
        public_channels: all || c.public_channels || single,
        full_private_groups: c.full || c.full_private_groups || single,
        full_private_channels: c.full || c.full_private_channels || single,
        resume: c.resume,
        media: parse_media(&c.media)?,
        size_limit: c.size_limit * 1024 * 1024,
        from: from.unwrap_or(0),
        till: till.unwrap_or(0),
        only_peer: c.chat.clone(),
        topic: c.topic,
    })
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() {
    let cli = Cli::parse();
    let settings = match build_settings(&cli) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(2);
        }
    };

    eprintln!("Connecting...");
    let (client, _shutdown): (Client, _) = match Client::builder()
        .api_id(cli.api_id)
        .api_hash(cli.api_hash.as_str())
        .session(&cli.session)
        .device_model("Desktop")
        .system_version("Linux")
        .app_version(env!("CARGO_PKG_VERSION"))
        .connect_and_login()
        .await
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: could not connect: {e}");
            std::process::exit(1);
        }
    };

    if let Err(e) = export::run(client, settings).await {
        eprintln!("\nerror: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }
}
