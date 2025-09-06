use anyhow::{Context, Result, anyhow};
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use tracing::{debug, info};

use crate::DeploymentConfig;

pub struct Packager<'a> {
    config: &'a DeploymentConfig,
}

impl<'a> Packager<'a> {
    pub fn new(config: &'a DeploymentConfig) -> Self {
        Self { config }
    }

    pub async fn create_package(&self, binary_path: &Path) -> Result<PathBuf> {
        info!("Creating deployment package...");

        // Create temporary directory for packaging
        let temp_dir = TempDir::new()
            .context("Failed to create temporary directory")?;
        
        let package_dir = temp_dir.path().join("silence-relay-deploy");
        tokio::fs::create_dir_all(&package_dir).await
            .context("Failed to create package directory")?;

        // Copy binary to package directory
        let binary_dest = package_dir.join("silence-relay");
        tokio::fs::copy(binary_path, &binary_dest).await
            .context("Failed to copy binary to package directory")?;

        debug!("Copied binary to {:?}", binary_dest);

        // Create systemd service file
        let service_content = self.create_systemd_service();
        let service_file = package_dir.join("silence-relay.service");
        tokio::fs::write(&service_file, service_content).await
            .context("Failed to write systemd service file")?;

        debug!("Created systemd service file at {:?}", service_file);

        // Create installation script
        let install_script = self.create_install_script();
        let install_file = package_dir.join("install.sh");
        tokio::fs::write(&install_file, install_script).await
            .context("Failed to write installation script")?;

        // Make install script executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = tokio::fs::metadata(&install_file).await
                .context("Failed to get install script metadata")?
                .permissions();
            perms.set_mode(0o755);
            tokio::fs::set_permissions(&install_file, perms).await
                .context("Failed to set install script permissions")?;
        }

        debug!("Created installation script at {:?}", install_file);

        // Create deployment configuration file
        let config_content = self.create_config_file()?;
        let config_file = package_dir.join("deploy.conf");
        tokio::fs::write(&config_file, config_content).await
            .context("Failed to write configuration file")?;

        debug!("Created configuration file at {:?}", config_file);

        // Create tarball
        let tarball_path = temp_dir.path().join("silence-relay-deploy.tar.gz");
        self.create_tarball(&package_dir, &tarball_path).await
            .context("Failed to create deployment tarball")?;

        // Move tarball to a persistent location
        let final_tarball_path = std::env::temp_dir().join("silence-relay-deploy.tar.gz");
        tokio::fs::copy(&tarball_path, &final_tarball_path).await
            .context("Failed to copy tarball to final location")?;

        info!("✅ Package created: {:?}", final_tarball_path);

        // Log package contents for verification
        let metadata = tokio::fs::metadata(&final_tarball_path).await
            .context("Failed to get package metadata")?;
        info!("Package size: {:.2} MB", metadata.len() as f64 / 1_048_576.0);

        Ok(final_tarball_path)
    }

    fn create_systemd_service(&self) -> String {
        format!(r#"[Unit]
Description=Silence Relay Server
After=network.target
Wants=network.target

[Service]
Type=simple
User=relay
Group=relay
WorkingDirectory=/opt/silence-relay
ExecStart=/opt/silence-relay/silence-relay --port {} --max-clients {} --max-message-size {} --bind-address {}
Restart=always
RestartSec=5
Environment=RUST_LOG=info
KillMode=mixed
TimeoutStopSec=5
PrivateTmp=yes
NoNewPrivileges=yes

# Security settings
ProtectSystem=strict
ProtectHome=yes
ProtectKernelTunables=yes
ProtectKernelModules=yes
ProtectControlGroups=yes
RestrictRealtime=yes
RestrictSUIDSGID=yes
LockPersonality=yes
MemoryDenyWriteExecute=yes

[Install]
WantedBy=multi-user.target
"#, 
            self.config.port, 
            self.config.max_clients, 
            self.config.max_message_size, 
            self.config.bind_address
        )
    }

    fn create_install_script(&self) -> String {
        r#"#!/bin/bash
set -euo pipefail

echo "🔧 Installing Silence Relay Server..."

# Create user for the service
if ! id -u relay >/dev/null 2>&1; then
    useradd --system --home /opt/silence-relay --shell /bin/false --comment "Silence Relay Server" relay
    echo "✅ Created relay user"
else
    echo "✅ Relay user already exists"
fi

# Create directories
mkdir -p /opt/silence-relay
chown relay:relay /opt/silence-relay

# Install binary
cp silence-relay /opt/silence-relay/
chmod +x /opt/silence-relay/silence-relay
chown relay:relay /opt/silence-relay/silence-relay

# Install systemd service
cp silence-relay.service /etc/systemd/system/
systemctl daemon-reload
systemctl enable silence-relay

# Stop existing service if running
systemctl stop silence-relay 2>/dev/null || true

echo "✅ Installation complete"
echo ""
echo "🚀 To start the service:"
echo "  systemctl start silence-relay"
echo ""
echo "📊 To check status:"
echo "  systemctl status silence-relay"
echo ""
echo "📋 To view logs:"
echo "  journalctl -u silence-relay -f"
"#.to_string()
    }

    fn create_config_file(&self) -> Result<String> {
        let config_toml = format!(r#"# Silence Relay Server Deployment Configuration
[server]
host = "{}"
port = {}
max_clients = {}
max_message_size = {}
bind_address = "{}"

[deployment]
user = "{}"
target_directory = "/opt/silence-relay"
service_name = "silence-relay"

[security]
create_user = true
enable_systemd_security = true
"#,
            self.config.host,
            self.config.port,
            self.config.max_clients,
            self.config.max_message_size,
            self.config.bind_address,
            self.config.user
        );

        Ok(config_toml)
    }

    async fn create_tarball(&self, source_dir: &Path, output_path: &Path) -> Result<()> {
        use flate2::Compression;
        use flate2::write::GzEncoder;
        use std::fs::File;
        use tar::Builder;

        debug!("Creating tarball from {:?} to {:?}", source_dir, output_path);

        // Create the compressed tar file
        let tar_gz = File::create(output_path)
            .context("Failed to create tarball file")?;
        let enc = GzEncoder::new(tar_gz, Compression::default());
        let mut tar = Builder::new(enc);

        // Add all files in the package directory to the tarball
        let mut entries = tokio::fs::read_dir(source_dir).await
            .context("Failed to read package directory")?;

        while let Some(entry) = entries.next_entry().await
            .context("Failed to read directory entry")? {
            
            let file_path = entry.path();
            let file_name = entry.file_name();
            
            if file_path.is_file() {
                debug!("Adding file to tarball: {:?}", file_name);
                tar.append_path_with_name(&file_path, &file_name)
                    .with_context(|| format!("Failed to add {:?} to tarball", file_name))?;
            }
        }

        tar.finish()
            .context("Failed to finalize tarball")?;

        debug!("Tarball created successfully");
        Ok(())
    }

    pub async fn cleanup_package(&self, package_path: &Path) -> Result<()> {
        if package_path.exists() {
            tokio::fs::remove_file(package_path).await
                .context("Failed to cleanup package file")?;
            info!("Cleaned up temporary package file");
        }
        Ok(())
    }

    pub async fn verify_package(&self, package_path: &Path) -> Result<()> {
        if !package_path.exists() {
            return Err(anyhow!("Package file does not exist: {:?}", package_path));
        }

        let metadata = tokio::fs::metadata(package_path).await
            .context("Failed to get package metadata")?;

        if metadata.len() == 0 {
            return Err(anyhow!("Package file is empty: {:?}", package_path));
        }

        debug!("Package verification passed: {:?} ({} bytes)", package_path, metadata.len());
        Ok(())
    }
}