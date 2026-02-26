use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use super::channel::ChannelId;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VideoId(String);

#[derive(Debug, PartialEq, Eq)]
pub struct VideoIdError;

impl std::fmt::Display for VideoIdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "video ID cannot be empty")
    }
}

impl std::error::Error for VideoIdError {}

impl VideoId {
    pub fn parse(id: impl Into<String>) -> Result<Self, VideoIdError> {
        let id = id.into();
        if id.is_empty() {
            return Err(VideoIdError);
        }
        Ok(Self(id))
    }
}

impl std::fmt::Display for VideoId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Video {
    pub id: VideoId,
    pub title: String,
    pub url: String,
    pub published: DateTime<Utc>,
    pub channel_name: String,
    pub channel_id: ChannelId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FetchWindowDays(i64);

#[derive(Debug, PartialEq, Eq)]
pub struct FetchWindowDaysError;

impl std::fmt::Display for FetchWindowDaysError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "fetch_window_days must be positive")
    }
}

impl std::error::Error for FetchWindowDaysError {}

impl FetchWindowDays {
    pub fn parse(days: i64) -> Result<Self, FetchWindowDaysError> {
        if days <= 0 {
            return Err(FetchWindowDaysError);
        }
        Ok(Self(days))
    }

    pub fn as_i64(self) -> i64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoNumber(usize);

#[derive(Debug, PartialEq, Eq)]
pub struct VideoNumberError;

impl std::fmt::Display for VideoNumberError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "video number must be at least 1")
    }
}

impl std::error::Error for VideoNumberError {}

impl VideoNumber {
    pub fn parse(n: usize) -> Result<Self, VideoNumberError> {
        if n == 0 {
            return Err(VideoNumberError);
        }
        Ok(Self(n))
    }

    pub fn to_index(self) -> usize {
        self.0 - 1
    }
}

#[must_use]
pub fn filter_unwatched<'a>(videos: &'a [Video], watched: &HashSet<VideoId>) -> Vec<&'a Video> {
    videos.iter().filter(|v| !watched.contains(&v.id)).collect()
}

#[must_use]
pub fn filter_by_window(videos: &[Video], after: DateTime<Utc>) -> Vec<&Video> {
    videos.iter().filter(|v| v.published >= after).collect()
}

pub fn sort_newest_first(videos: &mut [Video]) {
    videos.sort_by(|a, b| b.published.cmp(&a.published));
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn make_video(id: &str, title: &str, days_ago: i64) -> Video {
        Video {
            id: VideoId::parse(id).unwrap(),
            title: title.to_string(),
            url: format!("https://youtube.com/watch?v={id}"),
            published: Utc::now() - chrono::Duration::days(days_ago),
            channel_name: "Test Channel".to_string(),
            channel_id: ChannelId::parse("UC123").unwrap(),
        }
    }

    #[test]
    fn parses_valid_video_id() {
        let id = VideoId::parse("dQw4w9WgXcQ").unwrap();
        assert_eq!(id.to_string(), "dQw4w9WgXcQ");
    }

    #[test]
    fn rejects_empty_video_id() {
        assert_eq!(VideoId::parse(""), Err(VideoIdError));
    }

    #[test]
    fn filters_out_watched_videos() {
        let videos = vec![make_video("v1", "First", 1), make_video("v2", "Second", 2)];
        let watched: HashSet<VideoId> = [VideoId::parse("v1").unwrap()].into();

        let unwatched = filter_unwatched(&videos, &watched);

        assert_eq!(unwatched.len(), 1);
        assert_eq!(unwatched[0].id.to_string(), "v2");
    }

    #[test]
    fn returns_all_when_none_watched() {
        let videos = vec![make_video("v1", "First", 1), make_video("v2", "Second", 2)];
        let watched: HashSet<VideoId> = HashSet::new();

        let unwatched = filter_unwatched(&videos, &watched);

        assert_eq!(unwatched.len(), 2);
    }

    #[test]
    fn filters_videos_outside_window() {
        let videos = vec![
            make_video("v1", "Recent", 1),
            make_video("v2", "Old", 10),
            make_video("v3", "Edge", 6),
        ];
        let cutoff = Utc::now() - chrono::Duration::days(7);

        let within_window = filter_by_window(&videos, cutoff);

        assert_eq!(within_window.len(), 2);
        assert!(within_window.iter().any(|v| v.id.to_string() == "v1"));
        assert!(within_window.iter().any(|v| v.id.to_string() == "v3"));
    }

    #[test]
    fn sorts_videos_newest_first() {
        let mut videos = vec![
            make_video("v1", "Oldest", 5),
            make_video("v2", "Newest", 1),
            make_video("v3", "Middle", 3),
        ];

        sort_newest_first(&mut videos);

        assert_eq!(videos[0].id.to_string(), "v2");
        assert_eq!(videos[1].id.to_string(), "v3");
        assert_eq!(videos[2].id.to_string(), "v1");
    }

    #[test]
    fn filter_by_window_with_exact_boundary() {
        let boundary = Utc.with_ymd_and_hms(2024, 1, 15, 0, 0, 0).unwrap();
        let videos = vec![
            Video {
                id: VideoId::parse("v1").unwrap(),
                title: "Before".to_string(),
                url: "https://youtube.com/watch?v=v1".to_string(),
                published: Utc.with_ymd_and_hms(2024, 1, 14, 23, 59, 59).unwrap(),
                channel_name: "Test".to_string(),
                channel_id: ChannelId::parse("UC1").unwrap(),
            },
            Video {
                id: VideoId::parse("v2").unwrap(),
                title: "Exactly at".to_string(),
                url: "https://youtube.com/watch?v=v2".to_string(),
                published: boundary,
                channel_name: "Test".to_string(),
                channel_id: ChannelId::parse("UC1").unwrap(),
            },
            Video {
                id: VideoId::parse("v3").unwrap(),
                title: "After".to_string(),
                url: "https://youtube.com/watch?v=v3".to_string(),
                published: Utc.with_ymd_and_hms(2024, 1, 15, 0, 0, 1).unwrap(),
                channel_name: "Test".to_string(),
                channel_id: ChannelId::parse("UC1").unwrap(),
            },
        ];

        let result = filter_by_window(&videos, boundary);

        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|v| v.id.to_string() == "v2"));
        assert!(result.iter().any(|v| v.id.to_string() == "v3"));
    }

    #[test]
    fn parses_valid_fetch_window_days() {
        let days = FetchWindowDays::parse(7).unwrap();
        assert_eq!(days.as_i64(), 7);
    }

    #[test]
    fn rejects_zero_fetch_window_days() {
        assert_eq!(FetchWindowDays::parse(0), Err(FetchWindowDaysError));
    }

    #[test]
    fn rejects_negative_fetch_window_days() {
        assert_eq!(FetchWindowDays::parse(-1), Err(FetchWindowDaysError));
    }

    #[test]
    fn parses_valid_video_number() {
        let num = VideoNumber::parse(3).unwrap();
        assert_eq!(num.to_index(), 2);
    }

    #[test]
    fn rejects_zero_video_number() {
        assert_eq!(VideoNumber::parse(0), Err(VideoNumberError));
    }

    #[test]
    fn video_number_to_index_converts_to_zero_based() {
        assert_eq!(VideoNumber::parse(1).unwrap().to_index(), 0);
        assert_eq!(VideoNumber::parse(5).unwrap().to_index(), 4);
    }
}
