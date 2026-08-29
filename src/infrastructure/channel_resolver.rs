use std::process::Command;

use serde::Deserialize;

use crate::domain::channel::{Channel, ChannelId};

const YOUTUBE_URL: &str = "https://www.youtube.com/";

#[derive(Debug, PartialEq)]
pub enum ChannelResolverError {
    InvalidInput(String),
    YtDlp(String),
    InvalidOutput(String),
}

impl std::fmt::Display for ChannelResolverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInput(message) => write!(f, "invalid channel: {message}"),
            Self::YtDlp(message) => write!(f, "cannot resolve channel: {message}"),
            Self::InvalidOutput(message) => {
                write!(f, "yt-dlp returned invalid channel information: {message}")
            }
        }
    }
}

impl std::error::Error for ChannelResolverError {}

#[derive(Deserialize)]
struct YtDlpChannel {
    channel: Option<String>,
    channel_id: Option<String>,
}

pub fn resolve_channel(input: &str) -> Result<Channel, ChannelResolverError> {
    let url = normalize_channel_input(input)?;
    let output = Command::new("yt-dlp")
        .args([
            "--flat-playlist",
            "--playlist-items",
            "0",
            "--dump-single-json",
            &url,
        ])
        .output()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                ChannelResolverError::YtDlp(
                    "yt-dlp is not installed or is not available in PATH".to_string(),
                )
            } else {
                ChannelResolverError::YtDlp(format!("failed to run yt-dlp: {error}"))
            }
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = stderr.trim();
        return Err(ChannelResolverError::YtDlp(if detail.is_empty() {
            "yt-dlp failed without an error message".to_string()
        } else {
            detail.to_string()
        }));
    }

    let stdout = String::from_utf8(output.stdout)
        .map_err(|error| ChannelResolverError::InvalidOutput(error.to_string()))?;
    parse_channel_output(&stdout)
}

fn normalize_channel_input(input: &str) -> Result<String, ChannelResolverError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(ChannelResolverError::InvalidInput(
            "provide an @handle, YouTube channel URL, or UC channel ID".to_string(),
        ));
    }

    if let Some(handle) = input.strip_prefix('@') {
        if handle.is_empty() || handle.chars().any(char::is_whitespace) {
            return Err(ChannelResolverError::InvalidInput(
                "the YouTube handle is malformed".to_string(),
            ));
        }
        return Ok(format!("{YOUTUBE_URL}@{handle}"));
    }

    if input.starts_with("UC") {
        ChannelId::parse(input).map_err(|error| {
            ChannelResolverError::InvalidInput(format!("invalid channel ID: {error}"))
        })?;
        return Ok(format!("{YOUTUBE_URL}channel/{input}"));
    }

    let url = if input.starts_with("youtube.com/") || input.starts_with("www.youtube.com/") {
        format!("https://{input}")
    } else {
        input.to_string()
    };

    let Some(path) = youtube_path(&url) else {
        return Err(ChannelResolverError::InvalidInput(
            "expected an @handle, YouTube channel URL, or UC channel ID".to_string(),
        ));
    };

    if !(path.starts_with('@')
        || path.starts_with("channel/")
        || path.starts_with("c/")
        || path.starts_with("user/"))
    {
        return Err(ChannelResolverError::InvalidInput(
            "the URL does not point to a YouTube channel".to_string(),
        ));
    }

    Ok(url)
}

fn youtube_path(url: &str) -> Option<&str> {
    [
        "https://www.youtube.com/",
        "https://youtube.com/",
        "https://m.youtube.com/",
        "http://www.youtube.com/",
        "http://youtube.com/",
        "http://m.youtube.com/",
    ]
    .into_iter()
    .find_map(|prefix| url.strip_prefix(prefix))
}

fn parse_channel_output(output: &str) -> Result<Channel, ChannelResolverError> {
    let metadata: YtDlpChannel = serde_json::from_str(output.trim())
        .map_err(|error| ChannelResolverError::InvalidOutput(error.to_string()))?;

    let name = metadata
        .channel
        .filter(|name| !name.trim().is_empty())
        .ok_or_else(|| ChannelResolverError::InvalidOutput("missing channel name".to_string()))?;
    let raw_id = metadata
        .channel_id
        .ok_or_else(|| ChannelResolverError::InvalidOutput("missing channel ID".to_string()))?;
    let id = ChannelId::parse(raw_id)
        .map_err(|error| ChannelResolverError::InvalidOutput(error.to_string()))?;

    Ok(Channel { name, id })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_handle() {
        assert_eq!(
            normalize_channel_input("@Fireship").unwrap(),
            "https://www.youtube.com/@Fireship"
        );
    }

    #[test]
    fn normalizes_raw_channel_id() {
        assert_eq!(
            normalize_channel_input("UCsBjURrPoezykLs9EqgamOA").unwrap(),
            "https://www.youtube.com/channel/UCsBjURrPoezykLs9EqgamOA"
        );
    }

    #[test]
    fn adds_scheme_to_youtube_url() {
        assert_eq!(
            normalize_channel_input("youtube.com/@Fireship").unwrap(),
            "https://youtube.com/@Fireship"
        );
    }

    #[test]
    fn accepts_channel_url() {
        let url = "https://www.youtube.com/channel/UCsBjURrPoezykLs9EqgamOA/videos";
        assert_eq!(normalize_channel_input(url).unwrap(), url);
    }

    #[test]
    fn rejects_bare_channel_name() {
        assert!(matches!(
            normalize_channel_input("Fireship"),
            Err(ChannelResolverError::InvalidInput(_))
        ));
    }

    #[test]
    fn rejects_video_url() {
        assert!(matches!(
            normalize_channel_input("https://www.youtube.com/watch?v=abc"),
            Err(ChannelResolverError::InvalidInput(_))
        ));
    }

    #[test]
    fn parses_channel_metadata() {
        let output = r#"{
            "channel": "Fireship",
            "channel_id": "UCsBjURrPoezykLs9EqgamOA"
        }"#;

        let channel = parse_channel_output(output).unwrap();

        assert_eq!(channel.name, "Fireship");
        assert_eq!(channel.id.to_string(), "UCsBjURrPoezykLs9EqgamOA");
    }

    #[test]
    fn preserves_unicode_channel_name() {
        let output = r#"{
            "channel": "Crème brûlée 日本語",
            "channel_id": "UC123"
        }"#;

        let channel = parse_channel_output(output).unwrap();

        assert_eq!(channel.name, "Crème brûlée 日本語");
    }

    #[test]
    fn rejects_metadata_without_channel_id() {
        let result = parse_channel_output(r#"{"channel":"Fireship"}"#);

        assert!(matches!(
            result,
            Err(ChannelResolverError::InvalidOutput(message))
                if message == "missing channel ID"
        ));
    }
}
