use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;
use tracing::info;

mod ssh;
mod builder;
mod packager;
mod deployer;

use ssh::SshClient;
use builder::Builder;
use packager::Packager;
use deployer::Deployer;

#[derive(Parser, Clone)]
#[command(
    name = "deploy",
    about = "Rust deployment tool for Silence Relay Server on Cherry Servers",
    version = "0.1.0"
)]
struct Args {
    /// Cherry Server hostname
    #[arg(long, env = "CHERRY_HOST", default_value = "your-server.cherryservers.net")]
    host: String,

    /// SSH username
    #[arg(long, env = "CHERRY_USER", default_value = "root")]
    user: String,

    /// SSH private key path
    #[arg(long, env = "SSH_KEY", default_value = "~/.ssh/id_rsa")]
    ssh_key: PathBuf,

    /// Relay server port
    #[arg(long, env = "RELAY_PORT", default_value = "8080")]
    port: u16,

    /// Maximum number of clients
    #[arg(long, env = "MAX_CLIENTS", default_value = "100")]
    max_clients: u32,

    /// Maximum message size in bytes
    #[arg(long, env = "MAX_MESSAGE_SIZE", default_value = "65536")]
    max_message_size: u32,

    /// Bind address for the relay server
    #[arg(long, env = "BIND_ADDRESS", default_value = "0.0.0.0")]
    bind_address: String,

    /// Skip building and use existing binary
    #[arg(long)]
    skip_build: bool,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

pub struct DeploymentConfig {
    pub host: String,
    pub user: String,
    pub ssh_key: PathBuf,
    pub port: u16,
    pub max_clients: u32,
    pub max_message_size: u32,
    pub bind_address: String,
}

impl From<Args> for DeploymentConfig {
    fn from(args: Args) -> Self {
        Self {
            host: args.host,
            user: args.user,
            ssh_key: expand_home_path(args.ssh_key),
            port: args.port,
            max_clients: args.max_clients,
            max_message_size: args.max_message_size,
            bind_address: args.bind_address,
        }
    }
}

fn expand_home_path(path: PathBuf) -> PathBuf {
    if path.starts_with("~") {
        if let Some(home) = dirs::home_dir() {
            home.join(path.strip_prefix("~").unwrap_or(&path))
        } else {
            path
        }
    } else {
        path
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    let log_level = if args.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(log_level)
        .with_target(false)
        .init();

    let config = DeploymentConfig::from(args.clone());

    info!("🍒 Starting deployment to Cherry Servers...");
    info!("Target: {}@{}", config.user, config.host);
    info!("Port: {}", config.port);

    // Step 1: Build the relay server binary (unless skipped)
    let binary_path = if !args.skip_build {
        info!("📦 Building relay server...");
        let builder = Builder::new();
        builder.build().await
            .context("Failed to build relay server")?
    } else {
        info!("⏭️  Skipping build, using existing binary");
        PathBuf::from("relay-server/target/release/silence-relay")
    };

    // Step 2: Create deployment package
    info!("📦 Creating deployment package...");
    let packager = Packager::new(&config);
    let package_path = packager.create_package(&binary_path).await
        .context("Failed to create deployment package")?;

    // Step 3: Connect to Cherry Server via SSH
    info!("🔗 Connecting to Cherry Server...");
    let mut ssh_client = SshClient::new(&config).await
        .context("Failed to create SSH client")?;

    // Step 4: Deploy the package
    info!("🚀 Deploying to server...");
    let mut deployer = Deployer::new(&mut ssh_client, &config);
    deployer.deploy(&package_path).await
        .context("Failed to deploy to server")?;

    info!("✅ Deployment complete! Relay server should now be running on {}:{}", 
          config.host, config.port);

    Ok(())
}