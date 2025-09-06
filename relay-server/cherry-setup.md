# Cherry Servers Setup Guide for Silence Relay

## Prerequisites

1. **Cherry Servers Account**: Sign up at https://portal.cherryservers.com/
2. **cherryctl CLI**: Install the Cherry Servers CLI tool
3. **SSH Keys**: Generate and configure SSH keys for server access

## Step 1: Install cherryctl CLI

### macOS
```bash
brew install cherryservers/tap/cherryctl
```

### Linux
```bash
curl -sSL https://github.com/cherryservers/cherryctl/releases/latest/download/cherryctl_linux_amd64.tar.gz | tar -xz
sudo mv cherryctl /usr/local/bin/
```

### Configure cherryctl
```bash
# Set your API token (get from Cherry Servers portal)
export CHERRY_AUTH_TOKEN="your-api-token-here"

# Or configure permanently
cherryctl config set --api-token="your-api-token-here"
```

## Step 2: Provision Cherry Server

### List available server configurations
```bash
cherryctl server-plans
```

### Create a new server (example with small instance)
```bash
# Create server
cherryctl server create \
  --plan="e5_1620v4" \
  --image="ubuntu_20_04" \
  --region="EU-Nord-1" \
  --hostname="silence-relay" \
  --ssh-keys="your-ssh-key-id"

# List servers to get IP address
cherryctl servers
```

### Get server connection details
```bash
# Get server info including IP address
cherryctl server get --server-id="your-server-id"
```

## Step 3: Initial Server Configuration

### Connect to server
```bash
ssh root@YOUR_SERVER_IP
```

### Update system and install dependencies
```bash
# Update system
apt update && apt upgrade -y

# Install essential packages
apt install -y curl wget git build-essential ufw fail2ban

# Configure firewall
ufw default deny incoming
ufw default allow outgoing
ufw allow ssh
ufw allow 8080/tcp  # Relay server port
ufw --force enable

# Configure fail2ban for SSH protection
systemctl enable fail2ban
systemctl start fail2ban
```

## Step 4: Deploy Relay Server

### Option A: Using the deploy script (recommended)
```bash
# On your local machine, configure environment
export CHERRY_HOST="YOUR_SERVER_IP"
export CHERRY_USER="root"
export SSH_KEY="~/.ssh/id_rsa"
export RELAY_PORT="8080"

# Run deployment
cd relay-server
./deploy.sh
```

### Option B: Manual deployment
```bash
# Build locally
cargo build --release

# Copy to server
scp target/release/silence-relay root@YOUR_SERVER_IP:/tmp/

# SSH to server and install
ssh root@YOUR_SERVER_IP
mkdir -p /opt/silence-relay
mv /tmp/silence-relay /opt/silence-relay/
chmod +x /opt/silence-relay/silence-relay

# Create relay user
useradd --system --home /opt/silence-relay --shell /bin/false relay
chown -R relay:relay /opt/silence-relay

# Create systemd service (copy from deploy script)
# Start service
systemctl start silence-relay
systemctl enable silence-relay
```

## Step 5: Verify Deployment

### Check service status
```bash
systemctl status silence-relay
```

### View logs
```bash
journalctl -u silence-relay -f
```

### Test connectivity
```bash
# From another machine
telnet YOUR_SERVER_IP 8080
```

## Step 6: Security Hardening

### Configure SSH security
```bash
# Edit SSH config
nano /etc/ssh/sshd_config

# Recommended settings:
# PermitRootLogin no  # After creating regular user
# PasswordAuthentication no
# PubkeyAuthentication yes
# Port 22  # Consider changing default port

# Restart SSH
systemctl restart ssh
```

### Set up monitoring (optional)
```bash
# Install basic monitoring
apt install -y htop iotop nethogs

# Set up log rotation for relay server
cat > /etc/logrotate.d/silence-relay << EOF
/var/log/silence-relay.log {
    daily
    rotate 7
    compress
    delaycompress
    missingok
    notifempty
    copytruncate
}
EOF
```

## Step 7: Client Configuration

Update your P2P clients to use the relay server:

```rust
// In your client code, configure relay endpoint
const RELAY_SERVER: &str = "YOUR_SERVER_IP:8080";

// Modify connection logic to route through relay
// when direct P2P connection fails
```

## Monitoring and Maintenance

### Check server resources
```bash
# CPU and memory usage
htop

# Network connections
netstat -tulpn | grep :8080

# Service logs
journalctl -u silence-relay --since "1 hour ago"
```

### Update relay server
```bash
# Build new version locally and redeploy
cd relay-server
./deploy.sh

# Or update manually:
# 1. Stop service
# 2. Replace binary
# 3. Start service
```

## Troubleshooting

### Common issues
1. **Connection refused**: Check firewall settings and service status
2. **Permission denied**: Verify relay user permissions
3. **High CPU usage**: Monitor client connections and adjust limits

### Debug commands
```bash
# Check open connections
ss -tulpn | grep :8080

# Monitor system resources
top -p $(pgrep silence-relay)

# Network traffic
tcpdump -i any port 8080
```

## Cost Optimization

- **Start small**: Use lower-tier instances for testing
- **Monitor usage**: Track bandwidth and connection counts
- **Scale as needed**: Cherry Servers allows easy instance upgrades
- **Consider multiple regions**: Deploy relays closer to user bases