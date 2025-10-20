# Development Guide

## Project Overview

This is a **COSMIC applet** called "OpenVPN 3 Status" that provides a system tray interface for monitoring and managing OpenVPN 3 VPN connections. It's built in Rust using the COSMIC framework and integrates with the OpenVPN 3 client via D-Bus.

**Application ID**: `io.github.ryanhossain9797.OpenVPN3Status`

This ID is used throughout the application for desktop integration, including the desktop file, metainfo file, and the COSMIC applet registration.

## Architecture & Components

### 1. **Main Application Structure**
- **Entry Point**: `src/main.rs` - Simple entry point that runs the COSMIC applet
- **Core App**: `src/app.rs` - Main application logic implementing the COSMIC `Application` trait
- **Core Modules**: `src/core/` - Contains OpenVPN integration, types, and localization

### 2. **Key Features**

**Dynamic Credential Flow:**
- Server-driven authentication: The applet queries the VPN server for required credentials
- Dynamic dialog generation based on server requirements
- Supports any authentication method the server requires
- Inspired by `openvpn3-indicator`'s event-driven approach

**Profile Management:**
- Lists all available OpenVPN 3 profiles on the system
- Shows profile status (Connected, Connecting, Disconnected, Failed)
- Allows importing new VPN configuration files
- Provides profile deletion functionality

**Session Control:**
- Real-time detection of active VPN sessions via D-Bus
- Visual indicators for connection states
- One-click session disconnection
- Automatic refresh during connection attempts

**User Interface:**
- System tray icon that opens a popup when clicked
- Popup shows all profiles with action buttons
- Import dialog for adding new VPN configs
- Dynamic connect dialog that adapts to server requirements
- Waiting dialog during server credential queries

### 3. **OpenVPN 3 Integration** (`src/core/openvpn_dbus.rs`)

The app communicates with OpenVPN 3 through **D-Bus**, not command-line tools:

**Profile Management:**
- **`get_profiles()`** - Fetches profiles via D-Bus ConfigurationManager
- **`delete_profile()`** - Removes profiles via D-Bus
- **`import_config()`** - Imports new profiles via D-Bus

**Dynamic Credential Flow:**
- **`create_tunnel()`** - Creates a new session tunnel (returns session D-Bus path)
- **`query_required_inputs()`** - Queries session for required credential fields
- **`provide_credentials()`** - Provides user credentials to the session
- **`connect_session()`** - Initiates the actual VPN connection

**Session Management:**
- **`fetch_sessions()`** - Gets all active sessions with their status
- **`disconnect_profile()`** - Disconnects all sessions for a profile
- **`disconnect_session()`** - Disconnects a specific session by D-Bus path

**Connection Status Detection:**
- Parses D-Bus session status properties
- Maps OpenVPN status codes to app status (Connected, Connecting, Disconnected, Failed)
- Priority-based status: Connected > Connecting > Failed > Disconnected

### 4. **Dynamic Credential Flow Details**

This is the key differentiator from traditional VPN clients:

1. **User clicks "Connect"** → `Message::StartTunnelCreation(profile_name)`
2. **Create tunnel** → `client.create_tunnel(&profile_name)` returns session path
3. **Show waiting dialog** → "Connecting to {profile}..." with cancel button
4. **Query server** → `client.query_required_inputs(&session_path)` 
5. **Server responds** → Returns list of `CredentialInput` with:
   - `id`: Unique identifier
   - `input_type`, `input_group`: D-Bus classification
   - `name`: Display name (e.g., "Username", "Password", "Auth Token")
   - `description`: Help text
   - `hidden`: Whether to mask input (like passwords)
6. **Build dynamic dialog** → Generate text fields based on server requirements
7. **User fills in** → Only the fields the server actually needs
8. **Submit credentials** → `client.provide_credentials()` + `client.connect_session()`
9. **Connection complete** → Refresh profile list

This approach supports:
- Standard username/password
- TOTP/2FA codes
- Security tokens
- Certificate passphrases
- Any custom authentication method the server defines

### 5. **User Interface Flow**

1. **System Tray**: Shows a display icon in the system tray
   - Empty outline when no connections
   - Filled icon when any profile is connected
2. **Popup Window**: Clicking the icon opens a popup with:
   - Refresh and Import buttons at the top
   - List of all OpenVPN profiles
   - For active sessions: Disconnect button
   - For inactive profiles: Connect and Delete buttons
3. **Dialogs**: 
   - **Import dialog**: For adding new VPN configs
   - **Waiting dialog**: Shows while querying server requirements
   - **Connect dialog**: Dynamic fields based on server requirements

### 6. **Dependencies & Framework**

- **COSMIC Framework**: Uses `libcosmic` for the applet infrastructure
- **D-Bus**: Uses `zbus` for async D-Bus communication
- **Async Runtime**: Uses `tokio` for async operations
- **Internationalization**: Uses `i18n-embed` with Fluent for translations
- **Serialization**: Uses `serde` for data structures

### 7. **Build & Installation**

- **Build System**: Uses `just` (justfile) for build automation
- **Installation**: Installs as a system applet with desktop file and icons
- **Icons**: Custom logo for the applet picker
- **Desktop Integration**: Proper desktop entry for COSMIC applet system

### 8. **State Management**

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
    Default,                                  // No dialog open
    Import(ImportDialog),                     // Import config dialog
    Connect(ConnectDialog),                   // Dynamic connect dialog
    WaitingForCredentialRequirements {        // Waiting for server response
        profile_name: String,
        session_path: String,
    },
}
```

This design ensures:
- Profiles and dialogs only exist when OpenVPN is available
- Type-safe access to state components
- Clear separation between available and unavailable states
- Proper handling of async credential queries

### 9. **Error Handling**

- Graceful handling of OpenVPN 3 unavailability
- D-Bus connection error handling
- Retry logic for service activation (openvpn3 D-Bus service may be activatable)
- User feedback through console logging
- Session cleanup on connection cancellation

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
├── app.rs               # Main application logic and UI (1000+ lines)
└── core/
    ├── mod.rs           # Core module exports
    ├── openvpn_dbus.rs  # D-Bus integration with OpenVPN 3
    ├── dbus.rs          # D-Bus connection manager
    ├── types.rs         # Data structures
    ├── error.rs         # Error types and handling
    └── localization.rs  # Internationalization setup

i18n/
└── en/
    └── cosmic_openvpn3_status.ftl  # English translations

res/
├── icons/               # Applet icons
├── *.desktop            # Desktop entry
└── *.metainfo.xml       # AppStream metadata
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

### CredentialInput (Dynamic)
```rust
pub struct CredentialInput {
    pub id: u32,
    pub input_type: u32,
    pub input_group: u32,
    pub name: String,
    pub description: String,
    pub hidden: bool,
}
```

### Credentials (Dynamic)
```rust
pub struct Credentials {
    pub values: HashMap<u32, String>,  // id -> value mapping
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

**Dynamic Credential Flow:**
- `StartTunnelCreation(String)` - Start creating tunnel for profile
- `TunnelCreated(String, Result<String, String>)` - Handle tunnel creation
- `CheckCredentialRequirements(String, String)` - Query server for requirements
- `CredentialRequirementsFetched(...)` - Handle server response
- `InputChanged(u32, String)` - Update credential input value
- `SubmitCredentials` - Submit credentials and connect
- `CredentialsProvided(Result<(), String>)` - Handle connection result
- `CancelConnect(String)` - Cancel connection and cleanup session
- `SessionCancelled(Result<(), String>)` - Handle cancellation result

**Auto-refresh:**
- `AutoRefresh` - Trigger automatic profile refresh (when connections are in progress)

## D-Bus Interface

OpenVPN 3 D-Bus services used:

| Service | Interface | Purpose |
|---------|-----------|---------|
| `net.openvpn.v3.configuration` | `ConfigurationManager` | List and manage profiles |
| `net.openvpn.v3.sessions` | `SessionManager` | Create and manage sessions |
| Session objects | `Session` | Query requirements, provide credentials, connect |

### Key D-Bus Methods

**Configuration:**
- `FetchAvailableConfigs()` - Get all profiles
- `Import()` - Import new profile
- `Remove()` - Delete profile

**Session Management:**
- `NewTunnel(config_path)` - Create session tunnel
- `UserInputQueueGetTypeGroup()` - Get credential type/group pairs
- `UserInputQueueCheck(type, group)` - Get slot IDs needing input
- `UserInputQueueFetch(type, group, id)` - Get input details
- `UserInputProvide(type, group, id, value)` - Provide credential value
- `Connect()` - Start VPN connection
- `Disconnect()` - Stop VPN connection

## Internationalization

The app supports multiple languages through Fluent files in the `i18n/` directory.

### What is Localized:
- All button labels (Connect, Disconnect, Delete, Import, Cancel, Refresh)
- All dialog titles
- All status messages
- App title
- Profile connection status messages

### What is NOT Localized:
- **Text input placeholders**: Technical limitation with widget API (requires static `&str`)
- **Server-provided field names**: These come from the VPN server and are displayed as-is
- **Log messages**: Debug output stays in English for developers

### Adding a New Language:
1. Create directory: `i18n/{locale}/`
2. Copy `i18n/en/cosmic_openvpn3_status.ftl`
3. Translate all strings
4. Rebuild the applet

### Usage in Code:
```rust
// Simple string
let text = fl!("button-connect");

// String with parameter
let text = fl!("connect-title", name = profile_name.clone());
```

**Important**: The `fl!` macro returns a `String` that must be stored in a variable before passing to widgets to avoid lifetime issues.

## Desktop Integration

- **Desktop File**: `res/io.github.ryanhossain9797.OpenVPN3Status.desktop`
- **Metainfo**: `res/io.github.ryanhossain9797.OpenVPN3Status.metainfo.xml`
- **Icons**: Custom logo in multiple sizes under `res/icons/`
- **Applet Integration**: Uses `X-CosmicApplet=true` for system tray integration

## Important Implementation Details

### D-Bus vs CLI
The app uses D-Bus exclusively (not command-line tools). This provides:
- Better integration with OpenVPN 3's architecture
- Real-time status updates
- Event-driven credential flow
- Lower overhead than spawning processes

### Credential Field Matching
The `unique_id()` method combines type, group, and slot:
```rust
pub fn unique_id(&self) -> u32 {
    self.input_type * 1000000 + self.input_group * 1000 + self.id
}
```

This ensures credentials are matched to the correct fields when submitting.

### Multiple Sessions Per Profile
A profile can have multiple sessions (e.g., failed attempts + one successful connection). The app uses priority-based status selection to show the most relevant status.

### Auto-refresh Logic
When any profile has status `Connecting`, the app automatically refreshes every 3 seconds until no profiles are in connecting state.

### Service Activation
The OpenVPN 3 D-Bus service may not be running initially. The app:
1. Checks if service is available
2. Checks if service is activatable (can be auto-started)
3. Retries connection if service activation is in progress

### Session Cleanup
When user cancels a connection attempt:
1. Dialog closes immediately
2. Background task disconnects the session
3. Profiles refresh after cleanup

## Testing

### Manual Testing Checklist
- [ ] Profile list loads correctly
- [ ] Import new .ovpn configuration
- [ ] Connect to VPN with credentials
- [ ] Dynamic fields appear based on server requirements
- [ ] Cancel connection during setup
- [ ] Disconnect active session
- [ ] Delete profile
- [ ] Icon changes when connected
- [ ] Auto-refresh during connection
- [ ] Handle OpenVPN 3 service not running

### Debug Mode
Run with `RUST_LOG=debug` for detailed logging:
```bash
RUST_LOG=debug cargo run
```

## Common Issues

### "OpenVPN 3 is not available"
- Ensure `openvpn3` package is installed
- Check if `net.openvpn.v3.configuration` service is activatable:
  ```bash
  busctl --system list | grep openvpn
  ```

### Connection hangs at "Waiting for server..."
- The VPN server may not be responding
- Check session status with:
  ```bash
  openvpn3 sessions-list
  ```
- Cancel and try again

### Credentials not accepted
- Verify credentials are correct
- Check if server requires additional fields
- Look at D-Bus logs for error messages

## Future Improvements

- [ ] Connection statistics and bandwidth monitoring
- [ ] Connection history/logs
- [ ] Profile configuration editing
- [ ] Certificate-based authentication support
- [ ] Multiple simultaneous connections
- [ ] Network route management
- [ ] Split tunneling configuration
- [ ] Connection profiles with saved (encrypted) credentials
- [ ] Desktop notifications for connection events
- [ ] More granular error messages in UI

## Contributing

When contributing, please:
1. Follow Rust idioms and formatting (`cargo fmt`)
2. Add localization strings to `.ftl` files
3. Test with multiple VPN server types
4. Ensure D-Bus operations are async
5. Handle errors gracefully
6. Update this documentation

## References

- [COSMIC Framework Documentation](https://pop-os.github.io/libcosmic/)
- [OpenVPN 3 Linux Documentation](https://openvpn.net/openvpn-3-linux/)
- [zbus Documentation](https://docs.rs/zbus/)
- [Fluent Localization](https://projectfluent.org/)
