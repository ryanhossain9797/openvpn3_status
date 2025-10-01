// SPDX-License-Identifier: GPL-3.0-only

use std::fmt;

/// Result type alias for OpenVPN operations
pub type Result<T> = std::result::Result<T, Error>;

/// Error types for OpenVPN operations
#[derive(Debug, Clone)]
pub enum Error {
    /// Failed to execute OpenVPN command
    CommandExecution(String),
    /// OpenVPN command returned non-zero exit code
    CommandFailed { stderr: String, stdout: String },
    /// Failed to parse JSON output
    JsonParse(String),
    /// Failed to parse text output
    ParseError(String),
    /// Profile not found
    ProfileNotFound(String),
    /// Session not found
    SessionNotFound(String),
    /// OpenVPN 3 is not installed or not available
    NotAvailable,
    /// Authentication failed
    AuthenticationFailed(String),
    /// Invalid input
    InvalidInput(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CommandExecution(msg) => write!(f, "Failed to execute command: {}", msg),
            Self::CommandFailed { stderr, .. } => write!(f, "Command failed: {}", stderr),
            Self::JsonParse(msg) => write!(f, "Failed to parse JSON: {}", msg),
            Self::ParseError(msg) => write!(f, "Failed to parse output: {}", msg),
            Self::ProfileNotFound(name) => write!(f, "Profile not found: {}", name),
            Self::SessionNotFound(name) => write!(f, "No active session found for: {}", name),
            Self::NotAvailable => write!(f, "OpenVPN 3 is not available"),
            Self::AuthenticationFailed(msg) => write!(f, "Authentication failed: {}", msg),
            Self::InvalidInput(msg) => write!(f, "Invalid input: {}", msg),
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
