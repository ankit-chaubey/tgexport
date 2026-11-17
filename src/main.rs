//! tgexport exports a Telegram account to JSON and HTML, like the export in
//! Telegram Desktop does.

mod about;
mod api;
mod assets;
mod export;
mod html_out;
mod json_out;
mod model;
mod output;
mod parse;
#[cfg(test)]
mod selftest;
mod settings;

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

    /// Export everything, including left channels
    #[arg(long)]
    all: bool,
    #[arg(long)]
    personal_info: bool,
    #[arg(long)]
    userpics: bool,
    #[arg(long)]
    contacts: bool,
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
    #[arg(long)]
    saved: bool,
    #[arg(long)]
    left_channels: bool,
    /// Full history of groups (by default only your own messages are exported)
    #[arg(long)]
    full_groups: bool,
    /// Full history of channels (by default only your own messages are exported)
    #[arg(long)]
    full_channels: bool,
    /// Full history of all groups and channels
    #[arg(long)]
    full: bool,

    /// Media to download: any of photos,videos,voice,round,stickers,gifs,files, or `all` / `none`
    #[arg(long, default_value = "photos,videos,voice,round,files")]
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
    /// Export a single chat: @username, numeric id, or `me` for Saved Messages
    #[arg(long)]
    chat: Option<String>,
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
            "all" => {
                set.extend([
                    MediaClassKey::Photo,
                    MediaClassKey::Video,
                    MediaClassKey::VoiceMessage,
                    MediaClassKey::VideoMessage,
                    MediaClassKey::Sticker,
                    MediaClassKey::Gif,
                    MediaClassKey::File,
                ]);
            }
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
        || c.contacts
        || c.personal_chats
        || c.bot_chats
        || c.private_groups
        || c.public_groups
        || c.private_channels
        || c.public_channels
        || c.saved
        || c.left_channels
        || c.chat.is_some();
    let all = c.all;
    // With no selection flags, use the same defaults Telegram Desktop starts with.
    let dflt = !any_selected && !all;
    Ok(Settings {
        path: c.out.clone(),
        format: match c.format {
            FormatArg::Json => Format::Json,
            FormatArg::Html => Format::Html,
            FormatArg::Both => Format::Both,
        },
        personal_info: all || dflt || c.personal_info,
        userpics: all || dflt || c.userpics,
        contacts: all || dflt || c.contacts,
        personal_chats: all || dflt || c.personal_chats,
        bot_chats: all || c.bot_chats,
        private_groups: all || dflt || c.private_groups,
        public_groups: all || c.public_groups,
        private_channels: all || c.private_channels,
        public_channels: all || c.public_channels,
        saved_messages: all || dflt || c.saved,
        left_channels: all || c.left_channels,
        full_groups: c.full || c.full_groups,
        full_channels: c.full || c.full_channels,
        media: parse_media(&c.media)?,
        size_limit: c.size_limit.max(0) * 1024 * 1024,
        from: c
            .from
            .as_deref()
            .map(|d| parse_date(d, false))
            .transpose()?
            .unwrap_or(0),
        till: c
            .till
            .as_deref()
            .map(|d| parse_date(d, true))
            .transpose()?
            .unwrap_or(0),
        only_peer: c.chat.clone(),
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
