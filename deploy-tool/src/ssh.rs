use anyhow::{Context, Result, anyhow};
use ssh2::Session;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use tracing::{debug, info};

use crate::DeploymentConfig;

pub struct SshClient {
    session: Session,
    _stream: TcpStream,
    config: DeploymentConfig,
}

impl SshClient {
    pub async fn new(config: &DeploymentConfig) -> Result<Self> {
        debug!("Connecting to {}@{}", config.user, config.host);

        // Connect to SSH server
        let tcp_stream = TcpStream::connect(format!("{}:22", config.host))
            .context("Failed to connect to SSH server")?;
        
        let mut session = Session::new()
            .context("Failed to create SSH session")?;
        
        session.set_tcp_stream(tcp_stream);
        session.handshake()
            .context("SSH handshake failed")?;

        // Authenticate with private key
        session.userauth_pubkey_file(
            &config.user,
            None,
            &config.ssh_key,
            None,
        ).context("SSH authentication failed")?;

        if !session.authenticated() {
            return Err(anyhow!("SSH authentication failed for user {}", config.user));
        }

        info!("Successfully connected to {}@{}", config.user, config.host);

        // Create a placeholder TCP stream (won't be used but needed for struct)
        let placeholder_tcp = TcpStream::connect(format!("{}:22", config.host))
            .context("Failed to create placeholder connection")?;

        Ok(Self {
            session,
            _stream: placeholder_tcp,
            config: config.clone(),
        })
    }

    pub async fn execute_command(&mut self, command: &str) -> Result<String> {
        debug!("Executing command: {}", command);

        let mut channel = self.session.channel_session()
            .context("Failed to open SSH channel")?;

        channel.exec(command)
            .context("Failed to execute command")?;

        let mut output = String::new();
        channel.read_to_string(&mut output)
            .context("Failed to read command output")?;

        channel.wait_close()
            .context("Failed to close channel")?;

        let exit_status = channel.exit_status()
            .context("Failed to get exit status")?;

        if exit_status != 0 {
            return Err(anyhow!(
                "Command '{}' failed with exit status {}: {}",
                command,
                exit_status,
                output
            ));
        }

        debug!("Command output: {}", output);
        Ok(output)
    }

    pub async fn upload_file<P: AsRef<Path>>(&mut self, local_path: P, remote_path: &str) -> Result<()> {
        let local_path = local_path.as_ref();
        
        debug!("Uploading {} to {}", local_path.display(), remote_path);

        // Read local file
        let file_data = std::fs::read(local_path)
            .with_context(|| format!("Failed to read file {:?}", local_path))?;

        // Create remote directory if needed
        let remote_dir = Path::new(remote_path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "/tmp".to_string());

        self.execute_command(&format!("mkdir -p {}", remote_dir)).await
            .context("Failed to create remote directory")?;

        // Use SCP to transfer the file
        let mut channel = self.session.scp_send(
            Path::new(remote_path),
            0o644,
            file_data.len() as u64,
            None,
        ).context("Failed to create SCP channel")?;

        channel.write_all(&file_data)
            .context("Failed to write file data via SCP")?;

        channel.send_eof()
            .context("Failed to send EOF")?;

        channel.wait_eof()
            .context("Failed to wait for EOF")?;

        channel.close()
            .context("Failed to close SCP channel")?;

        channel.wait_close()
            .context("Failed to wait for channel close")?;

        // Set executable permissions if it's a binary
        if local_path.extension().is_none() || 
           local_path.file_name().map(|n| n.to_string_lossy().contains("silence-relay")).unwrap_or(false) {
            self.execute_command(&format!("chmod +x {}", remote_path)).await
                .context("Failed to set executable permissions")?;
        }

        info!("Successfully uploaded {} to {}", local_path.display(), remote_path);
        Ok(())
    }

    pub async fn file_exists(&mut self, path: &str) -> Result<bool> {
        match self.execute_command(&format!("test -f {}", path)).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    pub async fn create_directory(&mut self, path: &str) -> Result<()> {
        self.execute_command(&format!("mkdir -p {}", path)).await
            .with_context(|| format!("Failed to create directory {}", path))?;
        Ok(())
    }

    pub async fn disconnect(self) -> Result<()> {
        self.session.disconnect(None, "Deployment completed", None)
            .context("Failed to disconnect SSH session")?;
        Ok(())
    }
}

impl Clone for DeploymentConfig {
    fn clone(&self) -> Self {
        Self {
            host: self.host.clone(),
            user: self.user.clone(),
            ssh_key: self.ssh_key.clone(),
            port: self.port,
            max_clients: self.max_clients,
            max_message_size: self.max_message_size,
            bind_address: self.bind_address.clone(),
        }
    }
}