// SPDX-License-Identifier: GPL-3.0-only

use std::fmt;

/// Result type alias for OpenVPN operations
pub type Result<T> = std::result::Result<T, Error>;

/// Error types for OpenVPN operations
#[derive(Debug, Clone)]
pub enum Error {
    /// Failed to execute OpenVPN command
    CommandExecution(String),
    /// Failed to parse JSON output
    JsonParse(String),
    /// Profile not found
    ProfileNotFound(String),
    /// Invalid input
    InvalidInput(String),
    /// D-Bus connection error
    DbusConnection(String),
    /// D-Bus method call error
    DbusMethod(String),
    /// D-Bus property access error
    DbusProperty(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CommandExecution(msg) => write!(f, "Failed to execute command: {}", msg),
            Self::JsonParse(msg) => write!(f, "Failed to parse JSON: {}", msg),
            Self::ProfileNotFound(name) => write!(f, "Profile not found: {}", name),
            Self::InvalidInput(msg) => write!(f, "Invalid input: {}", msg),
            Self::DbusConnection(msg) => write!(f, "D-Bus connection error: {}", msg),
            Self::DbusMethod(msg) => write!(f, "D-Bus method call error: {}", msg),
            Self::DbusProperty(msg) => write!(f, "D-Bus property error: {}", msg),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::CommandExecution(err.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Self::JsonParse(err.to_string())
    }
}

impl From<zbus::Error> for Error {
    fn from(err: zbus::Error) -> Self {
        Self::DbusMethod(err.to_string())
    }
}
