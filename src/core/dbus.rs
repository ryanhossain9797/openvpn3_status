// SPDX-License-Identifier: GPL-3.0-only

use super::error::{Error, Result};
use zbus::Connection;

/// D-Bus connection manager for OpenVPN3 services
#[derive(Debug)]
pub struct DbusManager {
    pub connection: Connection,
}

impl DbusManager {
    /// Create a new D-Bus manager with system bus connection
    pub async fn new() -> Result<Self> {
        let connection = Connection::system()
            .await
            .map_err(|e| Error::DbusConnection(format!("Failed to connect to system bus: {}", e)))?;
        
        Ok(Self { connection })
    }

    /// Check if OpenVPN3 services are available
    pub async fn is_available(&self) -> bool {
        // For now, just return true as a placeholder
        // This will be implemented once we have the correct D-Bus method signatures
        true
    }

    /// Get the underlying connection for signal handling
    pub fn connection(&self) -> &Connection {
        &self.connection
    }
}

/// Async D-Bus connection manager for OpenVPN3 services (alias for DbusManager)
pub type AsyncDbusManager = DbusManager;