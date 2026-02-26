use std::process::{Command, Stdio};

use crate::application::ports::{PlayError, VideoPlayer};

pub struct MpvPlayer;

impl MpvPlayer {
    pub fn new() -> Result<Self, PlayError> {
        check_dependency("mpv")?;
        check_dependency("yt-dlp")?;
        Ok(Self)
    }
}

impl VideoPlayer for MpvPlayer {
    fn play(&self, url: &str) -> Result<(), PlayError> {
        Command::new("mpv")
            .arg(url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| PlayError::PlayerFailed(format!("failed to launch mpv: {e}")))?;

        Ok(())
    }
}

pub(crate) fn check_dependency(name: &str) -> Result<(), PlayError> {
    Command::new("which")
        .arg(name)
        .output()
        .map_err(|e| PlayError::PlayerFailed(format!("cannot check for {name}: {e}")))
        .and_then(|output| {
            if output.status.success() {
                Ok(())
            } else {
                Err(PlayError::PlayerFailed(format!(
                    "{name} is not installed. Install it with: brew install {name}"
                )))
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_dependency_finds_existing_binary() {
        // "sh" exists on every Unix system
        assert!(check_dependency("sh").is_ok());
    }

    #[test]
    fn check_dependency_rejects_missing_binary() {
        let result = check_dependency("nonexistent_binary_xyz_123");

        assert!(result.is_err());
    }

    #[test]
    fn check_dependency_error_includes_binary_name() {
        let result = check_dependency("nonexistent_binary_xyz_123");

        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("nonexistent_binary_xyz_123"),
            "error should include binary name, got: {err}"
        );
    }
}
