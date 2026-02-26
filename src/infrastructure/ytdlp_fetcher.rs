use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use serde::Deserialize;
use std::process::Command;

use crate::application::ports::{FeedFetcher, FetchError};
use crate::domain::channel::Channel;
use crate::domain::video::{Video, VideoId};

const CHANNEL_URL_TEMPLATE: &str = "https://www.youtube.com/channel/";

#[derive(Debug, Deserialize)]
struct YtDlpEntry {
    id: String,
    title: Option<String>,
    url: Option<String>,
    timestamp: Option<i64>,
    upload_date: Option<String>,
}

pub struct YtDlpFetcher;

impl YtDlpFetcher {
    pub fn new() -> Self {
        Self
    }
}

impl Default for YtDlpFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl FeedFetcher for YtDlpFetcher {
    fn fetch(&self, channel: &Channel) -> Result<Vec<Video>, FetchError> {
        let url = format!("{CHANNEL_URL_TEMPLATE}{}/videos", channel.id);
        let output = Command::new("yt-dlp")
            .args([
                "--flat-playlist",
                "--dump-json",
                "--extractor-args",
                "youtubetab:approximate_date",
                &url,
            ])
            .output()
            .map_err(|e| FetchError::Network(format!("failed to run yt-dlp: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(FetchError::Network(format!("yt-dlp failed: {stderr}")));
        }

        let stdout =
            String::from_utf8(output.stdout).map_err(|e| FetchError::Parse(e.to_string()))?;

        parse_ytdlp_output(&stdout, channel)
    }
}

pub fn parse_ytdlp_output(jsonl: &str, channel: &Channel) -> Result<Vec<Video>, FetchError> {
    jsonl
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| parse_entry(line, channel))
        .collect()
}

fn parse_entry(json_line: &str, channel: &Channel) -> Result<Video, FetchError> {
    let entry: YtDlpEntry =
        serde_json::from_str(json_line).map_err(|e| FetchError::Parse(e.to_string()))?;

    let id = VideoId::parse(&entry.id)
        .map_err(|e| FetchError::Parse(format!("invalid video ID: {e}")))?;

    let published = if let Some(ts) = entry.timestamp {
        Utc.timestamp_opt(ts, 0)
            .single()
            .ok_or_else(|| FetchError::Parse(format!("invalid timestamp: {ts}")))?
    } else if let Some(ref date_str) = entry.upload_date {
        parse_upload_date(date_str)?
    } else {
        Utc::now()
    };

    let title = entry.title.unwrap_or_default();
    let url = entry
        .url
        .unwrap_or_else(|| format!("https://www.youtube.com/watch?v={}", entry.id));

    Ok(Video {
        id,
        title,
        url,
        published,
        channel_name: channel.name.clone(),
        channel_id: channel.id.clone(),
    })
}

fn parse_upload_date(date_str: &str) -> Result<DateTime<Utc>, FetchError> {
    NaiveDate::parse_from_str(date_str, "%Y%m%d")
        .map(|date| {
            date.and_hms_opt(0, 0, 0)
                .expect("midnight is always valid")
                .and_utc()
        })
        .map_err(|e| FetchError::Parse(format!("invalid upload_date '{date_str}': {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::channel::ChannelId;

    fn test_channel() -> Channel {
        Channel {
            name: "Test Channel".to_string(),
            id: ChannelId::parse("UC_x5XG1OV2P6uZZ5FSM9Ttw").unwrap(),
        }
    }

    #[test]
    fn parses_valid_jsonl() {
        let jsonl = r#"{"id": "abc123", "title": "My Video", "url": "https://www.youtube.com/watch?v=abc123", "upload_date": "20240120"}
{"id": "def456", "title": "Another Video", "url": "https://www.youtube.com/watch?v=def456", "upload_date": "20240118"}"#;

        let videos = parse_ytdlp_output(jsonl, &test_channel()).unwrap();

        assert_eq!(videos.len(), 2);
        assert_eq!(videos[0].id.to_string(), "abc123");
        assert_eq!(videos[0].title, "My Video");
        assert_eq!(videos[0].url, "https://www.youtube.com/watch?v=abc123");
        assert_eq!(videos[0].channel_name, "Test Channel");
        assert_eq!(videos[1].id.to_string(), "def456");
    }

    #[test]
    fn prefers_timestamp_over_upload_date() {
        let jsonl = r#"{"id": "vid1", "title": "Test", "url": "https://www.youtube.com/watch?v=vid1", "timestamp": 1705334400, "upload_date": "20240115"}"#;

        let videos = parse_ytdlp_output(jsonl, &test_channel()).unwrap();

        let expected: DateTime<Utc> = "2024-01-15T16:00:00Z".parse().unwrap();
        assert_eq!(videos[0].published, expected);
    }

    #[test]
    fn falls_back_to_upload_date_when_no_timestamp() {
        let jsonl = r#"{"id": "vid1", "title": "Test", "url": "https://www.youtube.com/watch?v=vid1", "upload_date": "20240115"}"#;

        let videos = parse_ytdlp_output(jsonl, &test_channel()).unwrap();

        let expected: DateTime<Utc> = "2024-01-15T00:00:00Z".parse().unwrap();
        assert_eq!(videos[0].published, expected);
    }

    #[test]
    fn defaults_to_now_when_upload_date_missing() {
        let jsonl = r#"{"id": "live1", "title": "Live Stream", "url": "https://www.youtube.com/watch?v=live1"}"#;

        let before = Utc::now();
        let videos = parse_ytdlp_output(jsonl, &test_channel()).unwrap();
        let after = Utc::now();

        assert!(videos[0].published >= before);
        assert!(videos[0].published <= after);
    }

    #[test]
    fn handles_empty_output() {
        let videos = parse_ytdlp_output("", &test_channel()).unwrap();
        assert!(videos.is_empty());
    }

    #[test]
    fn returns_error_for_invalid_json() {
        let result = parse_ytdlp_output("not json", &test_channel());
        assert!(matches!(result, Err(FetchError::Parse(_))));
    }

    #[test]
    fn returns_error_for_invalid_date_format() {
        let jsonl = r#"{"id": "vid1", "title": "Test", "url": "https://www.youtube.com/watch?v=vid1", "upload_date": "2024-01-15"}"#;

        let result = parse_ytdlp_output(jsonl, &test_channel());
        assert!(
            matches!(result, Err(FetchError::Parse(msg)) if msg.contains("invalid upload_date"))
        );
    }

    #[test]
    fn generates_url_when_missing() {
        let jsonl = r#"{"id": "vid1", "title": "Test", "upload_date": "20240115"}"#;

        let videos = parse_ytdlp_output(jsonl, &test_channel()).unwrap();
        assert_eq!(videos[0].url, "https://www.youtube.com/watch?v=vid1");
    }

    #[test]
    fn skips_blank_lines() {
        let jsonl = r#"{"id": "vid1", "title": "Test", "url": "https://www.youtube.com/watch?v=vid1", "upload_date": "20240115"}

{"id": "vid2", "title": "Test 2", "url": "https://www.youtube.com/watch?v=vid2", "upload_date": "20240116"}"#;

        let videos = parse_ytdlp_output(jsonl, &test_channel()).unwrap();
        assert_eq!(videos.len(), 2);
    }
}
