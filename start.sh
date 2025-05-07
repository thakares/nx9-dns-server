#!/bin/bash
export DNS_BIND=0.0.0.0:53
export DNS_ENABLE_IPV6=1
export DNS_MAX_PACKET_SIZE=4096
export DNS_DB_PATH=/var/dns-server/dns.db
export DNS_NS_RECORDS=ns1.yourdomain.tld.,ns2.yourdomain.tld.
export DNS_AUTHORITATIVE=1
export DNS_CACHE_TTL=300
export RUST_LOG=info
export DNS_DEFAULT_DOMAIN=yourdomain.tld
export DNS_DEFAULT_IP=<YOUR-PUBLIC-IP4>

exec /var/dns-server/nx9-dns_server
