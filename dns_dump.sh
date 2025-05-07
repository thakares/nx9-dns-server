#!/bin/bash
DOMAIN=$1
RECORDS="A AAAA MX TXT CNAME NS SOA PTR"

for TYPE in $RECORDS; do
    echo "== $TYPE records for $DOMAIN =="
    dig $DOMAIN $TYPE 
    echo
done
