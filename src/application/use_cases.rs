use chrono::{Duration, Utc};

use crate::domain::channel::Channel;
use crate::domain::video::{
    filter_by_window, filter_unwatched, sort_newest_first, FetchWindowDays, Video,
};

use super::ports::{FeedFetcher, PlayError, ShortsChecker, StoreError, VideoPlayer, VideoStore};

#[derive(Debug)]
pub enum AppError {
    Store(StoreError),
    Play(PlayError),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Store(e) => write!(f, "{e}"),
            AppError::Play(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for AppError {}

impl From<StoreError> for AppError {
    fn from(e: StoreError) -> Self {
        AppError::Store(e)
    }
}

impl From<PlayError> for AppError {
    fn from(e: PlayError) -> Self {
        AppError::Play(e)
    }
}

pub fn fetch_videos(
    channels: &[Channel],
    fetcher: &dyn FeedFetcher,
    store: &dyn VideoStore,
    shorts_checker: &dyn ShortsChecker,
    fetch_window_days: FetchWindowDays,
) -> Result<Vec<Video>, AppError> {
    let cutoff = Utc::now() - Duration::days(fetch_window_days.as_i64());

    eprintln!("Updating videos list...");

    let mut all_videos: Vec<Video> = std::thread::scope(|s| {
        let handles: Vec<_> = channels
            .iter()
            .map(|channel| s.spawn(move || (channel, fetcher.fetch(channel))))
            .collect();

        let mut videos = Vec::new();
        for handle in handles {
            let (channel, result) = handle.join().unwrap();
            match result {
                Ok(fetched) => {
                    videos.extend(filter_by_window(&fetched, cutoff).into_iter().cloned());
                }
                Err(e) => {
                    eprintln!("Warning: failed to fetch {}: {e}", channel.name);
                }
            }
        }
        videos
    });

    sort_newest_first(&mut all_videos);

    let watched = store.load_watched()?;
    let unwatched: Vec<Video> = filter_unwatched(&all_videos, &watched)
        .into_iter()
        .cloned()
        .collect();

    let is_short: Vec<bool> = std::thread::scope(|s| {
        let handles: Vec<_> = unwatched
            .iter()
            .map(|v| s.spawn(|| shorts_checker.is_short(&v.id)))
            .collect();

        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });

    let non_shorts: Vec<Video> = unwatched
        .into_iter()
        .zip(is_short)
        .filter_map(|(v, short)| if short { None } else { Some(v) })
        .collect();

    Ok(non_shorts)
}

pub fn mark_and_play(
    video: &Video,
    store: &dyn VideoStore,
    player: &dyn VideoPlayer,
) -> Result<(), AppError> {
    println!("Playing: {} [{}]", video.title, video.channel_name);
    player.play(&video.url)?;
    store.mark_watched(&video.id)?;
    Ok(())
}

pub fn mark_as_watched(video: &Video, store: &dyn VideoStore) -> Result<(), AppError> {
    store.mark_watched(&video.id)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::ports::{
        FeedFetcher, FetchError, ShortsChecker, VideoPlayer, VideoStore,
    };
    use crate::domain::channel::{Channel, ChannelId};
    use crate::domain::video::{Video, VideoId};
    use std::cell::RefCell;
    use std::collections::HashSet;

    struct MockFetcher {
        videos: Vec<Video>,
    }

    impl FeedFetcher for MockFetcher {
        fn fetch(&self, _channel: &Channel) -> Result<Vec<Video>, FetchError> {
            Ok(self.videos.clone())
        }
    }

    struct FailingFetcher;

    impl FeedFetcher for FailingFetcher {
        fn fetch(&self, _channel: &Channel) -> Result<Vec<Video>, FetchError> {
            Err(FetchError::Network("connection refused".to_string()))
        }
    }

    struct MockStore {
        watched: RefCell<HashSet<VideoId>>,
    }

    impl MockStore {
        fn new() -> Self {
            Self {
                watched: RefCell::new(HashSet::new()),
            }
        }
    }

    impl VideoStore for MockStore {
        fn load_watched(&self) -> Result<HashSet<VideoId>, StoreError> {
            Ok(self.watched.borrow().clone())
        }

        fn mark_watched(&self, video_id: &VideoId) -> Result<(), StoreError> {
            self.watched.borrow_mut().insert(video_id.clone());
            Ok(())
        }
    }

    struct MockPlayer {
        played: RefCell<Vec<String>>,
    }

    impl MockPlayer {
        fn new() -> Self {
            Self {
                played: RefCell::new(Vec::new()),
            }
        }
    }

    impl VideoPlayer for MockPlayer {
        fn play(&self, url: &str) -> Result<(), PlayError> {
            self.played.borrow_mut().push(url.to_string());
            Ok(())
        }
    }

    struct FailingPlayer;

    impl VideoPlayer for FailingPlayer {
        fn play(&self, _url: &str) -> Result<(), PlayError> {
            Err(PlayError::PlayerFailed("mpv crashed".to_string()))
        }
    }

    struct MockShortsChecker {
        short_ids: HashSet<VideoId>,
    }

    impl MockShortsChecker {
        fn none() -> Self {
            Self {
                short_ids: HashSet::new(),
            }
        }

        fn with_shorts(ids: &[&str]) -> Self {
            Self {
                short_ids: ids.iter().map(|id| VideoId::parse(*id).unwrap()).collect(),
            }
        }
    }

    impl ShortsChecker for MockShortsChecker {
        fn is_short(&self, video_id: &VideoId) -> bool {
            self.short_ids.contains(video_id)
        }
    }

    fn make_video(id: &str, title: &str, days_ago: i64) -> Video {
        Video {
            id: VideoId::parse(id).unwrap(),
            title: title.to_string(),
            url: format!("https://youtube.com/watch?v={id}"),
            published: Utc::now() - Duration::days(days_ago),
            channel_name: "Test Channel".to_string(),
            channel_id: ChannelId::parse("UC123").unwrap(),
        }
    }

    fn test_channel() -> Channel {
        Channel {
            name: "Test Channel".to_string(),
            id: ChannelId::parse("UC123").unwrap(),
        }
    }

    fn seven_days() -> FetchWindowDays {
        FetchWindowDays::parse(7).unwrap()
    }

    #[test]
    fn fetch_videos_returns_recent_unwatched() {
        let videos = vec![make_video("v1", "Recent", 1), make_video("v2", "Old", 30)];
        let fetcher = MockFetcher { videos };
        let store = MockStore::new();
        let shorts = MockShortsChecker::none();

        let result =
            fetch_videos(&[test_channel()], &fetcher, &store, &shorts, seven_days()).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id.to_string(), "v1");
    }

    #[test]
    fn fetch_videos_continues_on_channel_failure() {
        let fetcher = FailingFetcher;
        let store = MockStore::new();
        let shorts = MockShortsChecker::none();

        let result = fetch_videos(&[test_channel()], &fetcher, &store, &shorts, seven_days());

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn fetch_videos_excludes_watched() {
        let videos = vec![make_video("v1", "First", 1), make_video("v2", "Second", 2)];
        let fetcher = MockFetcher { videos };
        let store = MockStore::new();
        store.mark_watched(&VideoId::parse("v1").unwrap()).unwrap();
        let shorts = MockShortsChecker::none();

        let unwatched =
            fetch_videos(&[test_channel()], &fetcher, &store, &shorts, seven_days()).unwrap();

        assert_eq!(unwatched.len(), 1);
        assert_eq!(unwatched[0].id.to_string(), "v2");
    }

    #[test]
    fn fetch_videos_returns_newest_first() {
        let videos = vec![make_video("v1", "Older", 3), make_video("v2", "Newer", 1)];
        let fetcher = MockFetcher { videos };
        let store = MockStore::new();
        let shorts = MockShortsChecker::none();

        let result =
            fetch_videos(&[test_channel()], &fetcher, &store, &shorts, seven_days()).unwrap();

        assert_eq!(result[0].id.to_string(), "v2");
        assert_eq!(result[1].id.to_string(), "v1");
    }

    #[test]
    fn fetch_videos_excludes_shorts() {
        let videos = vec![
            make_video("v1", "Regular", 1),
            make_video("short1", "A Short", 1),
            make_video("v2", "Also Regular", 2),
        ];
        let fetcher = MockFetcher { videos };
        let store = MockStore::new();
        let shorts = MockShortsChecker::with_shorts(&["short1"]);

        let result =
            fetch_videos(&[test_channel()], &fetcher, &store, &shorts, seven_days()).unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].id.to_string(), "v1");
        assert_eq!(result[1].id.to_string(), "v2");
    }

    #[test]
    fn mark_and_play_marks_as_watched() {
        let video = make_video("v1", "First", 1);
        let store = MockStore::new();
        let player = MockPlayer::new();

        mark_and_play(&video, &store, &player).unwrap();

        assert!(store
            .load_watched()
            .unwrap()
            .contains(&VideoId::parse("v1").unwrap()));
        assert_eq!(player.played.borrow()[0], "https://youtube.com/watch?v=v1");
    }

    #[test]
    fn mark_as_watched_marks_without_playing() {
        let video = make_video("v1", "First", 1);
        let store = MockStore::new();
        let player = MockPlayer::new();

        mark_as_watched(&video, &store).unwrap();

        assert!(store
            .load_watched()
            .unwrap()
            .contains(&VideoId::parse("v1").unwrap()));
        assert!(player.played.borrow().is_empty());
    }

    #[test]
    fn mark_and_play_returns_error_on_player_failure() {
        let video = make_video("v1", "First", 1);
        let store = MockStore::new();
        let player = FailingPlayer;

        let result = mark_and_play(&video, &store, &player);

        assert!(result.is_err());
    }
}
