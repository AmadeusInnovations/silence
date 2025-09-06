# Silence Relay Server

A minimal TCP relay server for the Silence P2P secure communication system. This server runs on Cherry Servers bare metal infrastructure to forward encrypted packets between P2P clients when direct connections are not possible.

## Features

- **Zero-Knowledge Relay**: Forwards encrypted packets without decryption
- **High Performance**: Async Rust implementation with minimal overhead
- **Scalable**: Handles multiple concurrent connections efficiently
- **Secure**: Systemd service with security hardening
- **Observable**: Structured logging and monitoring capabilities

## Architecture

```
Client A ←→ Relay Server ←→ Client B
    |                         |
    └─ Encrypted packets ─────┘
```

The relay server:
1. Accepts TCP connections from multiple clients
2. Forwards all messages between connected clients
3. Never decrypts or inspects message content
4. Maintains connection state and handles cleanup

## Quick Start

### 1. Build the Relay Server

```bash
cd relay-server
cargo build --release
```

### 2. Run Locally for Testing

```bash
# Start relay server on port 8080
cargo run -- --port 8080

# In another terminal, test with first client
cargo run --bin test-client -- --name "Alice"

# In another terminal, test with second client  
cargo run --bin test-client -- --name "Bob"
```

### 3. Deploy to Cherry Servers

```bash
# Configure your Cherry Server details
export CHERRY_HOST="your-server-ip"
export CHERRY_USER="root"
export SSH_KEY="~/.ssh/id_rsa"

# Deploy
./deploy.sh
```

## Configuration

### Environment Variables

- `RELAY_PORT`: Port to bind (default: 8080)
- `MAX_CLIENTS`: Maximum concurrent connections (default: 100)
- `MAX_MESSAGE_SIZE`: Maximum message size in bytes (default: 65536)
- `BIND_ADDRESS`: Address to bind (default: 0.0.0.0)
- `RUST_LOG`: Log level (default: info)

### Command Line Options

```bash
silence-relay --help
```

## Protocol

The relay uses a simple length-prefixed TCP protocol:

```
Message Format:
┌─────────────┬─────────────────┐
│   Length    │     Data        │
│  (4 bytes)  │  (Length bytes) │
└─────────────┴─────────────────┘
```

- Length: u32 big-endian
- Data: Raw encrypted message bytes (passed through unchanged)

## Cherry Servers Deployment

### Prerequisites

1. Cherry Servers account and API token
2. `cherryctl` CLI installed and configured
3. SSH key pair for server access

### Server Requirements

**Minimum:**
- 1 CPU core
- 1GB RAM
- 10GB storage
- 1Gbps network

**Recommended:**
- 2+ CPU cores
- 2GB+ RAM
- 20GB storage
- 1Gbps+ network

### Network Configuration

The relay server requires:
- Port 8080/tcp open for client connections
- SSH access (port 22/tcp) for management
- Outbound internet access for updates

### Security Features

- Runs as unprivileged `relay` user
- Systemd service with security restrictions
- Firewall configured to allow only necessary ports
- Fail2ban protection for SSH
- Log rotation and monitoring

## Monitoring

### Service Status
```bash
systemctl status silence-relay
```

### View Logs
```bash
journalctl -u silence-relay -f
```

### Connection Monitoring
```bash
# Active connections
ss -tulpn | grep :8080

# Resource usage
top -p $(pgrep silence-relay)
```

### Metrics

The relay server logs key metrics:
- Client connection/disconnection events
- Message forwarding statistics
- Error rates and types
- Resource utilization

## Client Integration

Update your P2P client configuration to include relay servers:

```rust
use silence::Config;

let config = Config {
    relay_servers: vec![
        "your-cherry-server.net:8080".to_string(),
        // Add backup relay servers for redundancy
    ],
    ..Default::default()
};
```

The client will automatically:
1. Try direct P2P connection first
2. Fall back to relay servers if direct connection fails
3. Maintain encrypted communication through relay

## Troubleshooting

### Common Issues

**Connection Refused**
- Check firewall settings: `ufw status`
- Verify service is running: `systemctl status silence-relay`
- Check port binding: `ss -tulpn | grep 8080`

**High CPU Usage**
- Monitor connection count in logs
- Consider increasing `MAX_CLIENTS` limit
- Check for message loops or DoS attacks

**Memory Issues**
- Monitor message sizes and connection count
- Adjust `MAX_MESSAGE_SIZE` if needed
- Check for memory leaks in logs

### Debug Mode

Run with debug logging:
```bash
RUST_LOG=debug silence-relay --port 8080
```

### Network Diagnostics

```bash
# Test basic connectivity
telnet your-server-ip 8080

# Monitor traffic
tcpdump -i any port 8080

# Check DNS resolution
dig your-server.cherryservers.net
```

## Performance Tuning

### OS Level
```bash
# Increase file descriptor limits
echo "relay soft nofile 65536" >> /etc/security/limits.conf
echo "relay hard nofile 65536" >> /etc/security/limits.conf

# Optimize TCP settings
echo 'net.core.somaxconn = 1024' >> /etc/sysctl.conf
echo 'net.ipv4.tcp_max_syn_backlog = 1024' >> /etc/sysctl.conf
```

### Application Level
- Adjust `MAX_CLIENTS` based on expected load
- Tune `MAX_MESSAGE_SIZE` for your use case
- Consider multiple relay instances behind load balancer

## Security Considerations

1. **Network Security**: Only required ports open
2. **System Security**: Service runs with minimal privileges
3. **Access Control**: SSH key-based authentication only
4. **Monitoring**: All connections and errors logged
5. **Updates**: Regular security updates via deployment script

The relay server is designed to be trustless - it cannot decrypt messages and does not store any persistent data.

## Cost Estimation

Typical Cherry Servers costs for relay deployment:

- **Small Instance** (1 core, 1GB RAM): ~$50-100/month
- **Medium Instance** (2 cores, 4GB RAM): ~$100-200/month
- **Large Instance** (4+ cores, 8GB+ RAM): ~$200+/month

Plus bandwidth costs (typically $0.10-0.30/GB)

## Contributing

1. Ensure all tests pass: `cargo test`
2. Check code formatting: `cargo fmt --check`
3. Run clippy lints: `cargo clippy`
4. Test deployment on Cherry Servers
5. Update documentation as needed

## License

Same as parent Silence project.