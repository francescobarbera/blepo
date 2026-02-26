use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChannelId(String);

#[derive(Debug, PartialEq, Eq)]
pub enum ChannelIdError {
    Empty,
    InvalidPrefix,
}

impl std::fmt::Display for ChannelIdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChannelIdError::Empty => write!(f, "channel ID cannot be empty"),
            ChannelIdError::InvalidPrefix => {
                write!(f, "channel ID must start with 'UC'")
            }
        }
    }
}

impl std::error::Error for ChannelIdError {}

impl ChannelId {
    pub fn parse(id: impl Into<String>) -> Result<Self, ChannelIdError> {
        let id = id.into();
        if id.is_empty() {
            return Err(ChannelIdError::Empty);
        }
        if !id.starts_with("UC") {
            return Err(ChannelIdError::InvalidPrefix);
        }
        Ok(Self(id))
    }
}

impl std::fmt::Display for ChannelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    pub name: String,
    pub id: ChannelId,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_channel_id() {
        let id = ChannelId::parse("UC_x5XG1OV2P6uZZ5FSM9Ttw").unwrap();
        assert_eq!(id.to_string(), "UC_x5XG1OV2P6uZZ5FSM9Ttw");
    }

    #[test]
    fn rejects_empty_channel_id() {
        assert_eq!(ChannelId::parse(""), Err(ChannelIdError::Empty));
    }

    #[test]
    fn rejects_channel_id_without_uc_prefix() {
        assert_eq!(
            ChannelId::parse("notavalidid"),
            Err(ChannelIdError::InvalidPrefix)
        );
    }

    #[test]
    fn channel_id_equality() {
        let a = ChannelId::parse("UC123").unwrap();
        let b = ChannelId::parse("UC123").unwrap();
        let c = ChannelId::parse("UC456").unwrap();
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
