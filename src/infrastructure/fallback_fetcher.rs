use crate::application::ports::{FeedFetcher, FetchError};
use crate::domain::channel::Channel;
use crate::domain::video::Video;

pub struct FallbackFetcher<P, F> {
    primary: P,
    fallback: F,
}

impl<P: FeedFetcher, F: FeedFetcher> FallbackFetcher<P, F> {
    pub fn new(primary: P, fallback: F) -> Self {
        Self { primary, fallback }
    }
}

impl<P: FeedFetcher, F: FeedFetcher> FeedFetcher for FallbackFetcher<P, F> {
    fn fetch(&self, channel: &Channel) -> Result<Vec<Video>, FetchError> {
        match self.primary.fetch(channel) {
            Err(FetchError::HttpError(404)) => {
                eprintln!("RSS feed returned 404, trying yt-dlp...");
                self.fallback.fetch(channel)
            }
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::channel::ChannelId;

    struct MockFetcher {
        result: Result<Vec<Video>, FetchError>,
    }

    impl MockFetcher {
        fn ok(videos: Vec<Video>) -> Self {
            Self { result: Ok(videos) }
        }

        fn err(error: FetchError) -> Self {
            Self { result: Err(error) }
        }
    }

    impl FeedFetcher for MockFetcher {
        fn fetch(&self, _channel: &Channel) -> Result<Vec<Video>, FetchError> {
            match &self.result {
                Ok(videos) => Ok(videos.clone()),
                Err(FetchError::HttpError(code)) => Err(FetchError::HttpError(*code)),
                Err(FetchError::Network(msg)) => Err(FetchError::Network(msg.clone())),
                Err(FetchError::Parse(msg)) => Err(FetchError::Parse(msg.clone())),
            }
        }
    }

    fn test_channel() -> Channel {
        Channel {
            name: "Test".to_string(),
            id: ChannelId::parse("UC_x5XG1OV2P6uZZ5FSM9Ttw").unwrap(),
        }
    }

    #[test]
    fn returns_primary_result_on_success() {
        let fetcher = FallbackFetcher::new(MockFetcher::ok(vec![]), MockFetcher::ok(vec![]));

        let result = fetcher.fetch(&test_channel());
        assert!(result.is_ok());
    }

    #[test]
    fn falls_back_on_404() {
        let primary = MockFetcher::err(FetchError::HttpError(404));
        let fallback = MockFetcher::ok(vec![]);
        let fetcher = FallbackFetcher::new(primary, fallback);

        let result = fetcher.fetch(&test_channel());
        assert!(result.is_ok());
    }

    #[test]
    fn does_not_fallback_on_500() {
        let primary = MockFetcher::err(FetchError::HttpError(500));
        let fallback = MockFetcher::ok(vec![]);
        let fetcher = FallbackFetcher::new(primary, fallback);

        let result = fetcher.fetch(&test_channel());
        assert!(matches!(result, Err(FetchError::HttpError(500))));
    }

    #[test]
    fn does_not_fallback_on_network_error() {
        let primary = MockFetcher::err(FetchError::Network("timeout".to_string()));
        let fallback = MockFetcher::ok(vec![]);
        let fetcher = FallbackFetcher::new(primary, fallback);

        let result = fetcher.fetch(&test_channel());
        assert!(matches!(result, Err(FetchError::Network(_))));
    }

    #[test]
    fn does_not_fallback_on_parse_error() {
        let primary = MockFetcher::err(FetchError::Parse("bad xml".to_string()));
        let fallback = MockFetcher::ok(vec![]);
        let fetcher = FallbackFetcher::new(primary, fallback);

        let result = fetcher.fetch(&test_channel());
        assert!(matches!(result, Err(FetchError::Parse(_))));
    }

    #[test]
    fn propagates_fallback_error() {
        let primary = MockFetcher::err(FetchError::HttpError(404));
        let fallback = MockFetcher::err(FetchError::Network("yt-dlp failed".to_string()));
        let fetcher = FallbackFetcher::new(primary, fallback);

        let result = fetcher.fetch(&test_channel());
        assert!(matches!(result, Err(FetchError::Network(_))));
    }
}
