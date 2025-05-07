# Create a file at /var/nx9-dns-server/dns_wrapper.sh
#!/bin/bash
# Log start with timestamp
echo "Starting DNS server at $(date)" >> /var/log/nx9-dns_server_wrapper.log

# Export all environment variables explicitly
export DNS_DB_PATH=/var/nx9-dns-server/dns.db
export DNS_BIND=0.0.0.0:53
export DNS_ENABLE_IPV6=1
export DNS_MAX_PACKET_SIZE=4096
export DNS_NS_RECORDS=ns1.yourdomain.tld.,ns2.yourdomain.tld.
export DNS_AUTHORITATIVE=1
export DNS_CACHE_TTL=300
export RUST_LOG=info
export DNS_DEFAULT_DOMAIN=yourdomain.tld
export DNS_DEFAULT_IP=<YOUR-PUBLIC-IP4>

# Print environment for debugging
env >> /var/log/nx9-dns-server_wrapper.log

# Run the DNS server
/var/dns-server/nx9-dns-server
