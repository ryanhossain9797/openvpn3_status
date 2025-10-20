// SPDX-License-Identifier: GPL-3.0-only

use super::error::{Error, Result};
use zbus::Connection;

#[derive(Debug)]
pub struct DbusManager {
    pub connection: Connection,
}

impl DbusManager {
    pub async fn new() -> Result<Self> {
        let connection = Connection::system().await.map_err(|e| {
            Error::DbusConnection(format!("Failed to connect to system bus: {}", e))
        })?;

        Ok(Self { connection })
    }

    pub async fn is_service_available(&self, service_name: &str) -> bool {
        match self
            .connection
            .call_method(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                Some("org.freedesktop.DBus"),
                "ListActivatableNames",
                &(),
            )
            .await
        {
            Ok(response) => {
                let services: Vec<String> = response.body().deserialize().unwrap_or_default();
                services.contains(&service_name.to_string())
            }
            Err(e) => {
                eprintln!("Failed to check D-Bus activatable services: {}", e);
                false
            }
        }
    }
}

pub type AsyncDbusManager = DbusManager;
