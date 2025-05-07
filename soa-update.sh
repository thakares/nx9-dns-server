#!/bin/bash
DB_PATH="/var/nx9-dns-server/dns.db"
ZONE="yourdomain.tld"

# Get today's date and an incremental suffix
DATE=$(date +%Y%m%d)
SERIAL_SUFFIX="01"
NEW_SERIAL="${DATE}${SERIAL_SUFFIX}"

# Update SOA value
sqlite3 "$DB_PATH" <<EOF
UPDATE dns_records
SET value = 'ns1.yourdomain.tld hostmaster.yourdomain.tld $NEW_SERIAL 10800 3600 604800 86400'
WHERE domain = '$ZONE' AND record_type = 'SOA';
EOF

echo "SOA record updated with serial: $NEW_SERIAL"
