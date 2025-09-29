# Development Guide

## Project Overview

This is a **COSMIC applet** called "OpenVPN 3 Status" that provides a system tray interface for monitoring and managing OpenVPN 3 VPN connections. It's built in Rust using the COSMIC framework and integrates with the OpenVPN 3 client.

## Architecture & Components

### 1. **Main Application Structure**
- **Entry Point**: `src/main.rs` - Simple entry point that runs the COSMIC applet
- **Core App**: `src/app.rs` - Main application logic implementing the COSMIC `Application` trait
- **Core Modules**: `src/core/` - Contains OpenVPN integration and localization

### 2. **Key Features**

**Profile Management:**
- Lists all available OpenVPN 3 profiles on the system
- Shows profile status (active/inactive sessions)
- Allows importing new VPN configuration files
- Provides profile deletion functionality

**Session Control:**
- Real-time detection of active VPN sessions
- Visual indicators (green dots) for active sessions
- One-click session disconnection
- Session connection with authentication (username/password/TOTP)

**User Interface:**
- System tray icon that opens a popup when clicked
- Popup shows all profiles with action buttons
- Import dialog for adding new VPN configs
- Connect dialog for starting VPN sessions with credentials

### 3. **OpenVPN 3 Integration** (`src/core/openvpn.rs`)

The app communicates with OpenVPN 3 through command-line interface:

- **`get_openvpn_profiles()`** - Uses `openvpn3 configs-list --json` to fetch profiles
- **`is_profile_session_active()`** - Uses `openvpn3 sessions-list` to check active sessions
- **`disconnect_openvpn_session()`** - Uses `openvpn3 session-manage --disconnect` to stop sessions
- **`delete_openvpn_profile()`** - Uses `openvpn3 config-remove` to delete profiles
- **`import_openvpn_config()`** - Uses `openvpn3 config-import` to add new profiles
- **`start_openvpn_session()`** - Uses `openvpn3 session-start` to connect with authentication

### 4. **User Interface Flow**

1. **System Tray**: Shows a display icon in the system tray
2. **Popup Window**: Clicking the icon opens a popup with:
   - Refresh and Import buttons at the top
   - List of all OpenVPN profiles
   - For active sessions: Disconnect button
   - For inactive profiles: Connect and Delete buttons
3. **Dialogs**: 
   - Import dialog for adding new VPN configs
   - Connect dialog for entering credentials (username, password, optional TOTP)

### 5. **Dependencies & Framework**

- **COSMIC Framework**: Uses `libcosmic` for the applet infrastructure
- **Async Runtime**: Uses `tokio` for async operations
- **Internationalization**: Uses `i18n-embed` with Fluent for translations
- **Serialization**: Uses `serde` for JSON parsing of OpenVPN output
- **File Operations**: Uses `open` crate for file operations

### 6. **Build & Installation**

- **Build System**: Uses `just` (justfile) for build automation
- **Installation**: Installs as a system applet with desktop file and icons
- **Icons**: Custom logo for the applet picker
- **Desktop Integration**: Proper desktop entry for COSMIC applet system

### 7. **State Management**

The app maintains state for:
- List of OpenVPN profiles
- OpenVPN 3 availability status
- Dialog states (import/connect dialogs)
- User input for dialogs (file paths, credentials)

### 8. **Error Handling**

- Graceful handling of OpenVPN 3 unavailability
- Error logging for failed operations
- User feedback through console output (could be improved with UI notifications)

## Development Setup

### Prerequisites
- Rust toolchain (version 1.80+)
- OpenVPN 3 client installed and configured
- COSMIC desktop environment
- `just` build tool

### Building
```bash
# Build in release mode
just build-release

# Build in debug mode
just build-debug

# Run with debug logs
just run
```

### Installation
```bash
# Install system-wide
sudo just install

# Uninstall
just uninstall
```

## Code Structure

```
src/
├── main.rs              # Application entry point
├── app.rs               # Main application logic and UI
└── core/
    ├── mod.rs           # Core module exports
    ├── openvpn.rs       # OpenVPN 3 integration
    └── localization.rs  # Internationalization setup
```

## Key Data Structures

### OpenVPNProfile
```rust
pub struct OpenVPNProfile {
    pub name: String,
    pub path: String,
}
```

### Application State
The main application state includes:
- `openvpn_profiles: Vec<OpenVPNProfile>` - List of available profiles
- `openvpn3_available: bool` - Whether OpenVPN 3 is installed
- Dialog states for import/connect operations
- User input fields for dialogs

## Message System

The app uses a message-based architecture with the following key messages:
- `TogglePopup` - Open/close the main popup
- `DeleteProfile(String)` - Delete a profile by name
- `DisconnectSession(String)` - Disconnect an active session
- `RefreshProfiles` - Reload the profile list
- `ImportConfig` - Open import dialog
- `ConnectProfile(String)` - Open connect dialog for a profile

## OpenVPN 3 Commands Used

| Function | Command | Purpose |
|----------|---------|---------|
| List profiles | `openvpn3 configs-list --json` | Get all available profiles |
| List sessions | `openvpn3 sessions-list` | Check active sessions |
| Disconnect | `openvpn3 session-manage --disconnect --config <name>` | Stop a session |
| Delete profile | `openvpn3 config-remove --config <name> --force` | Remove a profile |
| Import config | `openvpn3 config-import --config <path> --name <name> --persistent` | Add new profile |
| Start session | `openvpn3 session-start --config <name> --background` | Connect to VPN |

## Internationalization

The app supports multiple languages through Fluent files in the `i18n/` directory:
- `i18n/en/cosmic_openvpn3_status.ftl` - English translations
- `i18n/nl/cosmic_openvpn3_status.ftl` - Dutch translations

## Desktop Integration

- **Desktop File**: `res/com.system76.OpenVPN3Status.desktop`
- **Metainfo**: `res/com.system76.OpenVPN3Status.metainfo.xml`
- **Icons**: Custom logo in multiple sizes under `res/icons/`
- **Applet Integration**: Uses `X-CosmicApplet=true` for system tray integration

## Future Improvements

- Better error handling with user-visible notifications
- More comprehensive session status monitoring
- Profile configuration editing
- Connection statistics and logging
- Enhanced security features
- Additional authentication methods
