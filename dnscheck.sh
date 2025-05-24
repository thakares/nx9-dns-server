#!/bin/bash

DOMAIN="bzo.in"
DNS_SERVER="192.168.1.200"

echo "=== DNS Diagnostic Report for $DOMAIN using $DNS_SERVER ==="
echo

# DNSKEY with DNSSEC
echo ">>> DNSKEY (with DNSSEC):"
dig @"$DNS_SERVER" "$DOMAIN." DNSKEY +dnssec 
echo

# DS record
echo ">>> DS Record:"
dig @"$DNS_SERVER" "$DOMAIN." DS 
echo

# NS record
echo ">>> NS Record:"
dig @"$DNS_SERVER" "$DOMAIN." NS 
echo

# MX record
echo ">>> MX Record:"
dig @"$DNS_SERVER" "$DOMAIN." MX 
echo

# SOA record
echo ">>> SOA Record:"
dig @"$DNS_SERVER" "$DOMAIN." SOA 
echo

# A record
echo ">>> A Record:"
dig @"$DNS_SERVER" "$DOMAIN." A 
echo

# Optional: check AAAA (IPv6) record
echo ">>> AAAA (IPv6) Record:"
dig @"$DNS_SERVER" "$DOMAIN." AAAA 
echo

echo "=== End of DNS Report ==="
