// Silence Crypto - Ephemeral Key Cascade P2P Communication Library

pub mod crypto;
pub mod network;

pub use crypto::*;
pub use network::*;

/// Connection mode for P2P communication
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum ConnectionMode {
    /// Try direct P2P first, fallback to relay
    Auto,
    /// Direct P2P connection only
    DirectOnly,
    /// Relay connection only
    RelayOnly,
}

/// Application configuration
#[derive(Clone)]
pub struct Config {
    pub listen_port: u16,
    pub key_rotation_interval: u64,
    pub max_message_size: usize,
    pub connection_timeout: u64,
    pub relay_servers: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen_port: 7642,
            key_rotation_interval: 15, // seconds
            max_message_size: 4096,    // 4KB
            connection_timeout: 30,    // seconds
            relay_servers: vec![
                "185.191.116.220:8080".to_string(),
            ],
        }
    }
}