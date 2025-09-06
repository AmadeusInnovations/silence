// Silence Crypto - Ephemeral Key Cascade P2P Communication Library

pub mod crypto;
pub mod network;

pub use crypto::*;
pub use network::*;

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
                // Add your Cherry Server relay endpoints here
                // Example: "your-server.cherryservers.net:8080".to_string()
            ],
        }
    }
}