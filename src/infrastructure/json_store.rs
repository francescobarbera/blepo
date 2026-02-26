use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use crate::application::ports::{StoreError, VideoStore};
use crate::domain::video::VideoId;

pub struct JsonVideoStore {
    watched_path: PathBuf,
}

impl JsonVideoStore {
    pub fn new(data_dir: &PathBuf) -> Result<Self, StoreError> {
        fs::create_dir_all(data_dir)
            .map_err(|e| StoreError::Write(format!("cannot create data dir: {e}")))?;

        Ok(Self {
            watched_path: data_dir.join("watched.json"),
        })
    }
}

impl VideoStore for JsonVideoStore {
    fn load_watched(&self) -> Result<HashSet<VideoId>, StoreError> {
        if !self.watched_path.exists() {
            return Ok(HashSet::new());
        }

        let content = fs::read_to_string(&self.watched_path)
            .map_err(|e| StoreError::Read(format!("cannot read watched: {e}")))?;

        serde_json::from_str(&content)
            .map_err(|e| StoreError::Read(format!("invalid watched json: {e}")))
    }

    fn mark_watched(&self, video_id: &VideoId) -> Result<(), StoreError> {
        let mut watched = self.load_watched()?;
        watched.insert(video_id.clone());

        let json = serde_json::to_string_pretty(&watched)
            .map_err(|e| StoreError::Write(format!("cannot serialize watched: {e}")))?;

        fs::write(&self.watched_path, json)
            .map_err(|e| StoreError::Write(format!("cannot write watched: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn returns_empty_when_no_file() {
        let dir = TempDir::new().unwrap();
        let store = JsonVideoStore::new(&dir.path().to_path_buf()).unwrap();

        let watched = store.load_watched().unwrap();

        assert!(watched.is_empty());
    }

    #[test]
    fn marks_and_loads_watched() {
        let dir = TempDir::new().unwrap();
        let store = JsonVideoStore::new(&dir.path().to_path_buf()).unwrap();

        store.mark_watched(&VideoId::parse("v1").unwrap()).unwrap();
        store.mark_watched(&VideoId::parse("v2").unwrap()).unwrap();

        let watched = store.load_watched().unwrap();

        assert!(watched.contains(&VideoId::parse("v1").unwrap()));
        assert!(watched.contains(&VideoId::parse("v2").unwrap()));
        assert!(!watched.contains(&VideoId::parse("v3").unwrap()));
    }

    #[test]
    fn mark_watched_is_idempotent() {
        let dir = TempDir::new().unwrap();
        let store = JsonVideoStore::new(&dir.path().to_path_buf()).unwrap();

        store.mark_watched(&VideoId::parse("v1").unwrap()).unwrap();
        store.mark_watched(&VideoId::parse("v1").unwrap()).unwrap();

        let watched = store.load_watched().unwrap();

        assert_eq!(watched.len(), 1);
    }
}
