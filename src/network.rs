// P2P networking layer for Silence Crypto
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::crypto::{SilenceCrypto, EncryptedMessage, CryptoError};

/// Network errors
#[derive(Debug)]
pub enum NetworkError {
    Connection(std::io::Error),
    Serialization(bincode::Error),
    Crypto(CryptoError),
    InvalidMessage,
    MessageTooLarge,
    Timeout,
}

impl std::fmt::Display for NetworkError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            NetworkError::Connection(e) => write!(f, "Connection error: {}", e),
            NetworkError::Serialization(e) => write!(f, "Serialization error: {}", e),
            NetworkError::Crypto(e) => write!(f, "Crypto error: {}", e),
            NetworkError::InvalidMessage => write!(f, "Invalid message format"),
            NetworkError::MessageTooLarge => write!(f, "Message too large"),
            NetworkError::Timeout => write!(f, "Operation timeout"),
        }
    }
}

impl std::error::Error for NetworkError {}

impl From<std::io::Error> for NetworkError {
    fn from(err: std::io::Error) -> Self {
        NetworkError::Connection(err)
    }
}

impl From<bincode::Error> for NetworkError {
    fn from(err: bincode::Error) -> Self {
        NetworkError::Serialization(err)
    }
}

impl From<CryptoError> for NetworkError {
    fn from(err: CryptoError) -> Self {
        NetworkError::Crypto(err)
    }
}

/// Network message format
#[derive(Serialize, Deserialize, Clone)]
pub struct NetworkMessage {
    pub id: String,
    pub message_type: MessageType,
    pub encrypted_data: EncryptedMessage,
}

#[derive(Serialize, Deserialize, Clone)]
pub enum MessageType {
    Text,
    KeyRotation,
    Heartbeat,
}

/// P2P connection handler
pub struct P2PConnection {
    stream: TcpStream,
    peer_addr: SocketAddr,
    crypto: Arc<Mutex<SilenceCrypto>>,
    max_message_size: usize,
    is_relay: bool,
}

impl P2PConnection {
    /// Create new P2P connection
    pub async fn new(
        stream: TcpStream,
        peer_addr: SocketAddr,
        crypto: Arc<Mutex<SilenceCrypto>>,
        max_message_size: usize,
        is_relay: bool,
    ) -> Self {
        Self {
            stream,
            peer_addr,
            crypto,
            max_message_size,
            is_relay,
        }
    }
    
    /// Connect to a peer
    pub async fn connect(
        addr: SocketAddr,
        crypto: Arc<Mutex<SilenceCrypto>>,
        max_message_size: usize,
        is_relay: bool,
    ) -> Result<Self, NetworkError> {
        let stream = TcpStream::connect(addr).await?;
        Ok(Self::new(stream, addr, crypto, max_message_size, is_relay).await)
    }
    
    /// Send a text message
    pub async fn send_text(&mut self, content: &str) -> Result<(), NetworkError> {
        let message = NetworkMessage {
            id: uuid::Uuid::new_v4().to_string(),
            message_type: MessageType::Text,
            encrypted_data: {
                let mut crypto = self.crypto.lock().await;
                crypto.encrypt(content.as_bytes())?
            },
        };
        
        self.send_message(&message).await
    }
    
    /// Send a network message
    async fn send_message(&mut self, message: &NetworkMessage) -> Result<(), NetworkError> {
        if self.is_relay {
            // For relay connections, send serialized encrypted message
            let data = bincode::serialize(&message.encrypted_data)?;
            
            if data.len() > self.max_message_size {
                return Err(NetworkError::MessageTooLarge);
            }
            
            // Send length prefix (4 bytes) followed by serialized encrypted data
            let length = data.len() as u32;
            self.stream.write_u32(length).await?;
            self.stream.write_all(&data).await?;
            self.stream.flush().await?;
        } else {
            // For direct P2P connections, send full NetworkMessage
            let serialized = bincode::serialize(message)?;
            
            if serialized.len() > self.max_message_size {
                return Err(NetworkError::MessageTooLarge);
            }
            
            // Send length prefix (4 bytes) followed by message
            let length = serialized.len() as u32;
            self.stream.write_u32(length).await?;
            self.stream.write_all(&serialized).await?;
            self.stream.flush().await?;
        }
        
        Ok(())
    }
    
    /// Receive a network message
    pub async fn receive_message(&mut self) -> Result<Option<String>, NetworkError> {
        // Read length prefix
        let length = match self.stream.read_u32().await {
            Ok(len) => len as usize,
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(NetworkError::Connection(e)),
        };
        
        if length > self.max_message_size {
            return Err(NetworkError::MessageTooLarge);
        }
        
        // Read message data
        let mut buffer = vec![0u8; length];
        self.stream.read_exact(&mut buffer).await?;
        
        if self.is_relay {
            // For relay connections, buffer contains serialized encrypted data from other peer
            let encrypted_data: crate::crypto::EncryptedMessage = bincode::deserialize(&buffer)?;
            let mut crypto = self.crypto.lock().await;
            let decrypted = crypto.decrypt(&encrypted_data)?;
            let text = String::from_utf8(decrypted)
                .map_err(|_| NetworkError::InvalidMessage)?;
            Ok(Some(text))
        } else {
            // For direct P2P connections, deserialize NetworkMessage
            let message: NetworkMessage = bincode::deserialize(&buffer)?;
            
            // Decrypt and process based on type
            match message.message_type {
                MessageType::Text => {
                    let mut crypto = self.crypto.lock().await;
                    let decrypted = crypto.decrypt(&message.encrypted_data)?;
                    let text = String::from_utf8(decrypted)
                        .map_err(|_| NetworkError::InvalidMessage)?;
                    Ok(Some(text))
                }
                MessageType::KeyRotation => {
                    // Handle key rotation notification
                    let mut crypto = self.crypto.lock().await;
                    crypto.rotate_keys()?;
                    Ok(None) // Don't return key rotation as user message
                }
                MessageType::Heartbeat => {
                    // Handle heartbeat
                    Ok(None) // Don't return heartbeat as user message
                }
            }
        }
    }
    
    /// Get peer address
    pub fn peer_addr(&self) -> SocketAddr {
        self.peer_addr
    }
    
    /// Send heartbeat
    pub async fn send_heartbeat(&mut self) -> Result<(), NetworkError> {
        let message = NetworkMessage {
            id: uuid::Uuid::new_v4().to_string(),
            message_type: MessageType::Heartbeat,
            encrypted_data: {
                let mut crypto = self.crypto.lock().await;
                crypto.encrypt(b"heartbeat")?
            },
        };
        
        self.send_message(&message).await
    }
}

/// P2P server for accepting connections
pub struct P2PServer {
    listener: TcpListener,
    crypto: Arc<Mutex<SilenceCrypto>>,
    max_message_size: usize,
}

impl P2PServer {
    /// Create new P2P server
    pub async fn new(
        bind_addr: SocketAddr,
        crypto: Arc<Mutex<SilenceCrypto>>,
        max_message_size: usize,
    ) -> Result<Self, NetworkError> {
        let listener = TcpListener::bind(bind_addr).await?;
        Ok(Self {
            listener,
            crypto,
            max_message_size,
        })
    }
    
    /// Accept a single connection (P2P limitation)
    pub async fn accept(&self) -> Result<P2PConnection, NetworkError> {
        let (stream, addr) = self.listener.accept().await?;
        Ok(P2PConnection::new(
            stream,
            addr,
            Arc::clone(&self.crypto),
            self.max_message_size,
            false, // Server connections are direct P2P, not relay
        ).await)
    }
    
    /// Get local address
    pub fn local_addr(&self) -> Result<SocketAddr, NetworkError> {
        Ok(self.listener.local_addr()?)
    }
}

// Simplified without complex trait bounds to avoid Send issues

/// Connection manager for handling P2P connections
pub struct ConnectionManager {
    crypto: Arc<Mutex<SilenceCrypto>>,
    max_message_size: usize,
    relay_servers: Vec<String>,
}

impl ConnectionManager {
    /// Create new connection manager
    pub fn new(crypto: Arc<Mutex<SilenceCrypto>>, max_message_size: usize) -> Self {
        Self {
            crypto,
            max_message_size,
            relay_servers: Vec::new(),
        }
    }
    
    /// Create new connection manager with relay servers
    pub fn with_relays(
        crypto: Arc<Mutex<SilenceCrypto>>, 
        max_message_size: usize,
        relay_servers: Vec<String>
    ) -> Self {
        Self {
            crypto,
            max_message_size,
            relay_servers,
        }
    }
    
    /// Start server and accept a single connection (P2P)
    pub async fn start_server(&self, bind_addr: SocketAddr) -> Result<P2PConnection, NetworkError> {
        let server = P2PServer::new(
            bind_addr,
            Arc::clone(&self.crypto),
            self.max_message_size,
        ).await?;
        
        println!("Server listening on {}", server.local_addr()?);
        let connection = server.accept().await?;
        Ok(connection)
    }
    
    /// Connect to peer (try direct first, then relay)
    pub async fn connect_to_peer(&self, addr: SocketAddr) -> Result<P2PConnection, NetworkError> {
        self.connect_with_mode(addr, crate::ConnectionMode::Auto).await
    }
    
    /// Connect to peer with specific connection mode
    pub async fn connect_with_mode(&self, addr: SocketAddr, mode: crate::ConnectionMode) -> Result<P2PConnection, NetworkError> {
        match mode {
            crate::ConnectionMode::Auto => {
                // Try direct connection first
                match P2PConnection::connect(addr, Arc::clone(&self.crypto), self.max_message_size, false).await {
                    Ok(connection) => {
                        tracing::info!("Direct P2P connection established to {}", addr);
                        Ok(connection)
                    }
                    Err(direct_err) => {
                        tracing::warn!("Direct connection failed: {}, trying relay servers", direct_err);
                        self.connect_via_relay().await.or(Err(direct_err))
                    }
                }
            }
            crate::ConnectionMode::DirectOnly => {
                // Only try direct connection
                match P2PConnection::connect(addr, Arc::clone(&self.crypto), self.max_message_size, false).await {
                    Ok(connection) => {
                        tracing::info!("Direct P2P connection established to {}", addr);
                        Ok(connection)
                    }
                    Err(err) => {
                        tracing::error!("Direct connection failed (direct-only mode): {}", err);
                        Err(err)
                    }
                }
            }
            crate::ConnectionMode::RelayOnly => {
                // Only try relay connections
                tracing::info!("Using relay-only connection mode");
                self.connect_via_relay().await
            }
        }
    }
    
    /// Connect via relay servers only
    async fn connect_via_relay(&self) -> Result<P2PConnection, NetworkError> {
        for relay in &self.relay_servers {
            if let Ok(relay_addr) = relay.parse::<SocketAddr>() {
                match P2PConnection::connect(relay_addr, Arc::clone(&self.crypto), self.max_message_size, true).await {
                    Ok(connection) => {
                        tracing::info!("Relay connection established via {}", relay);
                        return Ok(connection);
                    }
                    Err(relay_err) => {
                        tracing::warn!("Relay {} failed: {}", relay, relay_err);
                        continue;
                    }
                }
            }
        }
        
        Err(NetworkError::Connection(std::io::Error::new(
            std::io::ErrorKind::ConnectionRefused,
            "All relay servers failed"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::SilenceCrypto;
    use std::time::Duration;
    use tokio::time::timeout;
    
    #[tokio::test]
    async fn test_local_connection() {
        let crypto1 = Arc::new(Mutex::new(SilenceCrypto::new(60).unwrap()));
        let crypto2 = Arc::new(Mutex::new(SilenceCrypto::new(60).unwrap()));
        
        let server_addr = "127.0.0.1:0".parse().unwrap();
        let server = P2PServer::new(server_addr, crypto1, 4096).await.unwrap();
        let actual_addr = server.local_addr().unwrap();
        
        // Spawn server task
        tokio::spawn(async move {
            let mut connection = server.accept().await.unwrap();
            if let Ok(Some(message)) = connection.receive_message().await {
                assert_eq!(message, "Hello, P2P!");
            }
        });
        
        // Give server time to start
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Connect and send message
        let mut client = P2PConnection::connect(actual_addr, crypto2, 4096, false).await.unwrap();
        
        timeout(Duration::from_secs(5), client.send_text("Hello, P2P!"))
            .await
            .unwrap()
            .unwrap();
    }
}