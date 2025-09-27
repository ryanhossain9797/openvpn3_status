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

/// Imports an OpenVPN configuration file
pub fn import_openvpn_config(config_path: &str, custom_name: Option<&str>) -> Result<String, String> {
    // Use custom name if provided and not empty/whitespace, otherwise fall back to filename
    let profile_name = if let Some(name) = custom_name {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            // Fall back to filename if custom name is empty/whitespace
            std::path::Path::new(config_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string()
        } else {
            trimmed.to_string()
        }
    } else {
        // No custom name provided, use filename
        std::path::Path::new(config_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string()
    };

    let output = Command::new("openvpn3")
        .args(&["config-import", "--config", config_path, "--name", &profile_name, "--persistent"])
        .output()
        .map_err(|e| format!("Failed to execute openvpn3 config-import command: {}", e))?;

    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to import config '{}': {}", config_path, error_msg));
    }

    Ok(profile_name)
}

/// Starts an OpenVPN session with authentication
pub fn start_openvpn_session(profile_name: &str, username: &str, password: &str, totp: Option<&str>) -> Result<String, String> {
    let args = vec!["session-start", "--config", profile_name, "--background"];
    
    // Set up input for authentication
    let auth_input = if let Some(totp_code) = totp {
        format!("{}\n{}\n{}\n", username, password, totp_code)
    } else {
        format!("{}\n{}\n", username, password)
    };
    
    let mut child = Command::new("openvpn3")
        .args(&args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start openvpn3 session-start command: {}", e))?;
    
    // Send authentication credentials
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin.write_all(auth_input.as_bytes())
            .map_err(|e| format!("Failed to send authentication: {}", e))?;
    }
    
    let result = child.wait_with_output()
        .map_err(|e| format!("Failed to wait for openvpn3 session-start: {}", e))?;
    
    if !result.status.success() {
        let error_msg = String::from_utf8_lossy(&result.stderr);
        return Err(format!("Failed to start session for profile '{}': {}", profile_name, error_msg));
    }
    
    Ok(format!("Session started for profile '{}'", profile_name))
}

/// Checks if OpenVPN 3 is available on the system
pub fn is_openvpn3_available() -> bool {
    Command::new("openvpn3")
        .arg("version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}
