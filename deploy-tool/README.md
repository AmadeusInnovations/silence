# Silence Relay Server Deployment Tool

A pure Rust deployment tool for deploying the Silence Relay Server to Cherry Servers via SSH. This tool replaces the bash deployment script with a comprehensive Rust solution.

## Features

- **Pure Rust Implementation**: No shell script dependencies
- **SSH-based Deployment**: Secure deployment via SSH with public key authentication
- **Automated Build Process**: Builds the relay server binary automatically
- **Package Management**: Creates deployment packages with all necessary files
- **Systemd Integration**: Automatically sets up systemd service with security hardening
- **Comprehensive Error Handling**: Detailed error reporting and logging
- **Service Management**: Built-in service start/stop/status functionality

## Prerequisites

- Rust toolchain installed
- SSH access to Cherry Server with public key authentication
- SSH private key configured (default: `~/.ssh/id_rsa`)

## Installation

Build the deployment tool from the workspace root:

```bash
cargo build --release -p deploy-tool
```

The binary will be available at `target/release/deploy`.

## Usage

### Basic Deployment

Deploy with default settings:

```bash
./target/release/deploy --host your-server.cherryservers.net --user root
```

### Environment Variables

Set deployment parameters via environment variables:

```bash
export CHERRY_HOST="your-server.cherryservers.net"
export CHERRY_USER="root"
export SSH_KEY="/path/to/your/private/key"
export RELAY_PORT="8080"
export MAX_CLIENTS="100"
export MAX_MESSAGE_SIZE="65536"
export BIND_ADDRESS="0.0.0.0"

./target/release/deploy
```

### Command Line Options

```bash
./target/release/deploy [OPTIONS]

Options:
      --host <HOST>                     Cherry Server hostname [env: CHERRY_HOST=] [default: your-server.cherryservers.net]
      --user <USER>                     SSH username [env: CHERRY_USER=] [default: root]
      --ssh-key <SSH_KEY>               SSH private key path [env: SSH_KEY=] [default: ~/.ssh/id_rsa]
      --port <PORT>                     Relay server port [env: RELAY_PORT=] [default: 8080]
      --max-clients <MAX_CLIENTS>       Maximum number of clients [env: MAX_CLIENTS=] [default: 100]
      --max-message-size <MAX_MESSAGE_SIZE>  Maximum message size in bytes [env: MAX_MESSAGE_SIZE=] [default: 65536]
      --bind-address <BIND_ADDRESS>     Bind address for the relay server [env: BIND_ADDRESS=] [default: 0.0.0.0]
      --skip-build                      Skip building and use existing binary
  -v, --verbose                         Enable verbose logging
  -h, --help                            Print help
  -V, --version                         Print version
```

## Deployment Process

The deployment tool performs the following steps:

1. **Build Phase**: Compiles the relay server binary in release mode
2. **Package Creation**: Creates a deployment package with:
   - Relay server binary
   - Systemd service file with security hardening
   - Installation script
   - Configuration file
3. **SSH Connection**: Establishes secure SSH connection to Cherry Server
4. **File Transfer**: Uploads deployment package via SCP
5. **Installation**: Runs installation script with elevated privileges
6. **Service Setup**: 
   - Creates dedicated `relay` user
   - Sets up systemd service
   - Configures security policies
7. **Service Start**: Starts the relay service
8. **Verification**: Verifies deployment success and service status
9. **Cleanup**: Removes temporary files

## Security Features

The deployed service includes comprehensive security hardening:

- **Dedicated User**: Runs under dedicated `relay` system user
- **Filesystem Protection**: Read-only filesystem, private temp directories
- **Process Isolation**: Memory execution protection, SUID/SGID restrictions
- **System Isolation**: Kernel parameter and module protection
- **Resource Limits**: Controlled resource access and usage

## Service Management

After deployment, manage the service with:

```bash
# Check service status
sudo systemctl status silence-relay

# View logs
sudo journalctl -u silence-relay -f

# Restart service
sudo systemctl restart silence-relay

# Stop service
sudo systemctl stop silence-relay
```

## Troubleshooting

### SSH Connection Issues

- Verify SSH key permissions: `chmod 600 ~/.ssh/id_rsa`
- Test SSH connection: `ssh -i ~/.ssh/id_rsa user@host`
- Check SSH agent: `ssh-add -l`

### Build Issues

- Ensure you're in the workspace root when building
- Verify Rust toolchain: `cargo --version`
- Clean build cache: `cargo clean`

### Deployment Issues

- Use verbose mode: `--verbose` or `-v`
- Check SSH connectivity and permissions
- Verify target server has required dependencies (systemd, etc.)

## Architecture

The deployment tool is structured as follows:

- **`main.rs`**: CLI interface and orchestration
- **`ssh.rs`**: SSH client for remote operations
- **`builder.rs`**: Relay server build functionality
- **`packager.rs`**: Deployment package creation
- **`deployer.rs`**: Remote deployment logic

## Comparison with Bash Script

| Feature | Bash Script | Rust Tool |
|---------|-------------|-----------|
| Dependencies | bash, ssh, scp, tar | Single Rust binary |
| Error Handling | Basic | Comprehensive with context |
| Logging | Echo statements | Structured logging |
| Type Safety | None | Full Rust type safety |
| Code Reuse | Limited | Modular design |
| Testing | Difficult | Unit testable |
| Cross-platform | Unix only | Cross-platform Rust |

## License

This deployment tool is part of the Silence project and follows the same license terms.