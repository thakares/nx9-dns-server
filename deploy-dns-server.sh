#!/bin/bash

set -e  # Exit on error

echo "🔧 Making preprocess-key.sh executable..."
sudo chmod +x /var/nx9-dns-server/preprocess-key.sh

echo "👤 Running preprocess-key.sh as dnsuser..."
sudo -u dnsuser /var/nx9-dns-server/preprocess-key.sh

echo "🔧 Setting ownership and permissions for soa-update.sh..."
sudo chown dnsuser:dnsuser /var/nx9-dns-server/soa-update.sh
sudo chmod +x /var/nx9-dns-server/soa-update.sh

echo "👤 Running soa-update.sh as dnsuser..."
sudo -u dnsuser /var/nx9-dns-server/soa-update.sh

echo "📄 Checking output of processed.key..."
sudo cat /var/nx9-dns-server/processed.key

echo "🛑 Stopping dns-server.service..."
sudo systemctl stop dns-server.service

echo "📦 Deploying compiled binary to /var/nx9-dns-server..."
sudo cp /home/sunil/apps/nx9-bzo-ddns/dns_server /var/nx9-dns-server/dns_server

echo "👤 Fixing ownership of /var/nx9-dns-server..."
sudo chown dnsuser:dnsuser /var/nx9-dns-server

echo "🔄 Reloading systemd daemon and restarting service..."
sudo systemctl daemon-reload
sudo systemctl restart dns-server.service

echo "📡 Checking status of dns-server.service..."
sudo systemctl status dns-server.service
