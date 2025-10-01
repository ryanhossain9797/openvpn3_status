# Development Guide

## Project Overview

This is a **COSMIC applet** called "OpenVPN 3 Status" that provides a system tray interface for monitoring and managing OpenVPN 3 VPN connections. It's built in Rust using the COSMIC framework and integrates with the OpenVPN 3 client.

**Application ID**: `io.github.ryanhossain9797.OpenVPN3Status`

This ID is used throughout the application for desktop integration, including the desktop file, metainfo file, and the COSMIC applet registration.

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

- **`get_profiles()`** - Fetches profiles with session status by combining `openvpn3 configs-list --json` and `openvpn3 sessions-list`
- **`disconnect_profile()`** - Uses `openvpn3 session-manage --disconnect --path <session-path>` to stop sessions
- **`delete_profile()`** - Uses `openvpn3 config-remove --config <name> --force` to remove profiles
- **`import_config()`** - Uses `openvpn3 config-import --config <path> --name <name> --persistent` to add new profiles
- **`start_session()`** - Uses `openvpn3 session-start --config <name> --background` with stdin for authentication
- **`check_totp_required()`** - Checks if a profile requires TOTP using `openvpn3 config-dump --config <name> --json`

**Connection Status Logic:**
- The app parses session output to determine status (Connected, Connecting, Failed, Disconnected)
- Priority-based status: If multiple sessions exist, Connected > Connecting > Failed > Disconnected
- This ensures the profile shows as connected even if there are failed connection attempts

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

The app uses an enum-based state design for type safety:

```rust
enum AppState {
    Unavailable,                              // OpenVPN 3 not installed
    Available { profiles: Vec<Profile>, dialog: DialogState },
}
```

**DialogState** manages UI dialogs:
```rust
enum DialogState {
    Default,                    // No dialog open
    Import(ImportDialog),       // Import config dialog
    Connect(ConnectDialog),     // Connect with credentials dialog
}
```

This design ensures:
- Profiles and dialogs only exist when OpenVPN is available
- Type-safe access to state components
- Clear separation between available and unavailable states

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
    ├── types.rs         # Data structures (Profile, Session, ConnectionStatus, Credentials)
    ├── error.rs         # Error types and handling
    └── localization.rs  # Internationalization setup
```

## Key Data Structures

### Profile
```rust
pub struct Profile {
    pub name: String,
    pub path: String,              // D-Bus path
    pub status: ConnectionStatus,
    pub metadata: ProfileMetadata,
}
```

### ConnectionStatus
```rust
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Failed,
}
```

### Session
```rust
pub struct Session {
    pub path: String,              // D-Bus path
    pub profile_name: String,
    pub status: ConnectionStatus,
}
```

### Credentials
```rust
pub struct Credentials {
    pub username: String,
    pub password: String,
    pub totp: Option<String>,
}
```

## Message System

The app uses a message-based architecture with the following key messages:

**Window Management:**
- `TogglePopup` - Open/close the main popup
- `PopupClosed(Id)` - Handle popup closure

**Profile Operations:**
- `RefreshProfiles` - Reload the profile list
- `ProfilesLoaded(Result<Vec<Profile>, String>)` - Handle loaded profiles
- `DeleteProfile(String)` - Delete a profile by name
- `ProfileDeleted(String, Result<(), String>)` - Handle deletion result
- `DisconnectSession(String)` - Disconnect an active session
- `SessionDisconnected(String, Result<(), String>)` - Handle disconnection result

**Import Dialog:**
- `OpenImportDialog` - Open import dialog
- `CloseImportDialog(Id)` - Close import dialog
- `ImportFilePathChanged(String)` - Update file path input
- `ImportCustomNameChanged(String)` - Update custom name input
- `SubmitImport` - Submit import
- `ConfigImported(Result<String, String>)` - Handle import result

**Connect Dialog:**
- `OpenConnectDialog { profile_name: String, requires_totp: bool }` - Open connect dialog
- `CloseConnectDialog(Id)` - Close connect dialog
- `ConnectUsernameChanged(String)` - Update username input
- `ConnectPasswordChanged(String)` - Update password input
- `ConnectTotpChanged(String)` - Update TOTP input
- `SubmitConnect` - Submit connection
- `SessionStarted(Result<(), String>)` - Handle connection result

**Auto-refresh:**
- `AutoRefresh` - Trigger automatic profile refresh (when connections are in progress)

## OpenVPN 3 Commands Used

| Function | Command | Purpose |
|----------|---------|---------|
| List profiles | `openvpn3 configs-list --json` | Get all available profiles |
| List sessions | `openvpn3 sessions-list` | Check active sessions (parsed as text) |
| Disconnect | `openvpn3 session-manage --disconnect --path <session-path>` | Stop a session by D-Bus path |
| Delete profile | `openvpn3 config-remove --config <name> --force` | Remove a profile |
| Import config | `openvpn3 config-import --config <path> --name <name> --persistent` | Add new profile |
| Start session | `openvpn3 session-start --config <name> --background` | Connect to VPN (credentials via stdin) |
| Check TOTP | `openvpn3 config-dump --config <name> --json` | Check if profile requires TOTP |
| Check available | `openvpn3 version` | Verify OpenVPN 3 is installed |

## Internationalization

The app supports multiple languages through Fluent files in the `i18n/` directory:
- `i18n/en/cosmic_openvpn3_status.ftl` - English translations
- `i18n/nl/cosmic_openvpn3_status.ftl` - Dutch translations

## Desktop Integration

- **Desktop File**: `res/io.github.ryanhossain9797.OpenVPN3Status.desktop`
- **Metainfo**: `res/io.github.ryanhossain9797.OpenVPN3Status.metainfo.xml`
- **Icons**: Custom logo in multiple sizes under `res/icons/`
- **Applet Integration**: Uses `X-CosmicApplet=true` for system tray integration

## Important Implementation Details

### Session Parsing
The `openvpn3 sessions-list` output is parsed as text (not JSON). Sessions are separated by blank lines, with each session containing:
- `Path:` - D-Bus path for the session
- `Config name:` - Profile name
- `Status:` - Connection status string

### Status Detection
Status strings are parsed case-insensitively:
- **Connected**: Contains "client connected", "connected" (without "failed"), or "ready"
- **Connecting**: Contains "connecting", "resolving", "authenticating", or "negotiating"
- **Failed**: Contains "failed" or "authentication failed"
- **Disconnected**: Default for any other status

### Multiple Sessions Per Profile
A profile can have multiple sessions (e.g., failed attempts + one successful connection). The app uses priority-based status selection to show the most relevant status.

### Auto-refresh
When any profile has status `Connecting`, the app automatically refreshes every 3 seconds until no profiles are in connecting state.

## Future Improvements

- Better error handling with user-visible notifications
- More comprehensive session status monitoring
- Profile configuration editing
- Connection statistics and logging
- Enhanced security features
- Additional authentication methods
- Support for certificate-based authentication
- Session reconnection on failure
