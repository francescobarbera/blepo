use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize)]
struct ExpectedVideo {
    id: String,
    title: String,
    url: String,
    published: DateTime<Utc>,
    channel_name: String,
    channel_id: String,
}

/// Integration test: parse yt-dlp JSONL fixture and verify against expected output.
///
/// Mirrors `tests/rss_parsing.rs` but for the yt-dlp fallback fetcher.
#[test]
fn parses_fixture_ytdlp_output_to_expected_videos() {
    let jsonl = fs::read_to_string("tests/fixtures/input/ytdlp_channel_output.jsonl")
        .expect("cannot read input fixture");

    let expected_json = fs::read_to_string("tests/fixtures/expected/ytdlp_parsed_videos.json")
        .expect("cannot read expected fixture");

    let expected: Vec<ExpectedVideo> =
        serde_json::from_str(&expected_json).expect("cannot parse expected fixture");

    let channel = blepo::domain::channel::Channel {
        name: "Google for Developers".to_string(),
        id: blepo::domain::channel::ChannelId::parse("UC_x5XG1OV2P6uZZ5FSM9Ttw").unwrap(),
    };

    let videos = blepo::infrastructure::ytdlp_fetcher::parse_ytdlp_output(&jsonl, &channel)
        .expect("failed to parse yt-dlp output");

    assert_eq!(videos.len(), expected.len());

    for (actual, exp) in videos.iter().zip(expected.iter()) {
        assert_eq!(actual.id.to_string(), exp.id);
        assert_eq!(actual.title, exp.title);
        assert_eq!(actual.url, exp.url);
        assert_eq!(actual.published, exp.published);
        assert_eq!(actual.channel_name, exp.channel_name);
        assert_eq!(actual.channel_id.to_string(), exp.channel_id);
    }
}
