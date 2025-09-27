// SPDX-License-Identifier: GPL-3.0-only

use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenVPNProfile {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenVPNProfileData {
    pub name: String,
    pub valid: bool,
    pub imported: String,
    pub lastused: String,
    pub use_count: u32,
}

type OpenVPNConfigsMap = std::collections::HashMap<String, OpenVPNProfileData>;

/// Fetches the list of OpenVPN 3 profiles from the system
pub fn get_openvpn_profiles() -> Result<Vec<OpenVPNProfile>, String> {
    let output = Command::new("openvpn3")
        .args(&["configs-list", "--json"])
        .output()
        .map_err(|e| format!("Failed to execute openvpn3 command: {}", e))?;

    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        return Err(format!("openvpn3 command failed: {}", error_msg));
    }

    let json_output = String::from_utf8_lossy(&output.stdout);
    
    // Parse the JSON output - it's a map where keys are D-Bus paths and values are profile data
    let configs_map: OpenVPNConfigsMap = serde_json::from_str(&json_output)
        .map_err(|e| format!("Failed to parse JSON: {}", e))?;

    // Convert the map to our profile list
    let profiles: Vec<OpenVPNProfile> = configs_map
        .into_iter()
        .map(|(path, data)| OpenVPNProfile {
            name: data.name,
            path,
        })
        .collect();

    Ok(profiles)
}

/// Checks if a profile has an active session
pub fn is_profile_session_active(profile_name: &str) -> bool {
    let output = Command::new("openvpn3")
        .args(&["sessions-list"])
        .output();
    
    match output {
        Ok(output) => {
            if output.status.success() {
                let sessions_output = String::from_utf8_lossy(&output.stdout);
                // Check if the profile name appears in the sessions list
                sessions_output.contains(profile_name)
            } else {
                false
            }
        }
        Err(_) => false,
    }
}

/// Disconnects an active OpenVPN 3 session by profile name
pub fn disconnect_openvpn_session(profile_name: &str) -> Result<(), String> {
    let output = Command::new("openvpn3")
        .args(&["session-manage", "--disconnect", "--config", profile_name])
        .output()
        .map_err(|e| format!("Failed to execute openvpn3 session-manage command: {}", e))?;

    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to disconnect session for profile '{}': {}", profile_name, error_msg));
    }

    Ok(())
}

/// Deletes an OpenVPN 3 profile by name
pub fn delete_openvpn_profile(profile_name: &str) -> Result<(), String> {
    let output = Command::new("openvpn3")
        .args(&["config-remove", "--config", profile_name, "--force"])
        .output()
        .map_err(|e| format!("Failed to execute openvpn3 config-remove command: {}", e))?;

    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to delete profile '{}': {}", profile_name, error_msg));
    }

    Ok(())
}

/// Checks if OpenVPN 3 is available on the system
pub fn is_openvpn3_available() -> bool {
    Command::new("openvpn3")
        .arg("version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}
