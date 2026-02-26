# Blepo

A CLI tool for watching YouTube subscriptions without ads, distractions, or tracking.

### Name

From the Ancient Greek *blepo* (βλέπω) — "I see, I look." Just watching, nothing else.

Blepo fetches videos via RSS feeds (with yt-dlp as fallback), filters out what you've already watched, and plays them through mpv.

## Requirements

- [mpv](https://mpv.io/) — video player
- [yt-dlp](https://github.com/yt-dlp/yt-dlp) — resolves YouTube URLs into video streams (mpv calls it automatically)
- [Rust](https://rustup.rs/) — to build from source

## Install

Install mpv and yt-dlp first:

```bash
brew install mpv yt-dlp    # macOS
```

Then build and install blepo:

```bash
cargo install --path .
```

## Usage

```bash
blepo    # Fetch videos, show list, pick one to play
```

Running `blepo` fetches the latest videos from your channels, shows the unwatched ones, and prompts you to pick a number. It launches mpv in the background and returns to the shell immediately. Enter `w3` to mark video 3 as watched without playing. Enter `q` or press Enter to quit.

## Configuration

Create a config file at:
- **macOS**: `~/Library/Application Support/blepo/config.toml`
- **Linux**: `~/.config/blepo/config.toml`

```toml
# Optional, defaults to 7
fetch_window_days = 7

[[channels]]
name = "3Blue1Brown"
id = "UCYO_jab_esuFRV4b17AJtAw"

[[channels]]
name = "Fireship"
id = "UCsBjURrPoezykLs9EqgamOA"
```

The channel ID is the `UC...` string from the channel's YouTube URL.

## How it works

1. Fetches RSS feeds for all configured channels, filters to the last N days
2. Filters out YouTube Shorts automatically
3. Displays unwatched videos numbered, newest first
4. Prompts for a video number — launches mpv in the background, marks it as watched, and exits (`w<number>` to mark watched without playing)
5. Watched state is stored locally — no accounts, no tracking

## Development

```bash
cargo build              # Development build
cargo test               # Run all tests
cargo clippy             # Lint
cargo fmt                # Format
```

## License

MIT
