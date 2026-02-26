# Blepo Specifications

## Overview

Blepo is a Rust CLI tool for watching YouTube subscriptions without ads, distractions, or tracking. The name comes from the Ancient Greek *blepo* (βλέπω) — "I see, I look." It fetches videos via RSS feeds (with yt-dlp as a fallback) and plays them through mpv + yt-dlp.

## Usage

Running `blepo` with no arguments:

1. Fetches latest videos from all configured channels (RSS first, yt-dlp fallback on 404)
2. Filters videos to the configured time window (default: 7 days)
3. Excludes videos tracked in `watched.json`
4. Filters out YouTube Shorts (via HTTP HEAD check)
5. Sorts by published date, newest first
6. Displays numbered list: `  1. [2024-01-20] Channel Name — Video Title`
7. Shows "No unwatched videos." and exits if list is empty
8. Prompts: `Enter number to play, w<number> to mark watched, q to quit: `
9. On valid number: launches mpv in the background, marks video as watched, blepo exits
10. On `w<number>`: marks the video as watched without playing, prints confirmation
11. On "q" or empty input: exits

One video per invocation. Run again to pick another.

### Fetching behavior

- Tries RSS feed first (`https://www.youtube.com/feeds/videos.xml?channel_id=<id>`)
- If RSS returns HTTP 404, falls back to yt-dlp (`yt-dlp --flat-playlist --dump-json --extractor-args "youtubetab:approximate_date"`)
- Other errors (network, parse, non-404 HTTP) propagate immediately — no fallback
- Prints "RSS feed returned 404, trying yt-dlp..." to stderr when falling back
- Continues fetching remaining channels if one fails (logs warning to stderr)
- Prints summary to stderr: "Fetched N videos from M channels"
- Channel fetching and Shorts checking run in parallel using `std::thread::scope` (one thread per channel/video)

### Date precision

- **RSS feeds**: exact timestamps (e.g., `2024-01-20T15:00:00Z`)
- **yt-dlp fallback**: uses `timestamp` (Unix epoch) when available via `approximate_date`, falls back to `upload_date` (YYYYMMDD → midnight UTC), defaults to now if neither present

### Shorts filtering

YouTube Shorts are filtered out before displaying the video list:

- A HEAD request is sent to `https://www.youtube.com/shorts/<video_id>` with redirects disabled
- HTTP 200 → video is a Short (filtered out)
- Any other status or network error → video is kept (fail-open)

### Playback

- Checks that `mpv` and `yt-dlp` are installed before attempting playback
- Launches `mpv <url>` in the background (yt-dlp is used by mpv automatically)
- mpv runs detached — blepo exits immediately after launch
- Marks the video as watched in `watched.json` at launch time
- Prints "Playing: <title> [<channel>]" before launching

## Configuration

Platform-dependent path resolved by the `directories` crate:
- **macOS**: `~/Library/Application Support/blepo/config.toml`
- **Linux**: `~/.config/blepo/config.toml`

```toml
# Optional, defaults to 7
fetch_window_days = 7

[[channels]]
name = "Channel Name"
id = "UCxxxxxxxxxxxxxxxxxxxxxx"
```

## Data Storage

Paths resolved by the `directories` crate (platform-native):

| File | macOS | Linux |
|------|-------|-------|
| Config | `~/Library/Application Support/blepo/config.toml` | `~/.config/blepo/config.toml` |
| Watched | `~/Library/Application Support/blepo/watched.json` | `~/.local/share/blepo/watched.json` |

### watched.json

Set of video IDs:

```json
["dQw4w9WgXcQ", "abc123def45"]
```

Videos are not persisted — they are fetched fresh each run and held in memory only.

## Architecture

Clean Architecture with four layers:

- **Domain** (`src/domain/`): `Channel`, `ChannelId`, `Video`, `VideoId`, `FetchWindowDays`, `VideoNumber`, pure filtering/sorting functions
- **Application** (`src/application/`): Port traits (`FeedFetcher`, `VideoStore`, `VideoPlayer`, `ShortsChecker`), use cases (`fetch_videos`, `mark_and_play`, `mark_as_watched`)
- **Infrastructure** (`src/infrastructure/`): `RssFeedFetcher`, `YtDlpFetcher`, `FallbackFetcher`, `JsonVideoStore`, `MpvPlayer`, `HttpShortsChecker`, config parsing
- **Presentation** (`src/presentation/`): Single interactive command with stdin prompt

### Parse, Don't Validate

All data is parsed into validated domain types at system boundaries:

- **`ChannelId`**: Validated at config loading — must be non-empty and start with "UC"
- **`VideoId`**: Validated at RSS parsing — must be non-empty
- **`FetchWindowDays`**: Validated at config loading — must be positive
- **`VideoNumber`**: Validated at user input — must be >= 1, converts to 0-based index
- **`ConfigError`**: Structured error enum replacing stringly-typed errors
- **RSS date parsing**: Errors propagated (not silently dropped)

Once parsed, downstream code trusts the types without re-validation.

## Runtime Dependencies

- `mpv` — video player
- `yt-dlp` — YouTube stream extraction (used by mpv for playback, and directly as fallback fetcher when RSS is unavailable)

## Error Handling

- Custom error enums per layer: `FetchError`, `StoreError`, `PlayError`, `AppError`, `ConfigError`
- Domain parse errors: `ChannelIdError`, `VideoIdError`, `FetchWindowDaysError`, `VideoNumberError`
- Manual `Display` and `Error` implementations (no external error crates)
- Errors propagated with `?`, converted at layer boundaries
- Channel fetch failures are warnings, not fatal errors
