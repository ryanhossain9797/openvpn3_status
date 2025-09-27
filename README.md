# OpenVPN 3 Status Applet

A COSMIC applet for monitoring and managing OpenVPN 3 profiles and active sessions.

## Features

- **Profile Management**: View all available OpenVPN 3 profiles on your system
- **Session Monitoring**: Real-time detection of active VPN sessions with visual indicators
- **Session Control**: Disconnect active sessions with a single click
- **Profile Cleanup**: Safely delete unused profiles
- **Status Indicators**: Green dot for active sessions, delete button for inactive profiles

## Requirements

- OpenVPN 3 client installed and configured
- COSMIC desktop environment
- Rust toolchain

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

- View all available OpenVPN 3 profiles
- See which profiles have active sessions (green dot indicator)
- Disconnect active sessions by clicking the stop button
- Delete unused profiles by clicking the delete button

## Development

This applet is built using the COSMIC framework. For more information on developing COSMIC applets, see the [COSMIC documentation](https://pop-os.github.io/libcosmic/cosmic/).

## License

GPL-3.0
