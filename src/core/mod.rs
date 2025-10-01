// SPDX-License-Identifier: GPL-3.0-only

pub mod error;
pub mod localization;
pub mod openvpn;
pub mod types;

// Re-export commonly used types
pub use error::Error;
pub use openvpn::OpenVpnClient;
pub use types::{Credentials, Profile};
