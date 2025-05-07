#!/bin/bash

INPUT_FILE="/var/dns-server/Kbzo.in.+008+24550.key"
OUTPUT_FILE="/var/dns-server/processed.key"

# Extract and clean the DNSKEY record
DNSKEY_RECORD=$(grep -E '^[^;].*IN[[:space:]]+DNSKEY' "$INPUT_FILE" | head -1)

if [ -z "$DNSKEY_RECORD" ]; then
    echo "Error: No valid DNSKEY record found" >&2
    exit 1
fi

# Format: "domain. IN DNSKEY flags protocol algorithm key"
echo "$DNSKEY_RECORD" | awk '{
    # Remove comments and extra spaces
    gsub(/;.*$/, "");
    gsub(/[[:space:]]+/, " ");
    # Reconstruct with clean base64
    printf "%s %s %s %s %s %s ", $1, $2, $3, $4, $5, $6;
    # Print key without any whitespace
    for (i=7; i<=NF; i++) printf "%s", $i;
    print "";
}' > "$OUTPUT_FILE"

exit 0
