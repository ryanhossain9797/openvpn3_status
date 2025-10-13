// SPDX-License-Identifier: GPL-3.0-only

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Status of a VPN connection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionStatus {
    /// Not connected
    Disconnected,
    /// Connection in progress
    Connecting,
    /// Successfully connected
    Connected,
    /// Connection failed
    Failed,
}

impl ConnectionStatus {
    /// Check if the status indicates an active connection
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Connected | Self::Connecting)
    }

    /// Check if the status indicates a connecting state
    pub fn is_connecting(&self) -> bool {
        matches!(self, Self::Connecting)
    }
}

/// OpenVPN profile with connection status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    /// Profile name
    pub name: String,
    /// D-Bus path
    pub path: String,
    /// Current connection status
    pub status: ConnectionStatus,
    /// Profile metadata
    pub metadata: ProfileMetadata,
}

impl Profile {
    /// Check if profile has an active session
    pub fn is_active(&self) -> bool {
        self.status.is_active()
    }

    /// Check if profile is connecting
    pub fn is_connecting(&self) -> bool {
        self.status.is_connecting()
    }
}

/// Profile metadata from OpenVPN
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileMetadata {
    /// Whether the profile configuration is valid
    pub valid: bool,
    /// When the profile was imported
    pub imported: String,
    /// When the profile was last used
    #[serde(rename = "lastused")]
    pub last_used: String,
    /// Number of times the profile has been used
    pub use_count: u32,
}

/// Raw profile data from OpenVPN JSON output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RawProfileData {
    pub name: String,
    pub valid: bool,
    pub imported: String,
    #[serde(rename = "lastused")]
    pub last_used: String,
    pub use_count: u32,
}

/// Map of D-Bus paths to profile data
pub(crate) type ProfilesMap = HashMap<String, RawProfileData>;

/// Session information
#[derive(Debug, Clone)]
pub struct Session {
    /// D-Bus path of the session
    pub path: String,
    /// Associated profile name
    pub profile_name: String,
    /// Current status
    pub status: ConnectionStatus,
}

/// A single credential input requirement from the VPN server
#[derive(Debug, Clone)]
pub struct CredentialInput {
    /// Slot ID for this input
    pub id: u32,
    /// Input type (from ClientAttentionType enum)
    pub input_type: u32,
    /// Input group (from ClientAttentionGroup enum)
    pub input_group: u32,
    /// Display name (e.g., "Username", "Password", "Enter OTP")
    pub name: String,
    /// Description text
    pub description: String,
    /// Whether the input should be masked (hidden)
    pub hidden: bool,
    /// Whether this credential can be stored
    pub can_store: bool,
}

impl CredentialInput {
    /// Get unique ID for this input (combining type, group, and slot)
    pub fn unique_id(&self) -> u32 {
        // Combine type, group, and slot into a unique ID
        // Formula: type * 1000000 + group * 1000 + slot
        // This assumes type < 1000, group < 1000, and slot < 1000 (reasonable for OpenVPN)
        self.input_type * 1000000 + self.input_group * 1000 + self.id
    }
}

/// Dynamic credentials - map of input ID to value
#[derive(Debug, Clone)]
pub struct DynamicCredentials {
    /// Map of input field ID to its value
    pub values: HashMap<u32, String>,
}

impl DynamicCredentials {
    /// Create new empty credentials
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    /// Add a credential value
    pub fn add(&mut self, id: u32, value: String) {
        self.values.insert(id, value);
    }
}

/// Authentication credentials for starting a session
#[derive(Debug, Clone)]
pub struct Credentials {
    /// Username
    pub username: String,
    /// Password
    pub password: String,
    /// Optional TOTP code
    pub totp: Option<String>,
}

impl Credentials {
    /// Create new credentials
    pub fn new(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            password: password.into(),
            totp: None,
        }
    }

    /// Add TOTP code to credentials
    pub fn with_totp(mut self, totp: impl Into<String>) -> Self {
        self.totp = Some(totp.into());
        self
    }

    /// Validate credentials (check for empty fields)
    pub fn validate(&self) -> Result<(), String> {
        if self.username.trim().is_empty() {
            return Err("Username cannot be empty".to_string());
        }
        if self.password.trim().is_empty() {
            return Err("Password cannot be empty".to_string());
        }
        Ok(())
    }

    /// Format credentials as input for openvpn3 command
    pub(crate) fn format_for_stdin(&self) -> String {
        if let Some(ref totp) = self.totp {
            format!("{}\n{}\n{}\n", self.username, self.password, totp)
        } else {
            format!("{}\n{}\n", self.username, self.password)
        }
    }
}

/// Configuration for profile requirements
#[derive(Debug, Clone)]
pub struct ProfileRequirements {
    /// Whether TOTP is required
    pub requires_totp: bool,
}

impl Default for ProfileRequirements {
    fn default() -> Self {
        Self {
            requires_totp: false,
        }
    }
}

/// Status update from D-Bus signals
#[derive(Debug, Clone)]
pub enum StatusUpdate {
    /// Session status changed
    SessionStatusChange {
        session_path: String,
        status: ConnectionStatus,
    },
    /// Session was closed
    SessionClosed {
        session_path: String,
    },
    /// Configuration changed
    ConfigChange {
        config_path: String,
    },
}
