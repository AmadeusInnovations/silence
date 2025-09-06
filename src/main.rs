// Silence Crypto - Main Application Entry Point
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::{command, generate_handler, Builder, State};
use std::net::SocketAddr;

use silence::{
    SilenceCrypto, 
    P2PConnection, 
    ConnectionManager,
    Config
};

/// Application state shared across Tauri commands
#[derive(Clone)]
pub struct AppState {
    crypto: Arc<Mutex<SilenceCrypto>>,
    connection_manager: Arc<ConnectionManager>,
    active_connection: Arc<Mutex<Option<P2PConnection>>>,
    config: Config,
}

/// Tauri command to connect to a peer
#[command]
async fn connect_to_peer(
    address: String,
    mode: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let addr: SocketAddr = address.parse()
        .map_err(|e| format!("Invalid address format: {}", e))?;
    
    // Parse connection mode
    let connection_mode = match mode.as_str() {
        "direct" => silence::ConnectionMode::DirectOnly,
        "relay" => silence::ConnectionMode::RelayOnly,
        _ => silence::ConnectionMode::Auto, // default
    };
    
    let connection = state.connection_manager
        .connect_with_mode(addr, connection_mode)
        .await
        .map_err(|e| format!("Connection failed: {}", e))?;
    
    // Store the active connection and start message receiving
    {
        let mut active_conn = state.active_connection.lock().await;
        *active_conn = Some(connection);
    }
    
    // Start message receiving loop for client connection
    let active_connection = Arc::clone(&state.active_connection);
    tokio::spawn(async move {
        loop {
            let mut active_conn = active_connection.lock().await;
            if let Some(ref mut conn) = active_conn.as_mut() {
                match conn.receive_message().await {
                    Ok(Some(message)) => {
                        println!("Received message: {}", message);
                        // TODO: Forward message to GUI via Tauri events
                    }
                    Ok(None) => {
                        // Connection closed
                        println!("Connection closed by peer");
                        *active_conn = None;
                        break;
                    }
                    Err(e) => {
                        eprintln!("Receive error: {}", e);
                        *active_conn = None;
                        break;
                    }
                }
            } else {
                break;
            }
        }
    });
    
    Ok(format!("Connected to {}", address))
}

/// Tauri command to start listening for connections
#[command]
async fn start_listening(
    state: State<'_, AppState>,
) -> Result<String, String> {
    let bind_addr = format!("0.0.0.0:{}", state.config.listen_port)
        .parse::<SocketAddr>()
        .map_err(|e| format!("Invalid bind address: {}", e))?;
    
    // Start server in background task to accept incoming connection
    let connection_manager = Arc::clone(&state.connection_manager);
    let active_connection = Arc::clone(&state.active_connection);
    
    tokio::spawn(async move {
        match connection_manager.start_server(bind_addr).await {
            Ok(connection) => {
                println!("Peer connected successfully");
                
                // Store the connection
                {
                    let mut active_conn = active_connection.lock().await;
                    *active_conn = Some(connection);
                }
                
                // Start message receiving loop
                loop {
                    let mut active_conn = active_connection.lock().await;
                    if let Some(ref mut conn) = active_conn.as_mut() {
                        match conn.receive_message().await {
                            Ok(Some(message)) => {
                                println!("Received message: {}", message);
                                // TODO: Forward message to GUI via Tauri events
                            }
                            Ok(None) => {
                                // Connection closed
                                println!("Connection closed by peer");
                                *active_conn = None;
                                break;
                            }
                            Err(e) => {
                                eprintln!("Receive error: {}", e);
                                *active_conn = None;
                                break;
                            }
                        }
                    } else {
                        break;
                    }
                }
            }
            Err(e) => {
                eprintln!("Server error: {}", e);
            }
        }
    });
    
    Ok(format!("Listening on port {}", state.config.listen_port))
}

/// Tauri command to send a message
#[command]
async fn send_message(
    content: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    if content.len() > state.config.max_message_size {
        return Err("Message too large".to_string());
    }
    
    let mut active_conn = state.active_connection.lock().await;
    
    if let Some(ref mut connection) = active_conn.as_mut() {
        connection.send_text(&content).await
            .map_err(|e| format!("Send failed: {}", e))?;
        Ok("Message sent".to_string())
    } else {
        Err("No active connection".to_string())
    }
}

/// Tauri command to get security status
#[command]
async fn get_security_status(
    state: State<'_, AppState>,
) -> Result<SecurityStatus, String> {
    let crypto = state.crypto.lock().await;
    let seconds_until_rotation = crypto.seconds_until_rotation();
    
    Ok(SecurityStatus {
        encryption_active: true,
        key_rotation_seconds: seconds_until_rotation,
        connection_active: {
            let conn = state.active_connection.lock().await;
            conn.is_some()
        },
    })
}

#[derive(serde::Serialize)]
struct SecurityStatus {
    encryption_active: bool,
    key_rotation_seconds: u64,
    connection_active: bool,
}

/// Initialize crypto and start key rotation background task
async fn initialize_crypto(config: &Config) -> Arc<Mutex<SilenceCrypto>> {
    let crypto = Arc::new(Mutex::new(
        SilenceCrypto::new(config.key_rotation_interval)
            .expect("Failed to initialize crypto")
    ));
    
    // Start automatic key rotation task
    let crypto_for_rotation = Arc::clone(&crypto);
    let rotation_interval = config.key_rotation_interval;
    
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(
            std::time::Duration::from_secs(rotation_interval)
        );
        
        loop {
            interval.tick().await;
            let mut crypto_guard = crypto_for_rotation.lock().await;
            if let Err(e) = crypto_guard.rotate_keys() {
                eprintln!("Automatic key rotation failed: {}", e);
            }
            // Key rotation now silent - status shown in UI timestamp
        }
    });
    
    crypto
}

#[tokio::main]
async fn main() {
    // Initialize configuration
    let config = Config::default();
    
    // Initialize cryptographic engine
    let crypto = initialize_crypto(&config).await;
    
    // Initialize connection manager with relay servers
    let connection_manager = Arc::new(ConnectionManager::with_relays(
        Arc::clone(&crypto),
        config.max_message_size,
        config.relay_servers.clone(),
    ));
    
    // Create application state
    let app_state = AppState {
        crypto,
        connection_manager,
        active_connection: Arc::new(Mutex::new(None)),
        config,
    };
    
    // Start Tauri application
    Builder::default()
        .manage(app_state)
        .invoke_handler(generate_handler![
            connect_to_peer,
            start_listening,
            send_message,
            get_security_status
        ])
        .run(tauri::generate_context!())
        .expect("Error while running Tauri application");
}
