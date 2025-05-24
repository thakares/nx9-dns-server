#!/bin/bash
set -e

# Paths
SRC_BIN="/home/<user-name>/apps/nx9-dns-server/dns_server"
DEST_DIR="/var/nx9-dns-server"
DEST_BIN="$DEST_DIR/nx9-dns_server"
PREPROCESS_SCRIPT="$DEST_DIR/preprocess-key.sh"
SOA_UPDATE_SCRIPT="$DEST_DIR/soa-update.sh"

echo "🔐 Fixing permissions and running preprocess..."
sudo chmod +x "$PREPROCESS_SCRIPT"
sudo -u dnsuser "$PREPROCESS_SCRIPT"

echo "🛠 Updating SOA record..."
sudo chown dnsuser:dnsuser "$SOA_UPDATE_SCRIPT"
sudo chmod +x "$SOA_UPDATE_SCRIPT"
sudo -u dnsuser "$SOA_UPDATE_SCRIPT"

echo "📄 Verifying processed.key content..."
sudo cat "$DEST_DIR/processed.key"

echo "🛑 Stopping DNS server..."
sudo systemctl stop dns-server.service

echo "📦 Deploying new dns_server binary..."
sudo cp "$SRC_BIN" "$DEST_BIN"
sudo chown dnsuser:dnsuser "$DEST_DIR"

echo "🔁 Reloading systemd and restarting service..."
sudo systemctl daemon-reload
sudo systemctl restart dns-server.service

echo "📈 Checking service status..."
sudo systemctl status dns-server.service
