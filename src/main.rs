// Silence Crypto - Main Application Entry Point
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::{command, generate_handler, Builder, State};
use std::net::SocketAddr;

use silence_crypto::{
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
    state: State<'_, AppState>,
) -> Result<String, String> {
    let addr: SocketAddr = address.parse()
        .map_err(|e| format!("Invalid address format: {}", e))?;
    
    let connection = state.connection_manager
        .connect_to_peer(addr)
        .await
        .map_err(|e| format!("Connection failed: {}", e))?;
    
    // Store the active connection
    let mut active_conn = state.active_connection.lock().await;
    *active_conn = Some(connection);
    
    Ok(format!("Connected to {}", address))
}

/// Tauri command to start listening for connections
#[command]
async fn start_listening(
    state: State<'_, AppState>,
) -> Result<String, String> {
    let _bind_addr = format!("0.0.0.0:{}", state.config.listen_port)
        .parse::<SocketAddr>()
        .map_err(|e| format!("Invalid bind address: {}", e))?;
    
    // TODO: Implement proper message handler for GUI integration
    // This is a placeholder for the completion guide
    
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
            } else {
                println!("🔄 Keys automatically rotated");
            }
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
    
    // Initialize connection manager
    let connection_manager = Arc::new(ConnectionManager::new(
        Arc::clone(&crypto),
        config.max_message_size,
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
