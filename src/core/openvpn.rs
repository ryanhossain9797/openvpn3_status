// SPDX-License-Identifier: GPL-3.0-only

use super::error::{Error, Result};
use super::types::*;
use std::path::Path;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

/// OpenVPN 3 client interface
#[derive(Debug, Clone, Copy, Default)]
pub struct OpenVpnClient;

impl OpenVpnClient {
    pub fn new() -> Self {
        Self
    }

    pub async fn is_available() -> bool {
        Command::new("openvpn3")
            .arg("version")
            .output()
            .await
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    /// Get all OpenVPN profiles with their current status
    pub async fn get_profiles(&self) -> Result<Vec<Profile>> {
        let profiles_map = self.fetch_profiles_map().await?;
        let sessions = self.fetch_sessions().await.unwrap_or_default();

        let profiles = profiles_map
            .into_iter()
            .map(|(path, data)| {
                // Find the best status among all sessions for this profile
                // Priority: Connected > Connecting > Failed > Disconnected
                let status = sessions
                    .iter()
                    .filter(|s| s.profile_name == data.name)
                    .map(|s| s.status)
                    .max_by_key(|s| match s {
                        ConnectionStatus::Connected => 3,
                        ConnectionStatus::Connecting => 2,
                        ConnectionStatus::Failed => 1,
                        ConnectionStatus::Disconnected => 0,
                    })
                    .unwrap_or(ConnectionStatus::Disconnected);

                Profile {
                    name: data.name,
                    path,
                    status,
                    metadata: ProfileMetadata {
                        valid: data.valid,
                        imported: data.imported,
                        last_used: data.last_used,
                        use_count: data.use_count,
                    },
                }
            })
            .collect();

        Ok(profiles)
    }

    /// Get a specific profile by name
    pub async fn get_profile(&self, name: &str) -> Result<Profile> {
        let profiles = self.get_profiles().await?;
        profiles
            .into_iter()
            .find(|p| p.name == name)
            .ok_or_else(|| Error::ProfileNotFound(name.to_string()))
    }

    /// Import a new VPN configuration file
    pub async fn import_config(&self, path: impl AsRef<Path>, name: Option<&str>) -> Result<String> {
        let path = path.as_ref();

        // Validate file exists
        if !path.exists() {
            return Err(Error::InvalidInput(format!(
                "Configuration file does not exist: {}",
                path.display()
            )));
        }

        // Determine profile name
        let profile_name = name
            .filter(|n| !n.trim().is_empty())
            .map(|n| n.trim().to_string())
            .or_else(|| {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
            })
            .ok_or_else(|| Error::InvalidInput("Could not determine profile name".to_string()))?;

        let output = Command::new("openvpn3")
            .args(&[
                "config-import",
                "--config",
                &path.display().to_string(),
                "--name",
                &profile_name,
                "--persistent",
            ])
            .output()
            .await?;

        if !output.status.success() {
            return Err(Error::CommandFailed {
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            });
        }

        Ok(profile_name)
    }

    /// Delete a profile by name
    pub async fn delete_profile(&self, name: &str) -> Result<()> {
        let output = Command::new("openvpn3")
            .args(&["config-remove", "--config", name, "--force"])
            .output()
            .await?;

        if !output.status.success() {
            return Err(Error::CommandFailed {
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            });
        }

        Ok(())
    }

    /// Start a VPN session with authentication
    pub async fn start_session(&self, profile_name: &str, credentials: &Credentials) -> Result<()> {
        credentials.validate().map_err(Error::InvalidInput)?;

        let mut child = Command::new("openvpn3")
            .args(&["session-start", "--config", profile_name, "--background"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        // Send authentication credentials
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(credentials.format_for_stdin().as_bytes())
                .await
                .map_err(|e| Error::CommandExecution(format!("Failed to send credentials: {}", e)))?;
            stdin.flush().await.ok();
        }

        let output = child.wait_with_output().await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("authentication") || stderr.contains("auth") {
                return Err(Error::AuthenticationFailed(stderr.to_string()));
            }
            return Err(Error::CommandFailed {
                stderr: stderr.to_string(),
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            });
        }

        Ok(())
    }

    /// Disconnect all sessions for a profile
    pub async fn disconnect_profile(&self, profile_name: &str) -> Result<()> {
        let sessions = self.fetch_sessions().await?;
        let profile_sessions: Vec<_> = sessions
            .iter()
            .filter(|s| s.profile_name == profile_name)
            .collect();

        if profile_sessions.is_empty() {
            return Err(Error::SessionNotFound(profile_name.to_string()));
        }

        let mut errors = Vec::new();
        let mut success_count = 0;

        for session in profile_sessions {
            match self.disconnect_session(&session.path).await {
                Ok(_) => success_count += 1,
                Err(e) => errors.push(format!("{}: {}", session.path, e)),
            }
        }

        if success_count == 0 {
            return Err(Error::CommandFailed {
                stderr: format!("Failed to disconnect any sessions: {}", errors.join("; ")),
                stdout: String::new(),
            });
        }

        if !errors.is_empty() {
            eprintln!("Warning: Some sessions failed to disconnect: {}", errors.join("; "));
        }

        Ok(())
    }

    /// Check if a profile requires TOTP
    pub async fn check_totp_required(&self, profile_name: &str) -> Result<bool> {
        let output = Command::new("openvpn3")
            .args(&["config-dump", "--config", profile_name, "--json"])
            .output()
            .await?;

        if !output.status.success() {
            return Ok(false); // Default to false if we can't check
        }

        let config_json = String::from_utf8_lossy(&output.stdout);
        Ok(config_json.contains("\"static-challenge\""))
    }

    // Private helper methods

    /// Fetch raw profiles map from OpenVPN
    async fn fetch_profiles_map(&self) -> Result<ProfilesMap> {
        let output = Command::new("openvpn3")
            .args(&["configs-list", "--json"])
            .output()
            .await?;

        if !output.status.success() {
            return Err(Error::CommandFailed {
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            });
        }

        let json_output = String::from_utf8_lossy(&output.stdout);
        let profiles_map: ProfilesMap = serde_json::from_str(&json_output)?;

        Ok(profiles_map)
    }

    /// Fetch all active sessions
    async fn fetch_sessions(&self) -> Result<Vec<Session>> {
        let output = Command::new("openvpn3")
            .args(&["sessions-list"])
            .output()
            .await?;

        if !output.status.success() {
            return Ok(Vec::new()); // No sessions is not an error
        }

        let sessions_text = String::from_utf8_lossy(&output.stdout);
        Ok(parser::parse_sessions(&sessions_text))
    }

    /// Disconnect a specific session by path
    async fn disconnect_session(&self, session_path: &str) -> Result<()> {
        let output = Command::new("openvpn3")
            .args(&["session-manage", "--disconnect", "--path", session_path])
            .output()
            .await?;

        if !output.status.success() {
            return Err(Error::CommandFailed {
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            });
        }

        Ok(())
    }
}


/// Parser module for OpenVPN text output
mod parser {
    use super::*;

    /// Parse sessions from sessions-list output
    pub fn parse_sessions(output: &str) -> Vec<Session> {
        parse_all_sessions(output)
    }

    /// Parse all sessions from output - sessions are separated by blank lines
    fn parse_all_sessions(output: &str) -> Vec<Session> {
        let mut sessions = Vec::new();
        let mut current_path = None;
        let mut current_profile = None;
        let mut current_status = None;

        for line in output.lines() {
            let trimmed = line.trim();

            // Skip delimiter lines
            if trimmed.starts_with("---") {
                continue;
            }

            // Empty line indicates end of a session block
            if trimmed.is_empty() {
                if let (Some(path), Some(profile_name), Some(status)) =
                    (current_path.take(), current_profile.take(), current_status.take()) {
                    sessions.push(Session {
                        path,
                        profile_name,
                        status,
                    });
                }
                continue;
            }

            // Parse fields
            if let Some(value) = trimmed.strip_prefix("Path:") {
                current_path = Some(value.trim().to_string());
            } else if let Some(value) = trimmed.strip_prefix("Config name:") {
                current_profile = Some(value.trim().to_string());
            } else if let Some(value) = trimmed.strip_prefix("Status:") {
                current_status = Some(parse_status(value.trim()));
            }
        }

        // Don't forget the last session if the file doesn't end with a blank line
        if let (Some(path), Some(profile_name), Some(status)) =
            (current_path, current_profile, current_status) {
            sessions.push(Session {
                path,
                profile_name,
                status,
            });
        }

        sessions
    }

    /// Parse status string into ConnectionStatus
    fn parse_status(status: &str) -> ConnectionStatus {
        let lower = status.to_lowercase();

        if lower.contains("client connected") {
            ConnectionStatus::Connected
        } else if lower.contains("connected") && !lower.contains("failed") {
            ConnectionStatus::Connected
        } else if lower.contains("ready") {
            ConnectionStatus::Connected
        } else if lower.contains("connecting")
            || lower.contains("resolving")
            || lower.contains("authenticating")
            || lower.contains("negotiating")
        {
            ConnectionStatus::Connecting
        } else if lower.contains("failed") || lower.contains("authentication failed") {
            ConnectionStatus::Failed
        } else {
            ConnectionStatus::Disconnected
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credentials_validation() {
        let valid = Credentials::new("user", "pass");
        assert!(valid.validate().is_ok());

        let empty_user = Credentials::new("", "pass");
        assert!(empty_user.validate().is_err());

        let empty_pass = Credentials::new("user", "");
        assert!(empty_pass.validate().is_err());
    }

    #[test]
    fn test_credentials_format() {
        let basic = Credentials::new("user", "pass");
        assert_eq!(basic.format_for_stdin(), "user\npass\n");

        let with_totp = Credentials::new("user", "pass").with_totp("123456");
        assert_eq!(with_totp.format_for_stdin(), "user\npass\n123456\n");
    }

    #[test]
    fn test_connection_status() {
        assert!(ConnectionStatus::Connected.is_active());
        assert!(ConnectionStatus::Connecting.is_active());
        assert!(!ConnectionStatus::Disconnected.is_active());
        assert!(!ConnectionStatus::Failed.is_active());

        assert!(ConnectionStatus::Connecting.is_connecting());
        assert!(!ConnectionStatus::Connected.is_connecting());
    }
}
