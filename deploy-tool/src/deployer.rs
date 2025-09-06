use anyhow::{Context, Result, anyhow};
use std::path::Path;
use tracing::{debug, info, warn};

use crate::{DeploymentConfig, ssh::SshClient};

pub struct Deployer<'a> {
    ssh_client: &'a mut SshClient,
    config: &'a DeploymentConfig,
}

impl<'a> Deployer<'a> {
    pub fn new(ssh_client: &'a mut SshClient, config: &'a DeploymentConfig) -> Self {
        Self { ssh_client, config }
    }

    pub async fn deploy(&mut self, package_path: &Path) -> Result<()> {
        info!("Starting deployment to Cherry Server...");

        // Step 1: Upload deployment package
        self.upload_package(package_path).await
            .context("Failed to upload deployment package")?;

        // Step 2: Extract package on remote server
        self.extract_package().await
            .context("Failed to extract deployment package")?;

        // Step 3: Run installation script
        self.run_installation().await
            .context("Failed to run installation")?;

        // Step 4: Start the service
        self.start_service().await
            .context("Failed to start relay service")?;

        // Step 5: Verify deployment
        self.verify_deployment().await
            .context("Failed to verify deployment")?;

        // Step 6: Cleanup temporary files
        self.cleanup_remote_files().await
            .context("Failed to cleanup remote files")?;

        info!("🎉 Deployment completed successfully!");
        Ok(())
    }

    async fn upload_package(&mut self, package_path: &Path) -> Result<()> {
        info!("📤 Uploading deployment package...");
        
        let remote_package_path = "/tmp/silence-relay-deploy.tar.gz";
        
        self.ssh_client.upload_file(package_path, remote_package_path).await
            .context("Failed to upload package to server")?;

        // Verify upload completed successfully
        if !self.ssh_client.file_exists(remote_package_path).await? {
            return Err(anyhow!("Package upload verification failed"));
        }

        info!("✅ Package uploaded successfully");
        Ok(())
    }

    async fn extract_package(&mut self) -> Result<()> {
        info!("📦 Extracting deployment package...");

        // Create extraction directory
        self.ssh_client.execute_command("rm -rf /tmp/silence-relay-extract").await
            .context("Failed to clean extraction directory")?;

        self.ssh_client.create_directory("/tmp/silence-relay-extract").await
            .context("Failed to create extraction directory")?;

        // Extract tarball
        let extract_cmd = "cd /tmp/silence-relay-extract && tar -xzf /tmp/silence-relay-deploy.tar.gz";
        self.ssh_client.execute_command(extract_cmd).await
            .context("Failed to extract deployment package")?;

        // Verify extraction
        let verify_cmd = "ls -la /tmp/silence-relay-extract/";
        let output = self.ssh_client.execute_command(verify_cmd).await
            .context("Failed to verify package extraction")?;

        debug!("Extracted files: {}", output);

        // Check for required files
        let required_files = ["silence-relay", "silence-relay.service", "install.sh"];
        for file in &required_files {
            let file_path = format!("/tmp/silence-relay-extract/{}", file);
            if !self.ssh_client.file_exists(&file_path).await? {
                return Err(anyhow!("Required file missing after extraction: {}", file));
            }
        }

        info!("✅ Package extracted successfully");
        Ok(())
    }

    async fn run_installation(&mut self) -> Result<()> {
        info!("🔧 Running installation script...");

        // Make install script executable (just in case)
        self.ssh_client.execute_command("chmod +x /tmp/silence-relay-extract/install.sh").await
            .context("Failed to make install script executable")?;

        // Run installation script with elevated privileges
        let install_cmd = "cd /tmp/silence-relay-extract && sudo ./install.sh";
        let output = self.ssh_client.execute_command(install_cmd).await
            .context("Failed to run installation script")?;

        debug!("Installation output: {}", output);

        // Verify installation was successful
        self.verify_installation().await
            .context("Installation verification failed")?;

        info!("✅ Installation completed successfully");
        Ok(())
    }

    async fn verify_installation(&mut self) -> Result<()> {
        debug!("Verifying installation...");

        // Check if binary was installed
        if !self.ssh_client.file_exists("/opt/silence-relay/silence-relay").await? {
            return Err(anyhow!("Binary not found at /opt/silence-relay/silence-relay"));
        }

        // Check if systemd service was installed
        if !self.ssh_client.file_exists("/etc/systemd/system/silence-relay.service").await? {
            return Err(anyhow!("Systemd service not found"));
        }

        // Check if service is enabled
        match self.ssh_client.execute_command("systemctl is-enabled silence-relay").await {
            Ok(output) => {
                if !output.trim().contains("enabled") {
                    warn!("Service may not be properly enabled: {}", output);
                }
            }
            Err(e) => {
                warn!("Could not verify service status: {}", e);
            }
        }

        debug!("Installation verification passed");
        Ok(())
    }

    async fn start_service(&mut self) -> Result<()> {
        info!("🚀 Starting relay service...");

        // Stop existing service if running
        let _ = self.ssh_client.execute_command("sudo systemctl stop silence-relay").await;

        // Start the service
        self.ssh_client.execute_command("sudo systemctl start silence-relay").await
            .context("Failed to start relay service")?;

        // Wait a moment for the service to start
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // Check service status
        let status_output = self.ssh_client.execute_command("sudo systemctl status silence-relay").await
            .context("Failed to check service status")?;

        debug!("Service status: {}", status_output);

        // Verify service is active
        let is_active = self.ssh_client.execute_command("sudo systemctl is-active silence-relay").await
            .context("Failed to check if service is active")?;

        if !is_active.trim().contains("active") {
            return Err(anyhow!("Service is not active: {}", is_active.trim()));
        }

        info!("✅ Service started successfully");
        Ok(())
    }

    async fn verify_deployment(&mut self) -> Result<()> {
        info!("🔍 Verifying deployment...");

        // Check if the service is listening on the expected port
        let port_check = format!("netstat -tuln | grep :{}", self.config.port);
        match self.ssh_client.execute_command(&port_check).await {
            Ok(output) => {
                if output.is_empty() {
                    warn!("Service may not be listening on port {}", self.config.port);
                } else {
                    info!("✅ Service is listening on port {}", self.config.port);
                    debug!("Port check output: {}", output);
                }
            }
            Err(_) => {
                // netstat might not be available, try alternative check
                let ss_check = format!("ss -tuln | grep :{}", self.config.port);
                match self.ssh_client.execute_command(&ss_check).await {
                    Ok(output) => {
                        if !output.is_empty() {
                            info!("✅ Service is listening on port {}", self.config.port);
                            debug!("Port check output: {}", output);
                        } else {
                            warn!("Service may not be listening on port {}", self.config.port);
                        }
                    }
                    Err(_) => {
                        warn!("Could not verify port listening status");
                    }
                }
            }
        }

        // Get recent logs to verify service is working
        let logs = self.ssh_client.execute_command("sudo journalctl -u silence-relay --no-pager -n 10").await
            .context("Failed to get service logs")?;

        debug!("Recent service logs: {}", logs);

        // Check for error patterns in logs
        if logs.contains("ERROR") || logs.contains("Failed") || logs.contains("Error") {
            warn!("Service logs contain error messages");
            info!("Recent logs: {}", logs);
        }

        info!("✅ Deployment verification completed");
        Ok(())
    }

    async fn cleanup_remote_files(&mut self) -> Result<()> {
        info!("🧹 Cleaning up temporary files...");

        // Remove extraction directory
        let _ = self.ssh_client.execute_command("rm -rf /tmp/silence-relay-extract").await;

        // Remove uploaded package
        let _ = self.ssh_client.execute_command("rm -f /tmp/silence-relay-deploy.tar.gz").await;

        info!("✅ Cleanup completed");
        Ok(())
    }

    pub async fn stop_service(&mut self) -> Result<()> {
        info!("🛑 Stopping relay service...");

        self.ssh_client.execute_command("sudo systemctl stop silence-relay").await
            .context("Failed to stop relay service")?;

        info!("✅ Service stopped");
        Ok(())
    }

    pub async fn restart_service(&mut self) -> Result<()> {
        info!("🔄 Restarting relay service...");

        self.ssh_client.execute_command("sudo systemctl restart silence-relay").await
            .context("Failed to restart relay service")?;

        // Wait for service to start
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // Verify service is running
        let is_active = self.ssh_client.execute_command("sudo systemctl is-active silence-relay").await
            .context("Failed to check service status after restart")?;

        if !is_active.trim().contains("active") {
            return Err(anyhow!("Service is not active after restart: {}", is_active.trim()));
        }

        info!("✅ Service restarted successfully");
        Ok(())
    }

    pub async fn get_service_status(&mut self) -> Result<String> {
        let status = self.ssh_client.execute_command("sudo systemctl status silence-relay --no-pager").await
            .context("Failed to get service status")?;

        Ok(status)
    }

    pub async fn get_service_logs(&mut self, lines: u32) -> Result<String> {
        let log_cmd = format!("sudo journalctl -u silence-relay --no-pager -n {}", lines);
        let logs = self.ssh_client.execute_command(&log_cmd).await
            .context("Failed to get service logs")?;

        Ok(logs)
    }

    pub async fn uninstall(&mut self) -> Result<()> {
        info!("🗑️  Uninstalling relay service...");

        // Stop and disable service
        let _ = self.ssh_client.execute_command("sudo systemctl stop silence-relay").await;
        let _ = self.ssh_client.execute_command("sudo systemctl disable silence-relay").await;

        // Remove systemd service file
        let _ = self.ssh_client.execute_command("sudo rm -f /etc/systemd/system/silence-relay.service").await;

        // Reload systemd
        let _ = self.ssh_client.execute_command("sudo systemctl daemon-reload").await;

        // Remove installation directory
        let _ = self.ssh_client.execute_command("sudo rm -rf /opt/silence-relay").await;

        // Remove user (optional, commented out for safety)
        // let _ = self.ssh_client.execute_command("sudo userdel relay").await;

        info!("✅ Uninstallation completed");
        Ok(())
    }
}