// SPDX-License-Identifier: GPL-3.0-only

use super::dbus::AsyncDbusManager;
use super::error::{Error, Result};
use super::types::*;
use std::path::Path;
use zbus::zvariant::{ObjectPath, OwnedObjectPath};

// D-Bus service names and paths
const CONFIGURATION_SERVICE: &str = "net.openvpn.v3.configuration";
const SESSION_SERVICE: &str = "net.openvpn.v3.sessions";
const CONFIGURATION_PATH: &str = "/net/openvpn/v3/configuration";
const SESSIONS_PATH: &str = "/net/openvpn/v3/sessions";
const CONFIGURATION_INTERFACE: &str = "net.openvpn.v3.configuration";
const SESSION_INTERFACE: &str = "net.openvpn.v3.sessions";

/// OpenVPN 3 client interface using D-Bus API
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
            Ok(manager) => manager.is_available().await,
            Err(e) => {
                eprintln!("Failed to create D-Bus manager: {}", e);
                false
            }
        }
    }

    /// Get all OpenVPN profiles with their current status
    pub async fn get_profiles(&self) -> Result<Vec<Profile>> {
        // Fetch all configuration paths
        let config_paths = self.fetch_config_paths().await?;

        // Fetch all active sessions
        let sessions = self.fetch_sessions().await.unwrap_or_default();

        // Build profile list with status
        let mut profiles = Vec::new();

        for config_path in config_paths {
            let profile = self.build_profile(&config_path, &sessions).await?;
            profiles.push(profile);
        }

        Ok(profiles)
    }

    /// Fetch all configuration paths from the configuration manager
    async fn fetch_config_paths(&self) -> Result<Vec<OwnedObjectPath>> {
        // Try to fetch configs with retry logic for service activation
        let mut last_error = None;

        for attempt in 1..=3 {
            match self.dbus_manager.connection.call_method(
                Some(CONFIGURATION_SERVICE),
                CONFIGURATION_PATH,
                Some(CONFIGURATION_INTERFACE),
                "FetchAvailableConfigs",
                &(),
            ).await {
                Ok(response) => {
                    let paths: Vec<OwnedObjectPath> = response.body()
                        .deserialize()
                        .map_err(|e| Error::DbusMethod(format!("Failed to parse config paths: {}", e)))?;
                    return Ok(paths);
                }
                Err(e) => {
                    let error_msg = e.to_string();

                    // If the service is not activated yet, wait and retry
                    if error_msg.contains("UnknownMethod") || error_msg.contains("does not exist") {
                        if attempt < 3 {
                            eprintln!("OpenVPN3 service activating, retrying... (attempt {}/3)", attempt);
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

        Err(Error::DbusMethod(format!("Failed to fetch configs after retries: {}",
            last_error.map(|e| e.to_string()).unwrap_or_else(|| "Unknown error".to_string()))))
    }

    /// Fetch all active sessions from the session manager
    pub async fn fetch_sessions(&self) -> Result<Vec<Session>> {
        // Get all session paths by calling FetchAvailableSessions with retry logic
        let mut response_result = None;

        for attempt in 1..=3 {
            match self.dbus_manager.connection.call_method(
                Some(SESSION_SERVICE),
                SESSIONS_PATH,
                Some(SESSION_INTERFACE),
                "FetchAvailableSessions",
                &(),
            ).await {
                Ok(r) => {
                    response_result = Some(r);
                    break;
                }
                Err(e) => {
                    let error_msg = e.to_string();

                    // If the service is not activated yet, wait and retry
                    if error_msg.contains("UnknownMethod") || error_msg.contains("does not exist") {
                        if attempt < 3 {
                            eprintln!("OpenVPN3 sessions service activating, retrying... (attempt {}/3)", attempt);
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

        let session_paths: Vec<OwnedObjectPath> = response.body()
            .deserialize()
            .unwrap_or_default();

        let mut sessions = Vec::new();
        for session_path in session_paths {
            if let Ok(session) = self.fetch_session_info(&session_path).await {
                sessions.push(session);
            }
        }

        Ok(sessions)
    }

    /// Fetch session information from a session path
    async fn fetch_session_info(&self, session_path: &OwnedObjectPath) -> Result<Session> {
        // Get config_name property
        let config_name = self.get_property(
            session_path.as_str(),
            SESSION_INTERFACE,
            "config_name"
        ).await?;

        // Get status property (tuple of StatusMajor, StatusMinor, StatusMessage)
        let status_tuple: (u32, u32, String) = self.get_property(
            session_path.as_str(),
            SESSION_INTERFACE,
            "status"
        ).await?;

        let status = self.parse_status(status_tuple.0, status_tuple.1, &status_tuple.2);

        Ok(Session {
            path: session_path.to_string(),
            profile_name: config_name,
            status,
        })
    }

    /// Parse OpenVPN status codes to ConnectionStatus
    fn parse_status(&self, major: u32, minor: u32, message: &str) -> ConnectionStatus {
        // Log the raw status for debugging
        eprintln!("Status: major={}, minor={}, message=\"{}\"", major, minor, message);

        // Based on OpenVPN 3 status codes from src/dbus/constants.hpp
        // StatusMajor: 0=UNSET, 1=CONFIG, 2=CONNECTION, 3=SESSION, 4=PKCS11, 5=PROCESS
        // StatusMinor for CONNECTION (major=2):
        //   4=CFG_REQUIRE_USER (waiting for credentials)
        //   5=CONN_INIT, 6=CONN_CONNECTING, 7=CONN_CONNECTED
        //   8=CONN_DISCONNECTING, 9=CONN_DISCONNECTED
        //   10=CONN_FAILED, 11=CONN_AUTH_FAILED, 12=CONN_RECONNECTING
        //   13=CONN_PAUSING, 14=CONN_PAUSED, 15=CONN_RESUMING, 16=CONN_DONE
        let status = match (major, minor) {
            // CONNECTION major (2)
            (2, 4) => ConnectionStatus::Connecting,     // CFG_REQUIRE_USER - waiting for credentials
            (2, 7) => ConnectionStatus::Connected,      // CONN_CONNECTED
            (2, 6) | (2, 12) | (2, 15) => ConnectionStatus::Connecting, // CONN_CONNECTING, CONN_RECONNECTING, CONN_RESUMING
            (2, 10) | (2, 11) => ConnectionStatus::Failed, // CONN_FAILED, CONN_AUTH_FAILED
            (2, 9) | (2, 8) | (2, 16) => ConnectionStatus::Disconnected, // CONN_DISCONNECTED, CONN_DISCONNECTING, CONN_DONE

            // SESSION major (3) - treat as connection states
            (3, _) => {
                // Session events, check message
                let msg_lower = message.to_lowercase();
                if msg_lower.contains("failed") || msg_lower.contains("error") {
                    ConnectionStatus::Failed
                } else {
                    ConnectionStatus::Disconnected
                }
            }

            // Other states
            _ => {
                // Check message for additional context
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

    /// Build a Profile from a configuration path and session list
    async fn build_profile(&self, config_path: &OwnedObjectPath, sessions: &[Session]) -> Result<Profile> {
        // Get profile name
        let name: String = self.get_property(
            config_path.as_str(),
            CONFIGURATION_INTERFACE,
            "name"
        ).await?;

        // Get metadata properties
        let metadata = self.fetch_metadata(config_path.as_str()).await?;

        // Determine status from sessions
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

    /// Fetch profile metadata
    async fn fetch_metadata(&self, config_path: &str) -> Result<ProfileMetadata> {

        // Get properties directly from D-Bus
        let valid: bool = self.get_property(
            config_path,
            CONFIGURATION_INTERFACE,
            "valid"
        ).await.unwrap_or(true);

        let import_timestamp: u64 = self.get_property(
            config_path,
            CONFIGURATION_INTERFACE,
            "import_timestamp"
        ).await.unwrap_or(0);

        let last_used_timestamp: u64 = self.get_property(
            config_path,
            CONFIGURATION_INTERFACE,
            "last_used_timestamp"
        ).await.unwrap_or(0);

        let used_count: u32 = self.get_property(
            config_path,
            CONFIGURATION_INTERFACE,
            "used_count"
        ).await.unwrap_or(0);

        // Convert timestamps to readable strings
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

    /// Format a Unix timestamp as a human-readable string
    fn format_timestamp(timestamp: u64) -> String {
        use chrono::{DateTime, Local};

        if let Some(datetime) = DateTime::from_timestamp(timestamp as i64, 0) {
            let local: DateTime<Local> = datetime.into();
            local.format("%Y-%m-%d %H:%M:%S").to_string()
        } else {
            format!("{}", timestamp)
        }
    }

    /// Helper to get a D-Bus property
    async fn get_property<T>(&self, path: &str, interface: &str, property: &str) -> Result<T>
    where
        T: serde::de::DeserializeOwned + zbus::zvariant::Type + TryFrom<zbus::zvariant::OwnedValue>,
        <T as TryFrom<zbus::zvariant::OwnedValue>>::Error: std::fmt::Debug,
    {
        use zbus::zvariant::OwnedValue;

        // Call Get method from org.freedesktop.DBus.Properties interface
        let message = self.dbus_manager.connection.call_method(
            Some(interface),
            path,
            Some("org.freedesktop.DBus.Properties"),
            "Get",
            &(interface, property),
        ).await.map_err(|e| Error::DbusProperty(format!("Failed to get property {}: {}", property, e)))?;

        // D-Bus Properties.Get returns a Variant, deserialize to owned value
        let variant: OwnedValue = message.body()
            .deserialize()
            .map_err(|e| Error::DbusProperty(format!("Failed to parse variant for property {}: {}", property, e)))?;

        // Extract the actual value from the variant
        let value: T = T::try_from(variant)
            .map_err(|e| Error::DbusProperty(format!("Failed to convert property {} from variant: {:?}", property, e)))?;

        Ok(value)
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

        // Read configuration file
        let config_str = std::fs::read_to_string(path)
            .map_err(|e| Error::InvalidInput(format!("Failed to read config file: {}", e)))?;

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

        // Call Import method on configuration manager
        let response = self.dbus_manager.connection.call_method(
            Some(CONFIGURATION_SERVICE),
            CONFIGURATION_PATH,
            Some(CONFIGURATION_INTERFACE),
            "Import",
            &(profile_name.as_str(), config_str.as_str(), false, true),
        ).await.map_err(|e| Error::DbusMethod(format!("Failed to import config: {}", e)))?;

        let _config_path: OwnedObjectPath = response.body()
            .deserialize()
            .map_err(|e| Error::DbusMethod(format!("Failed to parse import response: {}", e)))?;

        Ok(profile_name)
    }

    /// Delete a profile by name
    pub async fn delete_profile(&self, name: &str) -> Result<()> {
        // Find the profile's D-Bus path
        let profile = self.get_profile(name).await?;

        // Call Remove method on the configuration object
        self.dbus_manager.connection.call_method(
            Some(CONFIGURATION_SERVICE),
            profile.path.as_str(),
            Some(CONFIGURATION_INTERFACE),
            "Remove",
            &(),
        ).await.map_err(|e| Error::DbusMethod(format!("Failed to delete profile: {}", e)))?;

        Ok(())
    }

    /// Create a new tunnel session without providing credentials
    /// Returns the session D-Bus path for further operations
    pub async fn create_tunnel(&self, profile_name: &str) -> Result<String> {
        // Find the profile's D-Bus path
        let profile = self.get_profile(profile_name).await?;

        eprintln!("Creating new tunnel for profile: {}", profile_name);

        // Create a new tunnel (session)
        let response = self.dbus_manager.connection.call_method(
            Some(SESSION_SERVICE),
            SESSIONS_PATH,
            Some(SESSION_INTERFACE),
            "NewTunnel",
            &(ObjectPath::try_from(profile.path.as_str()).unwrap(),),
        ).await.map_err(|e| Error::DbusMethod(format!("Failed to create tunnel: {}", e)))?;

        let session_path: OwnedObjectPath = response.body()
            .deserialize()
            .map_err(|e| Error::DbusMethod(format!("Failed to parse session path: {}", e)))?;

        eprintln!("Tunnel created at: {}", session_path);
        Ok(session_path.to_string())
    }

    /// Query what credential inputs are required for a session
    pub async fn query_required_inputs(&self, session_path: &str) -> Result<Vec<CredentialInput>> {
        eprintln!("Querying required inputs for session: {}", session_path);

        let mut inputs = Vec::new();

        // Query what inputs are needed from the session
        let response = self.dbus_manager.connection.call_method(
            Some(SESSION_SERVICE),
            session_path,
            Some(SESSION_INTERFACE),
            "UserInputQueueGetTypeGroup",
            &(),
        ).await.map_err(|e| Error::DbusMethod(format!("Failed to get input queue: {}", e)))?;

        let type_groups: Vec<(u32, u32)> = response.body()
            .deserialize()
            .map_err(|e| Error::DbusMethod(format!("Failed to parse type groups: {}", e)))?;

        eprintln!("Found {} credential type/group pairs", type_groups.len());

        // For each (type, group) pair, fetch required inputs
        for (input_type, input_group) in type_groups {
            eprintln!("Processing input type={}, group={}", input_type, input_group);

            // Check which slots need input for this type/group
            let response = self.dbus_manager.connection.call_method(
                Some(SESSION_SERVICE),
                session_path,
                Some(SESSION_INTERFACE),
                "UserInputQueueCheck",
                &(input_type, input_group),
            ).await.map_err(|e| Error::DbusMethod(format!("Failed to check input queue: {}", e)))?;

            let slot_ids: Vec<u32> = response.body()
                .deserialize()
                .map_err(|e| Error::DbusMethod(format!("Failed to parse slot IDs: {}", e)))?;

            // For each slot, fetch details
            for slot_id in slot_ids {
                let response = self.dbus_manager.connection.call_method(
                    Some(SESSION_SERVICE),
                    session_path,
                    Some(SESSION_INTERFACE),
                    "UserInputQueueFetch",
                    &(input_type, input_group, slot_id),
                ).await.map_err(|e| Error::DbusMethod(format!("Failed to fetch input details: {}", e)))?;

                let input_details: (u32, u32, u32, String, String, bool) = response.body()
                    .deserialize()
                    .map_err(|e| Error::DbusMethod(format!("Failed to parse input details: {}", e)))?;

                let (_, _, id, name, description, hidden) = input_details;

                eprintln!("  Required input: name='{}', description='{}', hidden={}, id={}", name, description, hidden, id);

                // Determine if this can be stored (not OTP/challenge)
                let can_store = !name.to_lowercase().contains("otp")
                    && !name.to_lowercase().contains("challenge")
                    && !name.to_lowercase().contains("token");

                inputs.push(CredentialInput {
                    id,
                    input_type,
                    input_group,
                    name,
                    description,
                    hidden,
                    can_store,
                });
            }
        }

        Ok(inputs)
    }

    /// Provide dynamic credentials to a session
    pub async fn provide_dynamic_credentials(&self, session_path: &str, credentials: &DynamicCredentials) -> Result<()> {
        eprintln!("Providing {} credential values", credentials.values.len());

        // We need to fetch the inputs again to get type/group info for each ID
        let inputs = self.query_required_inputs(session_path).await?;

        for input in inputs {
            if let Some(value) = credentials.values.get(&input.unique_id()) {
                eprintln!("  Providing '{}' for field '{}'", if input.hidden { "***" } else { value }, input.name);

                self.dbus_manager.connection.call_method(
                    Some(SESSION_SERVICE),
                    session_path,
                    Some(SESSION_INTERFACE),
                    "UserInputProvide",
                    &(input.input_type, input.input_group, input.id, value.clone()),
                ).await.map_err(|e| Error::DbusMethod(format!("Failed to provide {}: {}", input.name, e)))?;
            }
        }

        Ok(())
    }

    /// Complete the connection after credentials are provided
    pub async fn connect_session(&self, session_path: &str) -> Result<()> {
        eprintln!("Calling Ready() to signal credentials provided");
        let _: () = self.dbus_manager.connection.call_method(
            Some(SESSION_SERVICE),
            session_path,
            Some(SESSION_INTERFACE),
            "Ready",
            &(),
        ).await
            .and_then(|r| r.body().deserialize())
            .map_err(|e| Error::DbusMethod(format!("Failed to call Ready: {}", e)))?;

        eprintln!("Calling Connect()");
        let _: () = self.dbus_manager.connection.call_method(
            Some(SESSION_SERVICE),
            session_path,
            Some(SESSION_INTERFACE),
            "Connect",
            &(),
        ).await
            .and_then(|r| r.body().deserialize())
            .map_err(|e| Error::DbusMethod(format!("Failed to connect session: {}", e)))?;

        eprintln!("Connection initiated successfully");
        Ok(())
    }

    /// Start a VPN session with authentication (legacy - for backward compatibility)
    pub async fn start_session(&self, profile_name: &str, credentials: &Credentials) -> Result<()> {
        credentials.validate().map_err(Error::InvalidInput)?;

        // Find the profile's D-Bus path
        let profile = self.get_profile(profile_name).await?;

        // Create a new tunnel (session)
        let response = self.dbus_manager.connection.call_method(
            Some(SESSION_SERVICE),
            SESSIONS_PATH,
            Some(SESSION_INTERFACE),
            "NewTunnel",
            &(ObjectPath::try_from(profile.path.as_str()).unwrap(),),
        ).await.map_err(|e| Error::DbusMethod(format!("Failed to create tunnel: {}", e)))?;

        let session_path: OwnedObjectPath = response.body()
            .deserialize()
            .map_err(|e| Error::DbusMethod(format!("Failed to parse session path: {}", e)))?;

        // Provide user credentials via UserInputProvide method
        // This is typically done through the backend client interface
        self.provide_credentials(&session_path, credentials).await?;

        // Start the connection
        let _: () = self.dbus_manager.connection.call_method(
            Some(SESSION_SERVICE),
            session_path.as_str(),
            Some(SESSION_INTERFACE),
            "Connect",
            &(),
        ).await
            .and_then(|r| r.body().deserialize())
            .map_err(|e| Error::DbusMethod(format!("Failed to connect session: {}", e)))?;

        Ok(())
    }

    /// Provide user credentials to a session
    async fn provide_credentials(&self, session_path: &OwnedObjectPath, credentials: &Credentials) -> Result<()> {
        // Query what inputs are needed from the session
        let response = self.dbus_manager.connection.call_method(
            Some(SESSION_SERVICE),
            session_path.as_str(),
            Some(SESSION_INTERFACE),
            "UserInputQueueGetTypeGroup",
            &(),
        ).await.map_err(|e| Error::DbusMethod(format!("Failed to get input queue: {}", e)))?;

        let type_groups: Vec<(u32, u32)> = response.body()
            .deserialize()
            .map_err(|e| Error::DbusMethod(format!("Failed to parse type groups: {}", e)))?;

        // For each (type, group) pair, fetch required inputs and provide values
        for (input_type, input_group) in type_groups {
            // Check which slots need input for this type/group
            let response = self.dbus_manager.connection.call_method(
                Some(SESSION_SERVICE),
                session_path.as_str(),
                Some(SESSION_INTERFACE),
                "UserInputQueueCheck",
                &(input_type, input_group),
            ).await.map_err(|e| Error::DbusMethod(format!("Failed to check input queue: {}", e)))?;

            let slot_ids: Vec<u32> = response.body()
                .deserialize()
                .map_err(|e| Error::DbusMethod(format!("Failed to parse slot IDs: {}", e)))?;

            // For each slot, fetch details and provide appropriate value
            for slot_id in slot_ids {
                let response = self.dbus_manager.connection.call_method(
                    Some(SESSION_SERVICE),
                    session_path.as_str(),
                    Some(SESSION_INTERFACE),
                    "UserInputQueueFetch",
                    &(input_type, input_group, slot_id),
                ).await.map_err(|e| Error::DbusMethod(format!("Failed to fetch input details: {}", e)))?;

                let input_details: (u32, u32, u32, String, String, bool) = response.body()
                    .deserialize()
                    .map_err(|e| Error::DbusMethod(format!("Failed to parse input details: {}", e)))?;

                let (_, _, id, name, _description, _hidden) = input_details;

                // Match the input name to determine which credential to provide
                let value = if name.to_lowercase().contains("user") || name.to_lowercase().contains("name") {
                    Some(&credentials.username)
                } else if name.to_lowercase().contains("pass") {
                    Some(&credentials.password)
                } else if name.to_lowercase().contains("otp") || name.to_lowercase().contains("challenge") {
                    credentials.totp.as_ref()
                } else {
                    None
                };

                if let Some(value) = value {
                    // Provide the input value
                    self.dbus_manager.connection.call_method(
                        Some(SESSION_SERVICE),
                        session_path.as_str(),
                        Some(SESSION_INTERFACE),
                        "UserInputProvide",
                        &(input_type, input_group, id, value.clone()),
                    ).await.map_err(|e| Error::DbusMethod(format!("Failed to provide {}: {}", name, e)))?;
                }
            }
        }

        Ok(())
    }

    /// Disconnect all sessions for a profile
    pub async fn disconnect_profile(&self, profile_name: &str) -> Result<()> {
        // Fetch all sessions and find ones matching this profile
        let sessions = self.fetch_sessions().await?;

        for session in sessions {
            if session.profile_name == profile_name {
                // Call Disconnect on the session
                self.dbus_manager.connection.call_method(
                    Some(SESSION_SERVICE),
                    session.path.as_str(),
                    Some(SESSION_INTERFACE),
                    "Disconnect",
                    &(),
                ).await.map_err(|e| Error::DbusMethod(format!("Failed to disconnect session: {}", e)))?;
            }
        }

        Ok(())
    }

    /// Disconnect a session by its D-Bus path
    pub async fn disconnect_session(&self, session_path: &str) -> Result<()> {
        eprintln!("Disconnecting session at: {}", session_path);

        self.dbus_manager.connection.call_method(
            Some(SESSION_SERVICE),
            session_path,
            Some(SESSION_INTERFACE),
            "Disconnect",
            &(),
        ).await.map_err(|e| Error::DbusMethod(format!("Failed to disconnect session: {}", e)))?;

        eprintln!("Session disconnected successfully");
        Ok(())
    }

    /// Check if a profile requires TOTP
    pub async fn check_totp_required(&self, profile_name: &str) -> Result<bool> {
        // Find the profile
        let profile = self.get_profile(profile_name).await?;

        // Fetch the configuration as JSON
        let response = self.dbus_manager.connection.call_method(
            Some(CONFIGURATION_SERVICE),
            profile.path.as_str(),
            Some(CONFIGURATION_INTERFACE),
            "FetchJSON",
            &(),
        ).await.map_err(|e| Error::DbusMethod(format!("Failed to fetch config JSON: {}", e)))?;

        let json_str: String = response.body()
            .deserialize()
            .map_err(|e| Error::DbusMethod(format!("Failed to parse JSON response: {}", e)))?;

        // Parse and check for TOTP/2FA requirements
        if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&json_str) {
            // Check various fields that might indicate TOTP requirement
            let requires_totp = json_value.get("static-challenge")
                .and_then(|v| v.as_str())
                .map(|s| s.to_lowercase().contains("otp") || s.to_lowercase().contains("2fa"))
                .unwrap_or(false);

            Ok(requires_totp)
        } else {
            Ok(false)
        }
    }

    /// Set up signal monitoring for real-time status updates
    pub async fn setup_signal_monitoring(&self) -> Result<()> {
        // Note: zbus signal handling is typically done through stream subscriptions
        // This is a placeholder for the signal monitoring setup
        // In a real implementation, you would set up signal streams here
        Ok(())
    }

    /// Process incoming D-Bus signals for status updates
    pub async fn process_signals(&self) -> Result<Vec<StatusUpdate>> {
        // Note: This is a placeholder for signal processing
        // In a real implementation, you would process signal streams here
        Ok(Vec::new())
    }
}

impl Clone for OpenVpnClient {
    fn clone(&self) -> Self {
        // Since we can clone the D-Bus connection, we can clone the client
        Self {
            dbus_manager: AsyncDbusManager {
                connection: self.dbus_manager.connection.clone(),
            }
        }
    }
}

impl Default for OpenVpnClient {
    fn default() -> Self {
        // This is a placeholder implementation
        // In practice, you would want to handle the async creation differently
        panic!("OpenVpnClient::default() is not supported. Use OpenVpnClient::new().await instead.")
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
