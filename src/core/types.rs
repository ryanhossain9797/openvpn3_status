// SPDX-License-Identifier: GPL-3.0-only

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Failed,
}

impl ConnectionStatus {
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Connected | Self::Connecting)
    }

    pub fn is_connecting(&self) -> bool {
        matches!(self, Self::Connecting)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,

    pub path: String,

    pub status: ConnectionStatus,

    pub metadata: ProfileMetadata,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileMetadata {
    pub valid: bool,

    pub imported: String,

    #[serde(rename = "lastused")]
    pub last_used: String,

    pub use_count: u32,
}

#[derive(Debug, Clone)]
pub struct Session {
    pub path: String,

    pub profile_name: String,

    pub status: ConnectionStatus,
}

#[derive(Debug, Clone)]
pub struct CredentialInput {
    pub id: u32,
    pub input_type: u32,
    pub input_group: u32,
    pub name: String,
    pub description: String,
    pub hidden: bool,
    pub can_store: bool,
}

impl CredentialInput {
    pub fn unique_id(&self) -> u32 {
        // Combine type, group, and slot into a unique ID
        // Formula: type * 1000000 + group * 1000 + slot
        // This assumes type < 1000, group < 1000, and slot < 1000 (reasonable for OpenVPN)
        self.input_type * 1000000 + self.input_group * 1000 + self.id
    }
}

#[derive(Debug, Clone)]
pub struct DynamicCredentials {
    pub values: HashMap<u32, String>,
}

impl DynamicCredentials {
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    pub fn add(&mut self, id: u32, value: String) {
        self.values.insert(id, value);
    }
}

#[derive(Debug, Clone)]
pub struct Credentials {
    pub username: String,
    pub password: String,
    pub totp: Option<String>,
}

impl Credentials {
    pub fn new(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            password: password.into(),
            totp: None,
        }
    }

    pub fn with_totp(mut self, totp: impl Into<String>) -> Self {
        self.totp = Some(totp.into());
        self
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.username.trim().is_empty() {
            return Err("Username cannot be empty".to_string());
        }
        if self.password.trim().is_empty() {
            return Err("Password cannot be empty".to_string());
        }
        Ok(())
    }

    pub(crate) fn format_for_stdin(&self) -> String {
        if let Some(ref totp) = self.totp {
            format!("{}\n{}\n{}\n", self.username, self.password, totp)
        } else {
            format!("{}\n{}\n", self.username, self.password)
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProfileRequirements {
    pub requires_totp: bool,
}

impl Default for ProfileRequirements {
    fn default() -> Self {
        Self {
            requires_totp: false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum StatusUpdate {
    SessionStatusChange {
        session_path: String,
        status: ConnectionStatus,
    },

    SessionClosed {
        session_path: String,
    },

    ConfigChange {
        config_path: String,
    },
}
