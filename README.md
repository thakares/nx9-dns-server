# nx9-dns-server

**nx9-dns-server** is a high-performance, RFC-compliant authoritative DNS server implemented in Rust. It is designed for the `bzo.in` domain, supporting a wide range of DNS record types, DNSSEC, and robust operational features. The server is optimized for reliability, security, and ease of deployment in production environments.

---

## Table of Contents

- [Features](#features)
- [Architecture](#architecture)
- [DNS Record Management](#dns-record-management)
- [DNSSEC Support](#dnssec-support)
- [Deployment](#deployment)
- [Configuration](#configuration)
- [Testing & Diagnostics](#testing--diagnostics)
- [License](#license)
- [Contributing](#contributing)
- [Acknowledgements](#acknowledgements)

---

## Features

- **Authoritative DNS**: Serves authoritative responses for all `bzo.in` domain queries.
- **Multi-Record Support**: Handles A, AAAA, MX, NS, SOA, PTR, TXT, and CNAME records.
- **DNSSEC Ready**: Supports DNSSEC key management and secure record signing.
- **High Performance**: Asynchronous networking (UDP/TCP) via Tokio for handling thousands of concurrent queries.
- **RFC Compliance**: Strict adherence to DNS protocol standards for interoperability.
- **Extensible Storage**: Uses SQLite for DNS record storage, allowing easy updates and migrations.
- **Easy Deployment**: Includes deployment and update scripts for smooth operational workflows.
- **Comprehensive Logging**: Integrates with `env_logger` for detailed runtime diagnostics.

---

## Architecture

- **Language**: Rust (2021 edition)
- **Async Runtime**: [Tokio](https://tokio.rs/)
- **Database**: SQLite via [rusqlite](https://crates.io/crates/rusqlite)
- **Logging**: [log](https://crates.io/crates/log) and [env_logger](https://crates.io/crates/env_logger)
- **Error Handling**: [thiserror](https://crates.io/crates/thiserror)
- **DNSSEC**: Built-in support for key loading and RRSIG/DS/DNSKEY records

---

## DNS Record Management

DNS records are managed in an SQLite database (`dns.db`). The schema supports multiple records per domain and type, and can be easily updated using SQL scripts.

**Example schema (`dns_records.sql`):**
```sql
CREATE TABLE IF NOT EXISTS dns_records (
  domain TEXT NOT NULL,
  record_type TEXT NOT NULL,
  value TEXT NOT NULL,
  ttl INTEGER DEFAULT 3600,
  PRIMARY KEY (domain, record_type, value)
) WITHOUT ROWID;
```

**Sample records:**
```sql
INSERT OR REPLACE INTO dns_records VALUES
('bzo.in', 'A', '60.254.61.33', 3600),
('bzo.in', 'MX', '10 mail.bzo.in', 3600),
('bzo.in', 'NS', 'ns1.bzo.in', 3600),
('bzo.in', 'NS', 'ns2.bzo.in', 3600),
('bzo.in', 'SOA', 'ns1.bzo.in hostmaster.bzo.in 1 10800 3600 604800 86400', 3600),
('bzo.in', 'TXT', '"v=spf1 a mx ~all"', 3600);
```

---

## DNSSEC Support

- **Key Management**: DNSSEC keys are loaded from environment-configured paths.
- **Record Signing**: Supports RRSIG, DS, and DNSKEY records for secure, signed DNS responses.
- **Preprocessing**: Key files can be preprocessed using provided scripts before deployment.

---

## Deployment

Deployment is automated and robust, using the provided [`deploy.sh`](deploy.sh) script. This script handles permissions, key preprocessing, SOA updates, binary replacement, and service management.

**Typical deployment steps:**
```bash
#!/bin/bash

set -e

SRC_BIN="/home/sunil/apps/bzo-ddns/dns_server"
DEST_DIR="/var/dns-server"
DEST_BIN="$DEST_DIR/dns_server"
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
```
See [`deploy.sh`](deploy.sh) for the full deployment script.

---

## Configuration

Configuration is environment-driven and highly flexible.

**Key environment variables:**
- `DNS_BIND`: Bind address (default: `0.0.0.0:53`)
- `DNS_DB_PATH`: Path to the SQLite database (default: `dns.db`)
- `DNSSEC_KEY_FILE`: Path to DNSSEC key file
- `DNS_FORWARDERS`: Comma-separated list of upstream DNS resolvers
- `DNS_NS_RECORDS`: Comma-separated list of NS records
- `DNS_CACHE_TTL`: Cache TTL in seconds

**Example:**
```bash
export DNS_BIND="0.0.0.0:53"
export DNS_DB_PATH="/var/dns-server/dns.db"
export DNSSEC_KEY_FILE="/var/dns-server/Kbzo.in.+008+24550.key"
export DNS_FORWARDERS="8.8.8.8:53,1.1.1.1:53"
export DNS_NS_RECORDS="ns1.bzo.in.,ns2.bzo.in."
```

---

## Testing & Diagnostics

A suite of shell scripts is provided for diagnostics and record verification:

- **dnscheck.sh**: Runs a series of `dig` queries for all major record types and DNSSEC.
- **dns_dump.sh**: Dumps all record types for a given domain.

**Example usage:**
```bash
bash dnscheck.sh
bash dns_dump.sh bzo.in
```

---

## License

This project is licensed under the [GNU General Public License v3.0 (GPLv3)](LICENSE).

---

## Contributing

Contributions, bug reports, and feature requests are welcome! Please open issues or pull requests via GitHub.

---

## Acknowledgements

- [Tokio](https://tokio.rs/) for async runtime
- [rusqlite](https://crates.io/crates/rusqlite) for SQLite integration
- [dig](https://linux.die.net/man/1/dig) for DNS diagnostics

---

**nx9-dns-server** is developed and maintained by [Sunil Thakare](https://github.com/thakares).  
For more information, see the source code or contact the maintainer via GitHub.

---
