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
}

impl CredentialInput {
    pub fn unique_id(&self) -> u32 {
        self.input_type * 1000000 + self.input_group * 1000 + self.id
    }
}

#[derive(Debug, Clone)]
pub struct Credentials {
    pub values: HashMap<u32, String>,
}

impl Credentials {
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    pub fn add(&mut self, id: u32, value: String) {
        self.values.insert(id, value);
    }
}
