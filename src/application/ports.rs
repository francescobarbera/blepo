use std::collections::HashSet;

use crate::domain::channel::Channel;
use crate::domain::video::{Video, VideoId};

#[derive(Debug)]
pub enum FetchError {
    Network(String),
    HttpError(u16),
    Parse(String),
}

impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FetchError::Network(msg) => write!(f, "network error: {msg}"),
            FetchError::HttpError(status) => write!(f, "HTTP {status} from YouTube"),
            FetchError::Parse(msg) => write!(f, "parse error: {msg}"),
        }
    }
}

impl std::error::Error for FetchError {}

#[derive(Debug)]
pub enum StoreError {
    Read(String),
    Write(String),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::Read(msg) => write!(f, "store read error: {msg}"),
            StoreError::Write(msg) => write!(f, "store write error: {msg}"),
        }
    }
}

impl std::error::Error for StoreError {}

#[derive(Debug)]
pub enum PlayError {
    PlayerFailed(String),
}

impl std::fmt::Display for PlayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlayError::PlayerFailed(msg) => write!(f, "player failed: {msg}"),
        }
    }
}

impl std::error::Error for PlayError {}

pub trait FeedFetcher: Send + Sync {
    fn fetch(&self, channel: &Channel) -> Result<Vec<Video>, FetchError>;
}

pub trait VideoStore {
    fn load_watched(&self) -> Result<HashSet<VideoId>, StoreError>;
    fn mark_watched(&self, video_id: &VideoId) -> Result<(), StoreError>;
}

pub trait VideoPlayer {
    fn play(&self, url: &str) -> Result<(), PlayError>;
}

pub trait ShortsChecker: Send + Sync {
    fn is_short(&self, video_id: &VideoId) -> bool;
}
