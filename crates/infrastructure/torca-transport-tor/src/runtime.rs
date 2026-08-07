use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

use crate::{OnionServiceConfig, TorProcessConfig};

pub const TOR_SOCKS_PORT: u16 = 19050;
pub const TOR_CONTROL_PORT: u16 = 19051;
pub const TOR_PEER_VIRTUAL_PORT: u16 = 17491;

/// Filesystem/process parameters for one Torca-owned Tor instance.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TorRuntimeConfig {
    pub process: TorProcessConfig,
    pub peer_listener: SocketAddr,
}

impl TorRuntimeConfig {
    /// Builds the complete owned Tor layout below one platform-specific private state root.
    pub fn new(
        executable: impl Into<PathBuf>,
        state_root: impl Into<PathBuf>,
        peer_listener: SocketAddr,
    ) -> Self {
        let state_root = state_root.into();
        let tor_root = state_root.join("tor");
        let loopback = IpAddr::V4(Ipv4Addr::LOCALHOST);
        Self {
            process: TorProcessConfig {
                executable: executable.into(),
                data_directory: tor_root.join("data"),
                torrc_path: tor_root.join("torrc"),
                socks_address: SocketAddr::new(loopback, TOR_SOCKS_PORT),
                control_port: TOR_CONTROL_PORT,
                onion_service: OnionServiceConfig {
                    directory: tor_root.join("onion-service"),
                    virtual_port: TOR_PEER_VIRTUAL_PORT,
                    target: peer_listener,
                },
            },
            peer_listener,
        }
    }
}
