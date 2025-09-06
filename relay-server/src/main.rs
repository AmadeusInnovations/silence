// Silence Relay Server - Minimal TCP packet forwarding for P2P clients
// Deploys on Cherry Servers bare metal for encrypted packet relay

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, broadcast};
use tracing::{info, warn, error, debug};
use uuid::Uuid;
use clap::Parser;

/// Command line arguments
#[derive(Parser, Debug)]
#[command(name = "silence-relay")]
#[command(about = "Minimal TCP relay server for encrypted P2P communication")]
struct Args {
    /// Port to bind the relay server
    #[arg(short, long, default_value = "8080", env = "RELAY_PORT")]
    port: u16,

    /// Maximum number of concurrent clients
    #[arg(short, long, default_value = "100", env = "MAX_CLIENTS")]
    max_clients: usize,

    /// Maximum message size in bytes
    #[arg(short, long, default_value = "65536", env = "MAX_MESSAGE_SIZE")]
    max_message_size: usize,

    /// Bind address (default: all interfaces)
    #[arg(short, long, default_value = "0.0.0.0", env = "BIND_ADDRESS")]
    bind_address: String,
}

/// Client connection information
#[derive(Debug, Clone)]
struct ClientInfo {
    id: Uuid,
    addr: SocketAddr,
    sender: broadcast::Sender<Vec<u8>>,
}

/// Relay server state
struct RelayServer {
    clients: Arc<Mutex<HashMap<Uuid, ClientInfo>>>,
    args: Args,
}

impl RelayServer {
    fn new(args: Args) -> Self {
        Self {
            clients: Arc::new(Mutex::new(HashMap::new())),
            args,
        }
    }

    /// Start the relay server
    async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let bind_addr = format!("{}:{}", self.args.bind_address, self.args.port);
        let listener = TcpListener::bind(&bind_addr).await?;
        
        info!("Silence Relay Server starting on {}", bind_addr);
        info!("Max clients: {}", self.args.max_clients);
        info!("Max message size: {} bytes", self.args.max_message_size);

        // Handle graceful shutdown
        let clients = Arc::clone(&self.clients);
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
            info!("Shutdown signal received");
            
            // Notify all clients of shutdown
            let clients_guard = clients.lock().await;
            for client in clients_guard.values() {
                let _ = client.sender.send(Vec::new()); // Empty message signals shutdown
            }
        });

        loop {
            match listener.accept().await {
                Ok((mut stream, addr)) => {
                    let clients_count = self.clients.lock().await.len();
                    
                    if clients_count >= self.args.max_clients {
                        warn!("Max clients ({}) reached, rejecting connection from {}", 
                              self.args.max_clients, addr);
                        let _ = stream.shutdown().await;
                        continue;
                    }

                    info!("New client connection from {}", addr);
                    let client_handler = ClientHandler {
                        clients: Arc::clone(&self.clients),
                        max_message_size: self.args.max_message_size,
                    };
                    
                    tokio::spawn(async move {
                        if let Err(e) = client_handler.handle_client(stream, addr).await {
                            error!("Client handler error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                }
            }
        }
    }
}

/// Handles individual client connections
struct ClientHandler {
    clients: Arc<Mutex<HashMap<Uuid, ClientInfo>>>,
    max_message_size: usize,
}

impl ClientHandler {
    /// Handle a client connection
    async fn handle_client(
        &self, 
        stream: TcpStream, 
        addr: SocketAddr
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let client_id = Uuid::new_v4();
        let (tx, mut rx) = broadcast::channel(64);
        
        // Register client
        {
            let client_info = ClientInfo {
                id: client_id,
                addr,
                sender: tx.clone(),
            };
            self.clients.lock().await.insert(client_id, client_info);
            info!("Client {} ({}) registered", client_id, addr);
        }

        // Split stream for concurrent read/write
        let (mut read_half, mut write_half) = stream.into_split();
        
        // Spawn task to handle outbound messages to this client
        let _clients_for_writer = Arc::clone(&self.clients);
        let client_id_for_writer = client_id;
        let write_task = tokio::spawn(async move {
            while let Ok(data) = rx.recv().await {
                if data.is_empty() {
                    // Empty data signals shutdown
                    break;
                }
                
                if let Err(e) = Self::send_message(&mut write_half, &data).await {
                    error!("Failed to send message to client {}: {}", client_id_for_writer, e);
                    break;
                }
            }
        });

        // Handle inbound messages from this client
        loop {
            match self.read_message(&mut read_half).await {
                Ok(Some(data)) => {
                    debug!("Received {} bytes from client {}", data.len(), client_id);
                    
                    // Forward message to all other clients
                    self.broadcast_message(client_id, data).await;
                }
                Ok(None) => {
                    // Client disconnected gracefully
                    info!("Client {} disconnected", client_id);
                    break;
                }
                Err(e) => {
                    warn!("Error reading from client {}: {}", client_id, e);
                    break;
                }
            }
        }

        // Cleanup
        write_task.abort();
        self.clients.lock().await.remove(&client_id);
        info!("Client {} ({}) unregistered", client_id, addr);

        Ok(())
    }

    /// Read a message from the stream (length-prefixed)
    async fn read_message(&self, stream: &mut tokio::net::tcp::OwnedReadHalf) -> 
        Result<Option<Vec<u8>>, Box<dyn std::error::Error + Send + Sync>> {
        
        // Read 4-byte length prefix
        let length = match stream.read_u32().await {
            Ok(len) => len as usize,
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(Box::new(e)),
        };

        if length > self.max_message_size {
            return Err(format!("Message too large: {} > {}", length, self.max_message_size).into());
        }

        if length == 0 {
            return Err("Invalid zero-length message".into());
        }

        // Read message data
        let mut buffer = vec![0u8; length];
        stream.read_exact(&mut buffer).await?;
        
        Ok(Some(buffer))
    }

    /// Send a message to the stream (length-prefixed)
    async fn send_message(stream: &mut tokio::net::tcp::OwnedWriteHalf, data: &[u8]) -> 
        Result<(), Box<dyn std::error::Error + Send + Sync>> {
        
        let length = data.len() as u32;
        stream.write_u32(length).await?;
        stream.write_all(data).await?;
        stream.flush().await?;
        Ok(())
    }

    /// Broadcast message to all clients except sender
    async fn broadcast_message(&self, sender_id: Uuid, data: Vec<u8>) {
        let clients_guard = self.clients.lock().await;
        let mut failed_clients = Vec::new();

        for (client_id, client_info) in clients_guard.iter() {
            if *client_id == sender_id {
                continue; // Don't echo back to sender
            }

            if let Err(_) = client_info.sender.send(data.clone()) {
                // Client channel is closed
                failed_clients.push(*client_id);
            }
        }

        // Clean up failed clients (will be handled by their connection tasks)
        for failed_id in failed_clients {
            debug!("Client {} channel closed during broadcast", failed_id);
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args = Args::parse();
    let server = RelayServer::new(args);
    
    server.run().await
}