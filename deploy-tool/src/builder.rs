use anyhow::{Context, Result, anyhow};
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, info, warn, error};

pub struct Builder {
    workspace_root: PathBuf,
}

impl Builder {
    pub fn new() -> Self {
        // Determine workspace root (should be parent of deploy-tool)
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let workspace_root = if current_dir.file_name().map(|n| n == "deploy-tool").unwrap_or(false) {
            current_dir.parent().unwrap_or(&current_dir).to_path_buf()
        } else {
            current_dir
        };

        Self { workspace_root }
    }

    pub async fn build(&self) -> Result<PathBuf> {
        info!("Building relay server in release mode...");
        
        // Change to relay-server directory
        let relay_server_dir = self.workspace_root.join("relay-server");
        
        if !relay_server_dir.exists() {
            return Err(anyhow!("Relay server directory not found at {:?}", relay_server_dir));
        }

        debug!("Building in directory: {:?}", relay_server_dir);

        // Execute cargo build --release
        let build_cmd = Command::new("cargo")
            .args(&["build", "--release"])
            .current_dir(&relay_server_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Failed to spawn cargo build command")?;

        // Wait for the build to complete
        let output = build_cmd.wait_with_output().await
            .context("Failed to wait for cargo build")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            
            error!("Build failed!");
            error!("stdout: {}", stdout);
            error!("stderr: {}", stderr);
            
            return Err(anyhow!("Cargo build failed with exit code {:?}", output.status.code()));
        }

        // Log build output for debugging
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !stdout.is_empty() {
            debug!("Build output: {}", stdout);
        }

        // Verify the binary exists
        let binary_path = relay_server_dir.join("target/release/silence-relay");
        
        if !binary_path.exists() {
            return Err(anyhow!("Built binary not found at {:?}", binary_path));
        }

        // Check if binary is executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = tokio::fs::metadata(&binary_path).await
                .context("Failed to get binary metadata")?;
            let permissions = metadata.permissions();
            
            if permissions.mode() & 0o111 == 0 {
                warn!("Binary may not be executable: {:?}", binary_path);
            }
        }

        info!("✅ Build completed successfully");
        info!("Binary location: {:?}", binary_path);

        // Get binary size for informational purposes
        let metadata = tokio::fs::metadata(&binary_path).await
            .context("Failed to get binary metadata")?;
        let size_mb = metadata.len() as f64 / 1_048_576.0;
        info!("Binary size: {:.2} MB", size_mb);

        Ok(binary_path)
    }

    pub async fn clean(&self) -> Result<()> {
        info!("Cleaning build artifacts...");
        
        let relay_server_dir = self.workspace_root.join("relay-server");
        
        let clean_cmd = Command::new("cargo")
            .args(&["clean"])
            .current_dir(&relay_server_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Failed to spawn cargo clean command")?;

        let output = clean_cmd.wait_with_output().await
            .context("Failed to wait for cargo clean")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("Clean command failed: {}", stderr);
        } else {
            info!("✅ Clean completed");
        }

        Ok(())
    }

    pub fn get_binary_path(&self) -> PathBuf {
        self.workspace_root.join("relay-server/target/release/silence-relay")
    }

    pub async fn verify_cargo_available(&self) -> Result<()> {
        let output = Command::new("cargo")
            .args(&["--version"])
            .output()
            .await
            .context("Failed to execute cargo --version")?;

        if !output.status.success() {
            return Err(anyhow!("Cargo is not available or not working properly"));
        }

        let version = String::from_utf8_lossy(&output.stdout);
        debug!("Cargo version: {}", version.trim());
        
        Ok(())
    }

    pub async fn check_dependencies(&self) -> Result<()> {
        info!("Checking build dependencies...");
        
        self.verify_cargo_available().await
            .context("Cargo verification failed")?;

        let relay_server_dir = self.workspace_root.join("relay-server");
        
        if !relay_server_dir.join("Cargo.toml").exists() {
            return Err(anyhow!("relay-server/Cargo.toml not found"));
        }

        info!("✅ Build dependencies verified");
        Ok(())
    }
}