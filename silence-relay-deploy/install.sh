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