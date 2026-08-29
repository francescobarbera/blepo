use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

use crate::domain::channel::{Channel, ChannelId};
use crate::domain::video::FetchWindowDays;

const DEFAULT_FETCH_WINDOW_DAYS: i64 = 7;

#[derive(Debug)]
pub enum ConfigError {
    NotFound(PathBuf),
    Read(String),
    Write(String),
    InvalidToml(String),
    InvalidChannel { name: String, reason: String },
    InvalidFetchWindow(String),
    DuplicateChannel { name: String },
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::NotFound(path) => write!(
                f,
                "config file not found at {}\n\nAdd your first channel with:\n\n  \
                 blepo add @ChannelHandle",
                path.display()
            ),
            ConfigError::Read(msg) => write!(f, "cannot read config: {msg}"),
            ConfigError::Write(msg) => write!(f, "cannot write config: {msg}"),
            ConfigError::InvalidToml(msg) => write!(f, "invalid config: {msg}"),
            ConfigError::InvalidChannel { name, reason } => {
                write!(f, "invalid channel \"{name}\": {reason}")
            }
            ConfigError::InvalidFetchWindow(msg) => {
                write!(f, "invalid fetch_window_days: {msg}")
            }
            ConfigError::DuplicateChannel { name } => {
                write!(f, "channel \"{name}\" is already configured")
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

#[derive(Serialize)]
struct ChannelAppend<'a> {
    channels: Vec<ChannelEntryRef<'a>>,
}

#[derive(Serialize)]
struct ChannelEntryRef<'a> {
    name: &'a str,
    id: String,
}

#[derive(Debug)]
pub struct AppConfig {
    pub fetch_window_days: FetchWindowDays,
    pub channels: Vec<Channel>,
    pub data_dir: PathBuf,
}

pub fn load_config() -> Result<AppConfig, ConfigError> {
    let dirs = project_dirs()?;
    let config_path = config_path(&dirs);
    let data_dir = dirs.data_dir().to_path_buf();

    load_config_from_path(&config_path, data_dir)
}

pub fn add_channel(channel: &Channel) -> Result<PathBuf, ConfigError> {
    let dirs = project_dirs()?;
    let path = config_path(&dirs);
    add_channel_to_path(&path, dirs.data_dir(), channel)?;
    Ok(path)
}

fn project_dirs() -> Result<ProjectDirs, ConfigError> {
    ProjectDirs::from("", "", "blepo")
        .ok_or_else(|| ConfigError::Read("cannot determine home directory".to_string()))
}

fn config_path(dirs: &ProjectDirs) -> PathBuf {
    dirs.config_dir().join("config.toml")
}

fn load_config_from_path(config_path: &Path, data_dir: PathBuf) -> Result<AppConfig, ConfigError> {
    if !config_path.exists() {
        return Err(ConfigError::NotFound(config_path.to_path_buf()));
    }

    let content = fs::read_to_string(config_path)
        .map_err(|e| ConfigError::Read(format!("{}: {e}", config_path.display())))?;

    parse_config_str(&content, data_dir)
}

fn add_channel_to_path(
    config_path: &Path,
    data_dir: &Path,
    channel: &Channel,
) -> Result<(), ConfigError> {
    let content = if config_path.exists() {
        fs::read_to_string(config_path)
            .map_err(|error| ConfigError::Read(format!("{}: {error}", config_path.display())))?
    } else {
        String::new()
    };

    let config = parse_config_str(&content, data_dir.to_path_buf())?;
    if config
        .channels
        .iter()
        .any(|existing| existing.id == channel.id)
    {
        return Err(ConfigError::DuplicateChannel {
            name: channel.name.clone(),
        });
    }

    let fragment = toml::to_string(&ChannelAppend {
        channels: vec![ChannelEntryRef {
            name: &channel.name,
            id: channel.id.to_string(),
        }],
    })
    .map_err(|error| ConfigError::Write(error.to_string()))?;

    let mut updated = content;
    if !updated.is_empty() {
        if !updated.ends_with('\n') {
            updated.push('\n');
        }
        updated.push('\n');
    }
    updated.push_str(&fragment);

    parse_config_str(&updated, data_dir.to_path_buf())?;
    write_atomically(config_path, updated.as_bytes())
}

fn write_atomically(path: &Path, content: &[u8]) -> Result<(), ConfigError> {
    let parent = path
        .parent()
        .ok_or_else(|| ConfigError::Write(format!("{} has no parent directory", path.display())))?;
    fs::create_dir_all(parent)
        .map_err(|error| ConfigError::Write(format!("{}: {error}", parent.display())))?;

    let mut temp = NamedTempFile::new_in(parent)
        .map_err(|error| ConfigError::Write(format!("{}: {error}", parent.display())))?;
    temp.write_all(content)
        .map_err(|error| ConfigError::Write(error.to_string()))?;
    temp.as_file()
        .sync_all()
        .map_err(|error| ConfigError::Write(error.to_string()))?;
    temp.persist(path)
        .map_err(|error| ConfigError::Write(format!("{}: {}", path.display(), error.error)))?;

    Ok(())
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

    #[test]
    fn add_channel_creates_config_and_parent_directory() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("nested/config.toml");
        let channel = Channel {
            name: "Fireship".to_string(),
            id: ChannelId::parse("UCsBjURrPoezykLs9EqgamOA").unwrap(),
        };

        add_channel_to_path(&config_path, dir.path(), &channel).unwrap();

        let config = load_config_from_path(&config_path, dir.path().to_path_buf()).unwrap();
        assert_eq!(config.channels.len(), 1);
        assert_eq!(config.channels[0].name, channel.name);
        assert_eq!(config.channels[0].id, channel.id);
    }

    #[test]
    fn add_channel_preserves_existing_content() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("config.toml");
        let original = "# Keep this comment\nfetch_window_days = 14\n";
        std::fs::write(&config_path, original).unwrap();
        let channel = Channel {
            name: "Fireship".to_string(),
            id: ChannelId::parse("UC123").unwrap(),
        };

        add_channel_to_path(&config_path, dir.path(), &channel).unwrap();

        let updated = std::fs::read_to_string(&config_path).unwrap();
        assert!(updated.starts_with(original));
        assert!(updated.contains("[[channels]]"));
        assert!(updated.contains("name = \"Fireship\""));
        assert!(updated.contains("id = \"UC123\""));
    }

    #[test]
    fn add_channel_escapes_name_and_preserves_unicode() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("config.toml");
        let channel = Channel {
            name: "A \"quoted\" 日本語 channel".to_string(),
            id: ChannelId::parse("UC123").unwrap(),
        };

        add_channel_to_path(&config_path, dir.path(), &channel).unwrap();

        let config = load_config_from_path(&config_path, dir.path().to_path_buf()).unwrap();
        assert_eq!(config.channels[0].name, channel.name);
        assert_eq!(config.channels[0].id, channel.id);
    }

    #[test]
    fn add_channel_rejects_duplicate_without_changing_file() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("config.toml");
        let original = "[[channels]]\nname = \"Original name\"\nid = \"UC123\"\n";
        std::fs::write(&config_path, original).unwrap();
        let channel = Channel {
            name: "New name".to_string(),
            id: ChannelId::parse("UC123").unwrap(),
        };

        let result = add_channel_to_path(&config_path, dir.path(), &channel);

        assert!(matches!(result, Err(ConfigError::DuplicateChannel { .. })));
        assert_eq!(std::fs::read_to_string(&config_path).unwrap(), original);
    }

    #[test]
    fn add_channel_rejects_invalid_existing_config_without_changing_it() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("config.toml");
        let original = "not valid TOML [[[";
        std::fs::write(&config_path, original).unwrap();
        let channel = Channel {
            name: "Fireship".to_string(),
            id: ChannelId::parse("UC123").unwrap(),
        };

        let result = add_channel_to_path(&config_path, dir.path(), &channel);

        assert!(matches!(result, Err(ConfigError::InvalidToml(_))));
        assert_eq!(std::fs::read_to_string(&config_path).unwrap(), original);
    }
}
