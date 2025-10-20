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
        // Check if the configuration manager service is activatable
        // OpenVPN 3 services are D-Bus activated (start on-demand), so we check
        // if they're in the list of activatable services rather than currently running
        match self.connection.call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            "ListActivatableNames",
            &(),
        ).await {
            Ok(response) => {
                let services: Vec<String> = response.body().deserialize().unwrap_or_default();
                let is_available = services.contains(&"net.openvpn.v3.configuration".to_string());
                if !is_available {
                    eprintln!("OpenVPN3 service 'net.openvpn.v3.configuration' not found in activatable services");
                }
                is_available
            }
            Err(e) => {
                eprintln!("Failed to check D-Bus activatable services: {}", e);
                false
            }
        }
    }

    /// Get the underlying connection for signal handling
    pub fn connection(&self) -> &Connection {
        &self.connection
    }
}

/// Async D-Bus connection manager for OpenVPN3 services (alias for DbusManager)
pub type AsyncDbusManager = DbusManager;