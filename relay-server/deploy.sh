#!/bin/bash
# Deployment script for Silence Relay Server on Cherry Servers

set -euo pipefail

# Configuration
RELAY_PORT=${RELAY_PORT:-8080}
MAX_CLIENTS=${MAX_CLIENTS:-100}
MAX_MESSAGE_SIZE=${MAX_MESSAGE_SIZE:-65536}
BIND_ADDRESS=${BIND_ADDRESS:-0.0.0.0}

# Cherry Server connection details (set these via environment or modify)
CHERRY_HOST=${CHERRY_HOST:-"your-server.cherryservers.net"}
CHERRY_USER=${CHERRY_USER:-"root"}
SSH_KEY=${SSH_KEY:-"~/.ssh/id_rsa"}

echo "🍒 Deploying Silence Relay Server to Cherry Servers..."
echo "Target: ${CHERRY_USER}@${CHERRY_HOST}"
echo "Port: ${RELAY_PORT}"

# Build the relay server binary
echo "📦 Building relay server..."
cargo build --release

# Create deployment directory
DEPLOY_DIR="silence-relay-deploy"
rm -rf ${DEPLOY_DIR}
mkdir -p ${DEPLOY_DIR}

# Copy binary and configuration
cp target/release/silence-relay ${DEPLOY_DIR}/
cp deploy/systemd/silence-relay.service ${DEPLOY_DIR}/ || true

# Create systemd service file
cat > ${DEPLOY_DIR}/silence-relay.service << EOF
[Unit]
Description=Silence Relay Server
After=network.target
Wants=network.target

[Service]
Type=simple
User=relay
Group=relay
WorkingDirectory=/opt/silence-relay
ExecStart=/opt/silence-relay/silence-relay --port ${RELAY_PORT} --max-clients ${MAX_CLIENTS} --max-message-size ${MAX_MESSAGE_SIZE} --bind-address ${BIND_ADDRESS}
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
EOF

# Create installation script
cat > ${DEPLOY_DIR}/install.sh << 'EOF'
#!/bin/bash
set -euo pipefail

echo "🔧 Installing Silence Relay Server..."

# Create user for the service
if ! id -u relay >/dev/null 2>&1; then
    useradd --system --home /opt/silence-relay --shell /bin/false --comment "Silence Relay Server" relay
    echo "✅ Created relay user"
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
EOF

chmod +x ${DEPLOY_DIR}/install.sh

# Create tarball
tar -czf silence-relay-deploy.tar.gz -C ${DEPLOY_DIR} .

echo "📤 Uploading to Cherry Server..."
scp -i ${SSH_KEY} silence-relay-deploy.tar.gz ${CHERRY_USER}@${CHERRY_HOST}:/tmp/

echo "🔧 Installing on Cherry Server..."
ssh -i ${SSH_KEY} ${CHERRY_USER}@${CHERRY_HOST} << 'REMOTE_SCRIPT'
cd /tmp
tar -xzf silence-relay-deploy.tar.gz
sudo ./install.sh
sudo systemctl start silence-relay

echo ""
echo "🎉 Deployment complete!"
echo "Relay server is now running on port ${RELAY_PORT:-8080}"
echo ""
echo "To check status:"
echo "  sudo systemctl status silence-relay"
echo ""
echo "To view logs:"
echo "  sudo journalctl -u silence-relay -f"
REMOTE_SCRIPT

# Clean up
rm -rf ${DEPLOY_DIR} silence-relay-deploy.tar.gz

echo "✅ Deployment complete! Relay server should now be running on ${CHERRY_HOST}:${RELAY_PORT}"