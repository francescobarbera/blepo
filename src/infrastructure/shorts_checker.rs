use reqwest::blocking::Client;
use reqwest::redirect::Policy;

use crate::application::ports::ShortsChecker;
use crate::domain::video::VideoId;

pub struct HttpShortsChecker {
    client: Client,
}

impl Default for HttpShortsChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpShortsChecker {
    pub fn new() -> Self {
        let client = Client::builder()
            .redirect(Policy::none())
            .build()
            .expect("failed to build HTTP client");
        Self { client }
    }
}

impl ShortsChecker for HttpShortsChecker {
    fn is_short(&self, video_id: &VideoId) -> bool {
        let url = format!("https://www.youtube.com/shorts/{video_id}");
        match self.client.head(&url).send() {
            Ok(response) => response.status().as_u16() == 200,
            Err(_) => false,
        }
    }
}
