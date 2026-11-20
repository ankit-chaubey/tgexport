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

### On Termux

Building directly on the device works too:

```
pkg install rust
cargo build --release
```

### Cross-compiling for Android

To build a Termux compatible binary (aarch64, bionic) on a PC you need the
Android NDK and [cargo-ndk](https://github.com/bbqsrc/cargo-ndk):

```
rustup target add aarch64-linux-android
cargo install cargo-ndk
export ANDROID_NDK_HOME=/path/to/android-ndk
cargo ndk --platform 24 -t arm64-v8a build --release
```

The result is `target/aarch64-linux-android/release/tgexport`.

If dates show up in UTC on Termux, set your zone, for example
`TZ=Asia/Kolkata tgexport`.

## Usage

Get an `api_id` and `api_hash` from https://my.telegram.org and pass them as
options:

```
tgexport --api-id 123456 --api-hash 0123456789abcdef
tgexport --api-id 123456 --api-hash 0123456789abcdef --all --full --media all --size-limit 2000
tgexport --api-id 123456 --api-hash 0123456789abcdef --chat @username --format json
tgexport --api-id 123456 --api-hash 0123456789abcdef --chat me
```

To avoid typing the credentials every time you can also set them once as the
`TG_API_ID` and `TG_API_HASH` environment variables.

The first run asks for your phone number, the login code and the 2FA password
if you have one. The session is stored in `tgexport.session` and reused after
that. Telegram may ask you to confirm the export request in another Telegram
app. The tool waits and retries until you do.

## Options

The options follow the checkboxes of the export dialog in Telegram Desktop.

| Option | What it exports |
| --- | --- |
| `--personal-info` | name, phone number, username, bio |
| `--userpics` | your profile pictures |
| `--stories` | your story archive |
| `--profile-music` | music saved on your profile |
| `--contacts` | contacts and frequent contacts |
| `--sessions` | active sessions and web sessions |
| `--other-data` | the other data file that Telegram keeps for the account |
| `--personal-chats` | private chats, including Saved Messages |
| `--bot-chats` | chats with bots |
| `--private-groups`, `--public-groups` | groups |
| `--private-channels`, `--public-channels` | channels |
| `--all` | everything above |

With none of these given, the same selection Telegram Desktop starts with is
used: personal info, profile pictures, stories, profile music, contacts,
personal chats and private groups. Groups that you left are exported
automatically when any group or channel type is selected.

Message history:

- Private chats and bots are always exported in full.
- Private groups and private channels only contain your own messages, as in
  Telegram Desktop, unless you pass `--full-private-groups`,
  `--full-private-channels` or `--full` for both.
- Public groups and public channels always contain only your own messages.

Media (like in Telegram Desktop only photos are downloaded by default):

- `--media photos,videos,voice,round,stickers,gifs,files`, or `all` / `none`
- `--size-limit MB`, 8 by default, at most 4000

Other options:

- `--format json|html|both`, `both` by default
- `--out DIR`, `--session FILE`
- `--resume` keeps files that are already complete in the output folder, so an
  interrupted export does not download everything again. The JSON and HTML are
  written from scratch each time.
- `--chat @username|id|me` exports a single chat (as a flat folder, without the
  index pages), `--topic ID` narrows that to one forum topic
- `--from YYYY-MM-DD` and `--till YYYY-MM-DD` limit the messages by date

Run `tgexport --help` for the full list.

## What is exported

- text with formatting, photos, videos, voice and round messages, stickers,
  GIFs, files with their thumbnails, contacts, locations, polls, buttons,
  reactions (with who reacted, where Telegram tells) and service messages
- custom emoji used in messages and reactions, saved as files (they follow the
  sticker filter and the size limit)
- every section of the table above, in `result.json` and as HTML pages
- expired file references are renewed automatically while downloading

## Not done yet

- rich messages (they are exported without their rich content)
