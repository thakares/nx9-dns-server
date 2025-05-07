#!/bin/bash
export DNS_BIND=0.0.0.0:53
export DNS_ENABLE_IPV6=1
export DNS_MAX_PACKET_SIZE=4096
export DNS_DB_PATH=/var/dns-server/dns.db
export DNS_NS_RECORDS=ns1.bzo.in.,ns2.bzo.in.
export DNS_AUTHORITATIVE=1
export DNS_CACHE_TTL=300
export RUST_LOG=info
export DNS_DEFAULT_DOMAIN=bzo.in
export DNS_DEFAULT_IP=60.254.61.33

exec /var/dns-server/dns_server
