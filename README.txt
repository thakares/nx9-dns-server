sudo chmod +x /var/dns-server/preprocess-key.sh
sudo -u dnsuser /var/dns-server/preprocess-key.sh
sudo chown dnsuser:dnsuser /var/dns-server/soa-update.sh
sudo chmod +x /var/dns-server/soa-update.sh
sudo -u dnsuser /var/dns-server/soa-update.sh
sudo cat /var/dns-server/processed.key  # Verify output format
sudo systemctl stop dns-server.service
sudo cp /home/sunil/apps/bzo-ddns/dns_server /var/dns-server/dns_server
sudo chown dnsuser:dnsuser /var/dns-server
sudo systemctl daemon-reload
sudo systemctl restart dns-server.service
sudo systemctl status dns-server.service


