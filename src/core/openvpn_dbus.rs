// SPDX-License-Identifier: GPL-3.0-only

use super::dbus::AsyncDbusManager;
use super::error::{Error, Result};
use super::types::*;
use std::path::Path;
use zbus::zvariant::{ObjectPath, OwnedObjectPath};

const CONFIGURATION_SERVICE: &str = "net.openvpn.v3.configuration";
const SESSION_SERVICE: &str = "net.openvpn.v3.sessions";
const CONFIGURATION_PATH: &str = "/net/openvpn/v3/configuration";
const SESSIONS_PATH: &str = "/net/openvpn/v3/sessions";
const CONFIGURATION_INTERFACE: &str = "net.openvpn.v3.configuration";
const SESSION_INTERFACE: &str = "net.openvpn.v3.sessions";

#[derive(Debug)]
pub struct OpenVpnClient {
    dbus_manager: AsyncDbusManager,
}

impl OpenVpnClient {
    pub async fn new() -> Result<Self> {
        let dbus_manager = AsyncDbusManager::new().await?;
        Ok(Self { dbus_manager })
    }

    pub async fn is_available() -> bool {
        match AsyncDbusManager::new().await {
            Ok(manager) => {
                let is_available = manager.is_service_available(CONFIGURATION_SERVICE).await;
                if !is_available {
                    eprintln!(
                        "OpenVPN 3 service '{}' not found in activatable services",
                        CONFIGURATION_SERVICE
                    );
                }
                is_available
            }
            Err(e) => {
                eprintln!("Failed to create D-Bus manager: {}", e);
                false
            }
        }
    }

    pub async fn get_profiles(&self) -> Result<Vec<Profile>> {
        let config_paths = self.fetch_config_paths().await?;
        let sessions = self.fetch_sessions().await.unwrap_or_default();
        let mut profiles = Vec::new();

        for config_path in config_paths {
            let profile = self.build_profile(&config_path, &sessions).await?;
            profiles.push(profile);
        }

        Ok(profiles)
    }

    async fn fetch_config_paths(&self) -> Result<Vec<OwnedObjectPath>> {
        let mut last_error = None;

        for attempt in 1..=3 {
            match self
                .dbus_manager
                .connection
                .call_method(
                    Some(CONFIGURATION_SERVICE),
                    CONFIGURATION_PATH,
                    Some(CONFIGURATION_INTERFACE),
                    "FetchAvailableConfigs",
                    &(),
                )
                .await
            {
                Ok(response) => {
                    let paths: Vec<OwnedObjectPath> =
                        response.body().deserialize().map_err(|e| {
                            Error::DbusMethod(format!("Failed to parse config paths: {}", e))
                        })?;
                    return Ok(paths);
                }
                Err(e) => {
                    let error_msg = e.to_string();

                    if error_msg.contains("UnknownMethod") || error_msg.contains("does not exist") {
                        if attempt < 3 {
                            eprintln!(
                                "OpenVPN3 service activating, retrying... (attempt {}/3)",
                                attempt
                            );
                            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                            last_error = Some(e);
                            continue;
                        }
                    }

                    last_error = Some(e);
                    break;
                }
            }
        }

        Err(Error::DbusMethod(format!(
            "Failed to fetch configs after retries: {}",
            last_error
                .map(|e| e.to_string())
                .unwrap_or_else(|| "Unknown error".to_string())
        )))
    }

    pub async fn fetch_sessions(&self) -> Result<Vec<Session>> {
        let mut response_result = None;

        for attempt in 1..=3 {
            match self
                .dbus_manager
                .connection
                .call_method(
                    Some(SESSION_SERVICE),
                    SESSIONS_PATH,
                    Some(SESSION_INTERFACE),
                    "FetchAvailableSessions",
                    &(),
                )
                .await
            {
                Ok(r) => {
                    response_result = Some(r);
                    break;
                }
                Err(e) => {
                    let error_msg = e.to_string();

                    if error_msg.contains("UnknownMethod") || error_msg.contains("does not exist") {
                        if attempt < 3 {
                            eprintln!(
                                "OpenVPN3 sessions service activating, retrying... (attempt {}/3)",
                                attempt
                            );
                            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                            continue;
                        }
                    }

                    break;
                }
            }
        }

        let response = match response_result {
            Some(r) => r,
            None => {
                eprintln!("No active sessions or session service not available");
                return Ok(Vec::new());
            }
        };

        let session_paths: Vec<OwnedObjectPath> = response.body().deserialize().unwrap_or_default();

        let mut sessions = Vec::new();
        for session_path in session_paths {
            if let Ok(session) = self.fetch_session_info(&session_path).await {
                sessions.push(session);
            }
        }

        Ok(sessions)
    }

    async fn fetch_session_info(&self, session_path: &OwnedObjectPath) -> Result<Session> {
        let config_name = self
            .get_property(session_path.as_str(), SESSION_INTERFACE, "config_name")
            .await?;

        let status_tuple: (u32, u32, String) = self
            .get_property(session_path.as_str(), SESSION_INTERFACE, "status")
            .await?;

        let status = self.parse_status(status_tuple.0, status_tuple.1, &status_tuple.2);

        Ok(Session {
            path: session_path.to_string(),
            profile_name: config_name,
            status,
        })
    }

    fn parse_status(&self, major: u32, minor: u32, message: &str) -> ConnectionStatus {
        eprintln!(
            "Status: major={}, minor={}, message=\"{}\"",
            major, minor, message
        );

        let status = match (major, minor) {
            (2, 4) => ConnectionStatus::Connecting,
            (2, 7) => ConnectionStatus::Connected,
            (2, 6) | (2, 12) | (2, 15) => ConnectionStatus::Connecting,
            (2, 10) | (2, 11) => ConnectionStatus::Failed,
            (2, 9) | (2, 8) | (2, 16) => ConnectionStatus::Disconnected,

            (3, _) => {
                let msg_lower = message.to_lowercase();
                if msg_lower.contains("failed") || msg_lower.contains("error") {
                    ConnectionStatus::Failed
                } else {
                    ConnectionStatus::Disconnected
                }
            }

            _ => {
                let msg_lower = message.to_lowercase();
                if msg_lower.contains("failed") || msg_lower.contains("error") {
                    ConnectionStatus::Failed
                } else if msg_lower.contains("connecting") {
                    ConnectionStatus::Connecting
                } else {
                    ConnectionStatus::Disconnected
                }
            }
        };

        eprintln!("Parsed status: {:?}", status);
        status
    }

    async fn build_profile(
        &self,
        config_path: &OwnedObjectPath,
        sessions: &[Session],
    ) -> Result<Profile> {
        let name: String = self
            .get_property(config_path.as_str(), CONFIGURATION_INTERFACE, "name")
            .await?;

        let metadata = self.fetch_metadata(config_path.as_str()).await?;

        let status = sessions
            .iter()
            .filter(|s| s.profile_name == name)
            .map(|s| s.status)
            .max_by_key(|s| match s {
                ConnectionStatus::Connected => 3,
                ConnectionStatus::Connecting => 2,
                ConnectionStatus::Failed => 1,
                ConnectionStatus::Disconnected => 0,
            })
            .unwrap_or(ConnectionStatus::Disconnected);

        Ok(Profile {
            name,
            path: config_path.to_string(),
            status,
            metadata,
        })
    }

    async fn fetch_metadata(&self, config_path: &str) -> Result<ProfileMetadata> {
        let valid: bool = self
            .get_property(config_path, CONFIGURATION_INTERFACE, "valid")
            .await
            .unwrap_or(true);

        let import_timestamp: u64 = self
            .get_property(config_path, CONFIGURATION_INTERFACE, "import_timestamp")
            .await
            .unwrap_or(0);

        let last_used_timestamp: u64 = self
            .get_property(config_path, CONFIGURATION_INTERFACE, "last_used_timestamp")
            .await
            .unwrap_or(0);

        let used_count: u32 = self
            .get_property(config_path, CONFIGURATION_INTERFACE, "used_count")
            .await
            .unwrap_or(0);

        let imported = if import_timestamp > 0 {
            Self::format_timestamp(import_timestamp)
        } else {
            "unknown".to_string()
        };

        let last_used = if last_used_timestamp > 0 {
            Self::format_timestamp(last_used_timestamp)
        } else {
            "never".to_string()
        };

        Ok(ProfileMetadata {
            valid,
            imported,
            last_used,
            use_count: used_count,
        })
    }

    fn format_timestamp(timestamp: u64) -> String {
        use chrono::{DateTime, Local};

        if let Some(datetime) = DateTime::from_timestamp(timestamp as i64, 0) {
            let local: DateTime<Local> = datetime.into();
            local.format("%Y-%m-%d %H:%M:%S").to_string()
        } else {
            format!("{}", timestamp)
        }
    }

    async fn get_property<T>(&self, path: &str, interface: &str, property: &str) -> Result<T>
    where
        T: serde::de::DeserializeOwned + zbus::zvariant::Type + TryFrom<zbus::zvariant::OwnedValue>,
        <T as TryFrom<zbus::zvariant::OwnedValue>>::Error: std::fmt::Debug,
    {
        use zbus::zvariant::OwnedValue;

        let message = self
            .dbus_manager
            .connection
            .call_method(
                Some(interface),
                path,
                Some("org.freedesktop.DBus.Properties"),
                "Get",
                &(interface, property),
            )
            .await
            .map_err(|e| {
                Error::DbusProperty(format!("Failed to get property {}: {}", property, e))
            })?;

        let variant: OwnedValue = message.body().deserialize().map_err(|e| {
            Error::DbusProperty(format!(
                "Failed to parse variant for property {}: {}",
                property, e
            ))
        })?;

        let value: T = T::try_from(variant).map_err(|e| {
            Error::DbusProperty(format!(
                "Failed to convert property {} from variant: {:?}",
                property, e
            ))
        })?;

        Ok(value)
    }

    pub async fn get_profile(&self, name: &str) -> Result<Profile> {
        let profiles = self.get_profiles().await?;
        profiles
            .into_iter()
            .find(|p| p.name == name)
            .ok_or_else(|| Error::ProfileNotFound(name.to_string()))
    }

    pub async fn import_config(
        &self,
        path: impl AsRef<Path>,
        name: Option<&str>,
    ) -> Result<String> {
        let path = path.as_ref();

        if !path.exists() {
            return Err(Error::InvalidInput(format!(
                "Configuration file does not exist: {}",
                path.display()
            )));
        }

        let config_str = std::fs::read_to_string(path)
            .map_err(|e| Error::InvalidInput(format!("Failed to read config file: {}", e)))?;

        let profile_name = name
            .filter(|n| !n.trim().is_empty())
            .map(|n| n.trim().to_string())
            .or_else(|| {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
            })
            .ok_or_else(|| Error::InvalidInput("Could not determine profile name".to_string()))?;

        let response = self
            .dbus_manager
            .connection
            .call_method(
                Some(CONFIGURATION_SERVICE),
                CONFIGURATION_PATH,
                Some(CONFIGURATION_INTERFACE),
                "Import",
                &(profile_name.as_str(), config_str.as_str(), false, true),
            )
            .await
            .map_err(|e| Error::DbusMethod(format!("Failed to import config: {}", e)))?;

        let _config_path: OwnedObjectPath = response
            .body()
            .deserialize()
            .map_err(|e| Error::DbusMethod(format!("Failed to parse import response: {}", e)))?;

        Ok(profile_name)
    }

    pub async fn delete_profile(&self, name: &str) -> Result<()> {
        let profile = self.get_profile(name).await?;

        self.dbus_manager
            .connection
            .call_method(
                Some(CONFIGURATION_SERVICE),
                profile.path.as_str(),
                Some(CONFIGURATION_INTERFACE),
                "Remove",
                &(),
            )
            .await
            .map_err(|e| Error::DbusMethod(format!("Failed to delete profile: {}", e)))?;

        Ok(())
    }

    pub async fn create_tunnel(&self, profile_name: &str) -> Result<String> {
        let profile = self.get_profile(profile_name).await?;

        eprintln!("Creating new tunnel for profile: {}", profile_name);

        let response = self
            .dbus_manager
            .connection
            .call_method(
                Some(SESSION_SERVICE),
                SESSIONS_PATH,
                Some(SESSION_INTERFACE),
                "NewTunnel",
                &(ObjectPath::try_from(profile.path.as_str()).unwrap(),),
            )
            .await
            .map_err(|e| Error::DbusMethod(format!("Failed to create tunnel: {}", e)))?;

        let session_path: OwnedObjectPath = response
            .body()
            .deserialize()
            .map_err(|e| Error::DbusMethod(format!("Failed to parse session path: {}", e)))?;

        eprintln!("Tunnel created at: {}", session_path);
        Ok(session_path.to_string())
    }

    pub async fn query_required_inputs(&self, session_path: &str) -> Result<Vec<CredentialInput>> {
        eprintln!("Querying required inputs for session: {}", session_path);

        let mut inputs = Vec::new();

        let response = self
            .dbus_manager
            .connection
            .call_method(
                Some(SESSION_SERVICE),
                session_path,
                Some(SESSION_INTERFACE),
                "UserInputQueueGetTypeGroup",
                &(),
            )
            .await
            .map_err(|e| Error::DbusMethod(format!("Failed to get input queue: {}", e)))?;

        let type_groups: Vec<(u32, u32)> = response
            .body()
            .deserialize()
            .map_err(|e| Error::DbusMethod(format!("Failed to parse type groups: {}", e)))?;

        eprintln!("Found {} credential type/group pairs", type_groups.len());

        for (input_type, input_group) in type_groups {
            eprintln!(
                "Processing input type={}, group={}",
                input_type, input_group
            );

            let response = self
                .dbus_manager
                .connection
                .call_method(
                    Some(SESSION_SERVICE),
                    session_path,
                    Some(SESSION_INTERFACE),
                    "UserInputQueueCheck",
                    &(input_type, input_group),
                )
                .await
                .map_err(|e| Error::DbusMethod(format!("Failed to check input queue: {}", e)))?;

            let slot_ids: Vec<u32> = response
                .body()
                .deserialize()
                .map_err(|e| Error::DbusMethod(format!("Failed to parse slot IDs: {}", e)))?;

            for slot_id in slot_ids {
                let response = self
                    .dbus_manager
                    .connection
                    .call_method(
                        Some(SESSION_SERVICE),
                        session_path,
                        Some(SESSION_INTERFACE),
                        "UserInputQueueFetch",
                        &(input_type, input_group, slot_id),
                    )
                    .await
                    .map_err(|e| {
                        Error::DbusMethod(format!("Failed to fetch input details: {}", e))
                    })?;

                let input_details: (u32, u32, u32, String, String, bool) =
                    response.body().deserialize().map_err(|e| {
                        Error::DbusMethod(format!("Failed to parse input details: {}", e))
                    })?;

                let (_, _, id, name, description, hidden) = input_details;

                eprintln!(
                    "  Required input: name='{}', description='{}', hidden={}, id={}",
                    name, description, hidden, id
                );

                inputs.push(CredentialInput {
                    id,
                    input_type,
                    input_group,
                    name,
                    description,
                    hidden,
                });
            }
        }

        Ok(inputs)
    }

    pub async fn provide_credentials(
        &self,
        session_path: &str,
        credentials: &Credentials,
    ) -> Result<()> {
        eprintln!("Providing {} credential values", credentials.values.len());

        let inputs = self.query_required_inputs(session_path).await?;

        for input in inputs {
            if let Some(value) = credentials.values.get(&input.unique_id()) {
                eprintln!(
                    "  Providing '{}' for field '{}'",
                    if input.hidden { "***" } else { value },
                    input.name
                );

                self.dbus_manager
                    .connection
                    .call_method(
                        Some(SESSION_SERVICE),
                        session_path,
                        Some(SESSION_INTERFACE),
                        "UserInputProvide",
                        &(input.input_type, input.input_group, input.id, value.clone()),
                    )
                    .await
                    .map_err(|e| {
                        Error::DbusMethod(format!("Failed to provide {}: {}", input.name, e))
                    })?;
            }
        }

        Ok(())
    }

    pub async fn connect_session(&self, session_path: &str) -> Result<()> {
        eprintln!("Calling Ready() to signal credentials provided");
        let _: () = self
            .dbus_manager
            .connection
            .call_method(
                Some(SESSION_SERVICE),
                session_path,
                Some(SESSION_INTERFACE),
                "Ready",
                &(),
            )
            .await
            .and_then(|r| r.body().deserialize())
            .map_err(|e| Error::DbusMethod(format!("Failed to call Ready: {}", e)))?;

        eprintln!("Calling Connect()");
        let _: () = self
            .dbus_manager
            .connection
            .call_method(
                Some(SESSION_SERVICE),
                session_path,
                Some(SESSION_INTERFACE),
                "Connect",
                &(),
            )
            .await
            .and_then(|r| r.body().deserialize())
            .map_err(|e| Error::DbusMethod(format!("Failed to connect session: {}", e)))?;

        eprintln!("Connection initiated successfully");
        Ok(())
    }

    pub async fn disconnect_profile(&self, profile_name: &str) -> Result<()> {
        let sessions = self.fetch_sessions().await?;

        for session in sessions {
            if session.profile_name == profile_name {
                self.dbus_manager
                    .connection
                    .call_method(
                        Some(SESSION_SERVICE),
                        session.path.as_str(),
                        Some(SESSION_INTERFACE),
                        "Disconnect",
                        &(),
                    )
                    .await
                    .map_err(|e| {
                        Error::DbusMethod(format!("Failed to disconnect session: {}", e))
                    })?;
            }
        }

        Ok(())
    }

    pub async fn disconnect_session(&self, session_path: &str) -> Result<()> {
        eprintln!("Disconnecting session at: {}", session_path);

        self.dbus_manager
            .connection
            .call_method(
                Some(SESSION_SERVICE),
                session_path,
                Some(SESSION_INTERFACE),
                "Disconnect",
                &(),
            )
            .await
            .map_err(|e| Error::DbusMethod(format!("Failed to disconnect session: {}", e)))?;

        eprintln!("Session disconnected successfully");
        Ok(())
    }
}

impl Clone for OpenVpnClient {
    fn clone(&self) -> Self {
        Self {
            dbus_manager: AsyncDbusManager {
                connection: self.dbus_manager.connection.clone(),
            },
        }
    }
}

impl Default for OpenVpnClient {
    fn default() -> Self {
        panic!("OpenVpnClient::default() is not supported. Use OpenVpnClient::new().await instead.")
    }
}
