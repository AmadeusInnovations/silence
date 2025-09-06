// Simple test client for Silence Relay Server
// Usage: cargo run --bin test-client -- --relay-server 127.0.0.1:8080

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use clap::Parser;
use std::time::Duration;

#[derive(Parser)]
#[command(name = "test-client")]
#[command(about = "Test client for Silence Relay Server")]
struct Args {
    #[arg(long, default_value = "127.0.0.1:8080")]
    relay_server: String,
    
    #[arg(long, default_value = "client")]
    name: String,
}

async fn send_message(stream: &mut TcpStream, data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let length = data.len() as u32;
    stream.write_u32(length).await?;
    stream.write_all(data).await?;
    stream.flush().await?;
    Ok(())
}

async fn read_message(stream: &mut TcpStream) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error>> {
    let length = match stream.read_u32().await {
        Ok(len) => len as usize,
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(Box::new(e)),
    };

    let mut buffer = vec![0u8; length];
    stream.read_exact(&mut buffer).await?;
    Ok(Some(buffer))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    
    println!("Connecting to relay server at {}", args.relay_server);
    let mut stream = TcpStream::connect(&args.relay_server).await?;
    println!("Connected successfully!");
    
    // Send test messages
    let test_messages = vec![
        format!("Hello from {}!", args.name),
        format!("{} is testing the relay", args.name),
        format!("Encrypted message from {}", args.name),
        format!("Final test message from {}", args.name),
    ];
    
    let mut receive_task = {
        let mut read_stream = stream.try_clone()?;
        tokio::spawn(async move {
            loop {
                match read_message(&mut read_stream).await {
                    Ok(Some(data)) => {
                        if let Ok(message) = String::from_utf8(data) {
                            println!("📨 Received: {}", message);
                        } else {
                            println!("📨 Received {} bytes of binary data", data.len());
                        }
                    }
                    Ok(None) => {
                        println!("🔌 Connection closed by server");
                        break;
                    }
                    Err(e) => {
                        println!("❌ Read error: {}", e);
                        break;
                    }
                }
            }
        })
    };
    
    // Send test messages with delays
    for (i, message) in test_messages.iter().enumerate() {
        tokio::time::sleep(Duration::from_secs(2)).await;
        
        println!("📤 Sending: {}", message);
        if let Err(e) = send_message(&mut stream, message.as_bytes()).await {
            println!("❌ Send error: {}", e);
            break;
        }
        
        if i == 0 {
            println!("💡 If you have another test client running, you should see messages being relayed");
        }
    }
    
    // Keep connection alive to receive messages
    println!("⏳ Waiting for messages (press Ctrl+C to exit)...");
    
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            println!("🛑 Shutting down...");
        }
        _ = &mut receive_task => {
            println!("🔌 Receive task completed");
        }
    }
    
    Ok(())
}