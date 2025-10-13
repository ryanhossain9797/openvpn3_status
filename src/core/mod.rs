// SPDX-License-Identifier: GPL-3.0-only

pub mod dbus;
pub mod error;
pub mod localization;
pub mod openvpn_dbus;
pub mod types;

// Re-export commonly used types
pub use error::Error;
pub use openvpn_dbus::OpenVpnClient;
pub use types::{Credentials, Profile, StatusUpdate};
