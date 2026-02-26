use chrono::{DateTime, Utc};
use quick_xml::de::from_str;
use serde::Deserialize;

use crate::application::ports::{FeedFetcher, FetchError};
use crate::domain::channel::Channel;
use crate::domain::video::{Video, VideoId};

const RSS_URL_TEMPLATE: &str = "https://www.youtube.com/feeds/videos.xml?channel_id=";

#[derive(Debug, Deserialize)]
struct Feed {
    #[serde(default)]
    entry: Vec<Entry>,
}

#[derive(Debug, Deserialize)]
struct Entry {
    #[serde(rename = "videoId")]
    video_id: String,
    title: String,
    published: String,
    link: Link,
}

#[derive(Debug, Deserialize)]
struct Link {
    #[serde(rename = "@href")]
    href: String,
}

pub struct RssFeedFetcher {
    client: reqwest::blocking::Client,
}

impl Default for RssFeedFetcher {
    fn default() -> Self {
        Self {
            client: reqwest::blocking::Client::new(),
        }
    }
}

impl RssFeedFetcher {
    pub fn new() -> Self {
        Self::default()
    }
}

impl FeedFetcher for RssFeedFetcher {
    fn fetch(&self, channel: &Channel) -> Result<Vec<Video>, FetchError> {
        let url = format!("{RSS_URL_TEMPLATE}{}", channel.id);
        let response = self
            .client
            .get(&url)
            .send()
            .map_err(|e| FetchError::Network(e.to_string()))?;

        let status = response.status().as_u16();
        if !response.status().is_success() {
            return Err(FetchError::HttpError(status));
        }

        let body = response
            .text()
            .map_err(|e| FetchError::Network(e.to_string()))?;

        parse_feed(&body, channel)
    }
}

pub fn parse_feed(xml: &str, channel: &Channel) -> Result<Vec<Video>, FetchError> {
    let feed: Feed = from_str(xml).map_err(|e| FetchError::Parse(e.to_string()))?;

    feed.entry
        .into_iter()
        .map(|entry| parse_entry(entry, channel))
        .collect()
}

fn parse_entry(entry: Entry, channel: &Channel) -> Result<Video, FetchError> {
    let id = VideoId::parse(entry.video_id)
        .map_err(|e| FetchError::Parse(format!("invalid video ID: {e}")))?;

    let published: DateTime<Utc> = entry
        .published
        .parse()
        .map_err(|e| FetchError::Parse(format!("invalid date '{}': {e}", entry.published)))?;

    Ok(Video {
        id,
        title: entry.title,
        url: entry.link.href,
        published,
        channel_name: channel.name.clone(),
        channel_id: channel.id.clone(),
    })
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

    const SAMPLE_FEED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns:yt="http://www.youtube.com/xml/schemas/2015" xmlns:media="http://search.yahoo.com/mrss/" xmlns="http://www.w3.org/2005/Atom">
  <title>Test Channel</title>
  <entry>
    <yt:videoId>dQw4w9WgXcQ</yt:videoId>
    <title>Test Video 1</title>
    <link rel="alternate" href="https://www.youtube.com/watch?v=dQw4w9WgXcQ"/>
    <published>2024-01-15T10:00:00+00:00</published>
  </entry>
  <entry>
    <yt:videoId>abc123def45</yt:videoId>
    <title>Test Video 2</title>
    <link rel="alternate" href="https://www.youtube.com/watch?v=abc123def45"/>
    <published>2024-01-14T08:30:00+00:00</published>
  </entry>
</feed>"#;

    #[test]
    fn parses_youtube_rss_feed() {
        let channel = test_channel();
        let videos = parse_feed(SAMPLE_FEED, &channel).unwrap();

        assert_eq!(videos.len(), 2);
        assert_eq!(videos[0].id.to_string(), "dQw4w9WgXcQ");
        assert_eq!(videos[0].title, "Test Video 1");
        assert_eq!(videos[0].url, "https://www.youtube.com/watch?v=dQw4w9WgXcQ");
        assert_eq!(videos[0].channel_name, "Test Channel");
    }

    #[test]
    fn parses_published_dates() {
        let channel = test_channel();
        let videos = parse_feed(SAMPLE_FEED, &channel).unwrap();

        assert_eq!(
            videos[0].published,
            "2024-01-15T10:00:00+00:00"
                .parse::<DateTime<Utc>>()
                .unwrap()
        );
    }

    #[test]
    fn handles_empty_feed() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>Empty Channel</title>
</feed>"#;

        let channel = test_channel();
        let videos = parse_feed(xml, &channel).unwrap();

        assert!(videos.is_empty());
    }

    #[test]
    fn returns_error_for_invalid_xml() {
        let result = parse_feed("not xml at all", &test_channel());

        assert!(result.is_err());
    }

    #[test]
    fn returns_error_for_invalid_date() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns:yt="http://www.youtube.com/xml/schemas/2015" xmlns="http://www.w3.org/2005/Atom">
  <entry>
    <yt:videoId>vid1</yt:videoId>
    <title>Bad Date Video</title>
    <link rel="alternate" href="https://www.youtube.com/watch?v=vid1"/>
    <published>not-a-date</published>
  </entry>
</feed>"#;

        let result = parse_feed(xml, &test_channel());

        assert!(matches!(result, Err(FetchError::Parse(msg)) if msg.contains("invalid date")));
    }
}
