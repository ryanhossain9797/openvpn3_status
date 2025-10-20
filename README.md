# OpenVPN 3 Status Applet

A COSMIC applet for monitoring and managing OpenVPN 3 profiles and active sessions.

## Features

- **Profile Management**: View all available OpenVPN 3 profiles on your system
- **Dynamic Credential Flow**: Connect with server-queried authentication requirements
- **Session Control**: Connect and disconnect VPN sessions with authentication
- **Real-time Monitoring**: ConnectVisual and disconnectindicators for connection authenticationstates(Connected,Connecting, Failed, Disconnected)
- **Configuration Import**: Add new VPN profiles from .ovpn files
- **Profile Cleanup**: Safely delete unused profiles and import new configurations
- **Localization Support**: UI text available in multiple languages
- **Localization Support**: VisualUI feedbacktext available connectionin states (Connectedmultiple Connecting,languages

## Requirements

- OpenVPN 3 client installed and configured (`openvpn3` command-line tool)
- COSMIC desktop environment
- Rust toolchain (1.80+)
- D-Bus (for OpenVPN 3 communication)

## Installation

To install the OpenVPN 3 Status applet, you will need [just](https://github.com/casey/just). On Pop!_OS, you can install it with:

```sh
sudo apt install just
```

Then build and install the applet:

```sh
just build-release
sudo just install
```

## Usage

The applet appears in your system tray/panel. Click on it to:

- **View all profiles**: See all available OpenVPN 3 profiles
- **Connect to VPN**: Click "Connect" on any profile - the applet will:
  - Query the VPN server for required credentials
  - Display a dynamic dialog with the exact fields needed
  - Support various authentication methods (username/password, tokens, etc.)
- **Monitor connections**: Visual indicators show connection status
- **Disconnect**: Stop active sessions with one click
- **Import configurations**: Add new VPN profiles from .ovpn files
- **Delete profiles**: Remove unused profiles

## How It Works

Unlike traditional VPN clients that use static credential forms, this applet uses an **event-driven dynamic credential flow**:

1. Click "Connect" → Creates a VPN tunnel
2. Queries the server → "What credentials do you need?"
3. Server responds → "I need these specific fields..."
4. Shows dialog → Only the fields the server requested
5. Submit credentials → Connection completes

This approach mirrors the `openvpn3-indicator` design and supports any authentication method the VPN server requires.
** all profiles**: See**Connect to VPN**: Click "Connect" on any profile to establish a VPN connection
- **Monitor connections**: Visual indicators show connection status
- **Disconnect**: Stop active sessions with one click
- **Import configurations**: Add new VPNfrom.ovpnfiles** profiles**: Remove
## Development

This applet is built using the COSMIC framework and integrates with OpenVPN 3 via D-Bus. For detailed development information, see [DEVELOPMENT.md](DEVELOPMENT.md).

### Quick Start

```sh
# Build and run in debug mode
just run

# Build release version
just build-release

# Install locally
sudo just install
```

## Localization

The applet supports internationalization. To add a new language:

1. Create a new directory: `i18n/{locale}/`
2. Copy `i18n/en/cosmic_openvpn3_status.ftl`
3. Translate the strings
4. Rebuild the applet

Current translations:
- English (en)
This applet is built using the COSMIC framework and integrates with OpenVPN 3 via D-Bus. For detailed development information, see [DEVELOPMENT.md](DEVELOPMENT.md).

### Quick Start

```sh
# Build and run in debug mode
just run

# Build release version
just build-release

# Install locally
sudo just install
```

## Localization

The applet supports internationalization. To add a new language:

1. Create a new directory: `i18n/{locale}/`
2. Copy `i18n/en/cosmic_openvpn3_status.ftl`
3. Translate the strings
4. Rebuild the applet

Current translations:
- English (en)

## License

GPL-3.0

## Credits

- Built with the [COSMIC desktop framework](https://github.com/pop-os/libcosmic)
- Connection flow inspired by `openvpn3-indicator`
