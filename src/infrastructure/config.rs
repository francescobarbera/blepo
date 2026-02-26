use std::fs;
use std::path::PathBuf;

use directories::ProjectDirs;
use serde::Deserialize;

use crate::domain::channel::{Channel, ChannelId};
use crate::domain::video::FetchWindowDays;

const DEFAULT_FETCH_WINDOW_DAYS: i64 = 7;

#[derive(Debug)]
pub enum ConfigError {
    NotFound(PathBuf),
    Read(String),
    InvalidToml(String),
    InvalidChannel { name: String, reason: String },
    InvalidFetchWindow(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::NotFound(path) => write!(
                f,
                "config file not found at {}\n\nCreate it with:\n\n\
                 [[channels]]\n\
                 name = \"Channel Name\"\n\
                 id = \"UCxxxxxxxxxxxxxxxxxxxxxx\"\n",
                path.display()
            ),
            ConfigError::Read(msg) => write!(f, "cannot read config: {msg}"),
            ConfigError::InvalidToml(msg) => write!(f, "invalid config: {msg}"),
            ConfigError::InvalidChannel { name, reason } => {
                write!(f, "invalid channel \"{name}\": {reason}")
            }
            ConfigError::InvalidFetchWindow(msg) => {
                write!(f, "invalid fetch_window_days: {msg}")
            }
        }
    }
}

impl std::error::Error for ConfigError {}

#[derive(Debug, Deserialize)]
struct ConfigFile {
    fetch_window_days: Option<i64>,
    channels: Option<Vec<ChannelEntry>>,
}

#[derive(Debug, Deserialize)]
struct ChannelEntry {
    name: String,
    id: String,
}

#[derive(Debug)]
pub struct AppConfig {
    pub fetch_window_days: FetchWindowDays,
    pub channels: Vec<Channel>,
    pub data_dir: PathBuf,
}

pub fn load_config() -> Result<AppConfig, ConfigError> {
    let dirs = ProjectDirs::from("", "", "blepo").ok_or(ConfigError::Read(
        "cannot determine home directory".to_string(),
    ))?;

    let config_path = dirs.config_dir().join("config.toml");
    let data_dir = dirs.data_dir().to_path_buf();

    load_config_from_path(&config_path, data_dir)
}

fn load_config_from_path(
    config_path: &std::path::Path,
    data_dir: PathBuf,
) -> Result<AppConfig, ConfigError> {
    if !config_path.exists() {
        return Err(ConfigError::NotFound(config_path.to_path_buf()));
    }

    let content = fs::read_to_string(config_path)
        .map_err(|e| ConfigError::Read(format!("{}: {e}", config_path.display())))?;

    parse_config_str(&content, data_dir)
}

fn parse_config_str(content: &str, data_dir: PathBuf) -> Result<AppConfig, ConfigError> {
    let config: ConfigFile =
        toml::from_str(content).map_err(|e| ConfigError::InvalidToml(e.to_string()))?;

    let raw_days = config
        .fetch_window_days
        .unwrap_or(DEFAULT_FETCH_WINDOW_DAYS);
    let fetch_window_days = FetchWindowDays::parse(raw_days)
        .map_err(|e| ConfigError::InvalidFetchWindow(e.to_string()))?;

    let channels = config
        .channels
        .unwrap_or_default()
        .into_iter()
        .map(|entry| {
            let id = ChannelId::parse(&entry.id).map_err(|e| ConfigError::InvalidChannel {
                name: entry.name.clone(),
                reason: e.to_string(),
            })?;
            Ok(Channel {
                name: entry.name,
                id,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(AppConfig {
        fetch_window_days,
        channels,
        data_dir,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn parse(toml_str: &str) -> Result<AppConfig, ConfigError> {
        parse_config_str(toml_str, PathBuf::from("/tmp/test"))
    }

    #[test]
    fn load_config_returns_not_found_for_missing_file() {
        let dir = TempDir::new().unwrap();
        let missing = dir.path().join("config.toml");

        let result = load_config_from_path(&missing, dir.path().to_path_buf());

        assert!(matches!(result, Err(ConfigError::NotFound(p)) if p == missing));
    }

    #[test]
    fn load_config_reads_and_parses_file() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("config.toml");
        std::fs::write(
            &config_path,
            r#"
            fetch_window_days = 3

            [[channels]]
            name = "Test"
            id = "UC123"
            "#,
        )
        .unwrap();

        let config = load_config_from_path(&config_path, dir.path().to_path_buf()).unwrap();

        assert_eq!(config.fetch_window_days.as_i64(), 3);
        assert_eq!(config.channels.len(), 1);
        assert_eq!(config.data_dir, dir.path());
    }

    #[test]
    fn load_config_rejects_invalid_file_content() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("config.toml");
        std::fs::write(&config_path, "not valid [[[").unwrap();

        let result = load_config_from_path(&config_path, dir.path().to_path_buf());

        assert!(matches!(result, Err(ConfigError::InvalidToml(_))));
    }

    #[test]
    fn parses_valid_config() {
        let toml = r#"
            fetch_window_days = 14

            [[channels]]
            name = "Test Channel"
            id = "UC123"

            [[channels]]
            name = "Another Channel"
            id = "UC456"
        "#;

        let config = parse(toml).unwrap();

        assert_eq!(config.fetch_window_days.as_i64(), 14);
        assert_eq!(config.channels.len(), 2);
        assert_eq!(config.channels[0].name, "Test Channel");
        assert_eq!(config.channels[0].id.to_string(), "UC123");
    }

    #[test]
    fn uses_default_fetch_window() {
        let toml = r#"
            [[channels]]
            name = "Test"
            id = "UC123"
        "#;

        let config = parse(toml).unwrap();

        assert_eq!(config.fetch_window_days.as_i64(), DEFAULT_FETCH_WINDOW_DAYS);
    }

    #[test]
    fn handles_empty_channels() {
        let config = parse("").unwrap();

        assert!(config.channels.is_empty());
    }

    #[test]
    fn rejects_invalid_toml() {
        let result = parse("this is not valid toml [[[");

        assert!(matches!(result, Err(ConfigError::InvalidToml(_))));
    }

    #[test]
    fn rejects_negative_fetch_window_days() {
        let toml = r#"
            fetch_window_days = -1

            [[channels]]
            name = "Test"
            id = "UC123"
        "#;

        let result = parse(toml);

        assert!(matches!(result, Err(ConfigError::InvalidFetchWindow(_))));
    }

    #[test]
    fn rejects_zero_fetch_window_days() {
        let toml = r#"
            fetch_window_days = 0

            [[channels]]
            name = "Test"
            id = "UC123"
        "#;

        let result = parse(toml);

        assert!(matches!(result, Err(ConfigError::InvalidFetchWindow(_))));
    }

    #[test]
    fn rejects_channel_without_uc_prefix() {
        let toml = r#"
            [[channels]]
            name = "Bad Channel"
            id = "not-a-channel-id"
        "#;

        let result = parse(toml);

        assert!(matches!(
            result,
            Err(ConfigError::InvalidChannel { name, .. }) if name == "Bad Channel"
        ));
    }

    #[test]
    fn rejects_channel_with_empty_id() {
        let toml = r#"
            [[channels]]
            name = "Empty ID"
            id = ""
        "#;

        let result = parse(toml);

        assert!(matches!(result, Err(ConfigError::InvalidChannel { .. })));
    }
}
