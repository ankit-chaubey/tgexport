# tgexport

Exports a Telegram account to JSON and HTML, the same way the export in
Telegram Desktop does, but as a command line tool built on
[ferogram](https://crates.io/crates/ferogram).

Data is fetched through a takeout session, so files and message history are
requested the same way Telegram Desktop requests them. The HTML pages reuse the
stylesheet, script and images from Telegram Desktop, and `result.json` follows
the same layout.

## Build

```
cargo build --release
```

The binary ends up in `target/release/tgexport`.

## Usage

Get an `api_id` and `api_hash` from https://my.telegram.org, then:

```
export TG_API_ID=123456
export TG_API_HASH=0123456789abcdef

tgexport                                   # default selection, JSON and HTML
tgexport --all --full --media all --size-limit 2000
tgexport --chat @username --format json    # a single chat
tgexport --chat me                         # Saved Messages
```

The first run asks for your phone number, the login code and the 2FA password
if you have one. The session is stored in `tgexport.session` and reused after
that. Telegram may ask you to confirm the export request in another Telegram
app. The tool waits and retries until you do.

Run `tgexport --help` for all options.

## What is exported

- personal information, profile pictures and contacts
- private chats, bots, groups, channels, Saved Messages and left channels
- text with formatting, photos, videos, voice and round messages, stickers,
  GIFs, files, contacts, locations, polls and service messages
- groups and channels only contain your own messages unless you pass `--full`

## Not done yet

- sessions, web sessions, frequent contacts and stories
- video thumbnails
- refreshing expired file references (such files are marked as unavailable)
- resuming an interrupted export
