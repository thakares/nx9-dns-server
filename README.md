# nx9-dns-server

**nx9-dns-server** is a high-performance, RFC-compliant authoritative DNS server implemented in Rust. It is designed for any domain (e.g., `anydomain.tld`), supporting a wide range of DNS record types, DNSSEC, and robust operational features. The server is optimized for reliability, security, and ease of deployment in production environments.

---

## Table of Contents

- [Features](#features)
- [Architecture](#architecture)
- [DNS Record Management](#dns-record-management)
- [Web UI](#web-ui)
- [API Service](#api-service)
- [User Management](#user-management)
- [DNSSEC Support](#dnssec-support)
- [How to Create DNSSEC_KEY_FILE](#how-to-create-dnssec_key_file)
- [Deployment](#deployment)
- [Configuration](#configuration)
- [Testing & Diagnostics](#testing--diagnostics)
- [Roadmap](#roadmap)
- [Contributing](#contributing)
- [License](#license)
- [Acknowledgements](#acknowledgements)

---

## Features

- **Authoritative DNS**: Serves authoritative responses for all queries to your domain (e.g., `anydomain.tld`).
- **Multi-Record Support**: Handles A, AAAA, MX, NS, SOA, PTR, TXT, and CNAME records.
- **DNSSEC Ready**: Supports DNSSEC key management and secure record signing.
- **High Performance**: Asynchronous networking (UDP/TCP) via Tokio for handling thousands of concurrent queries.
- **RFC Compliance**: Strict adherence to DNS protocol standards for interoperability.
- **Extensible Storage**: Uses SQLite for DNS record storage, allowing easy updates and migrations.
- **Easy Deployment**: Includes deployment and update scripts for smooth operational workflows.
- **Comprehensive Logging**: Integrates with `env_logger` for detailed runtime diagnostics.
- **Web Interface**: (Coming soon) Administrative web UI for DNS record management.
- **API Service**: (Coming soon) RESTful API service for programmatic DNS record management.
- **User Management**: (Coming soon) Multi-user access control with role-based permissions.

---

## Architecture

- **Language**: Rust (2021 edition)
- **Async Runtime**: [Tokio](https://tokio.rs/)
- **Database**: SQLite via [rusqlite](https://crates.io/crates/rusqlite)
- **Logging**: [log](https://crates.io/crates/log) and [env_logger](https://crates.io/crates/env_logger)
- **Error Handling**: [thiserror](https://crates.io/crates/thiserror)
- **DNSSEC**: Built-in support for key loading and RRSIG/DS/DNSKEY records
- **Web Framework**: (Coming soon) [Rocket](https://rocket.rs/) or [Axum](https://github.com/tokio-rs/axum) for UI and API endpoints
- **Authentication**: (Coming soon) JWT-based authentication and role-based authorization

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
('anydomain.tld', 'A', '203.0.113.10', 3600),
('anydomain.tld', 'MX', '10 mail.anydomain.tld', 3600),
('anydomain.tld', 'NS', 'ns1.anydomain.tld', 3600),
('anydomain.tld', 'NS', 'ns2.anydomain.tld', 3600),
('anydomain.tld', 'SOA', 'ns1.anydomain.tld hostmaster.anydomain.tld 1 10800 3600 604800 86400', 3600),
('anydomain.tld', 'TXT', '"v=spf1 a mx ~all"', 3600),
('www.anydomain.tld', 'A', '203.0.113.10', 3600);
```

---

## Web UI

> 🚧 **Under Development - Seeking Contributors!** 🚧
>
> We're actively looking for community contributions to our Web UI implementation. If you have experience with Rust web frameworks (Rocket/Axum) and modern frontend technologies (React/Vue/Svelte), please consider contributing!

The planned Web UI will provide:

- **Dashboard**: Visual overview of DNS zone statistics and recent queries
- **Record Management**: Intuitive interface for creating, viewing, updating, and deleting DNS records
- **DNSSEC Management**: UI for key generation, rotation, and signature verification
- **Audit Logging**: Visual timeline of all record changes with user attribution
- **Responsive Design**: Mobile-friendly interface for management on any device

**Tech Stack (Proposed):**
- Backend: Rust with Rocket or Axum
- Frontend: TypeScript with React or Svelte
- Authentication: JWT-based with session management

**Contribution Areas:**
- UI/UX design mockups
- Frontend component development
- API integration
- Automated testing
- Documentation

If interested in contributing, please open an issue discussing your implementation approach before submitting PRs.

---

## API Service

> 🚧 **Under Development - Seeking Contributors!** 🚧
>
> We're building a RESTful API service for programmatic DNS record management. Contributors with experience in API design and Rust web services are welcome!

The DNS record management API will provide:

- **Full CRUD Operations**: Create, read, update, and delete DNS records via REST endpoints
- **Batch Operations**: Support for bulk record changes in a single request
- **Validation**: Strict validation of record syntax and domain integrity
- **Rate Limiting**: Protection against API abuse
- **Authentication**: Secure token-based authentication with scoped permissions
- **Webhooks**: (Planned) Event notifications for record changes

**Planned Endpoints:**
```
GET    /api/v1/zones                  # List all zones
POST   /api/v1/zones                  # Create new zone
GET    /api/v1/zones/{zone}           # Get zone details
PUT    /api/v1/zones/{zone}           # Update zone properties
DELETE /api/v1/zones/{zone}           # Remove zone

GET    /api/v1/zones/{zone}/records               # List all records in zone
POST   /api/v1/zones/{zone}/records               # Create new record
GET    /api/v1/zones/{zone}/records/{id}          # Get record details
PUT    /api/v1/zones/{zone}/records/{id}          # Update record
DELETE /api/v1/zones/{zone}/records/{id}          # Remove record

POST   /api/v1/zones/{zone}/records/batch         # Batch create/update/delete
```

If you're interested in contributing to the API service, please refer to our API design document in the project wiki.

---

## User Management

> 🚧 **Under Development - Community Input Requested!** 🚧
>
> We're designing a user management system and need input from the community on requirements and features.

Planned user management features:

- **Multi-User Support**: Multiple administrator and operator accounts
- **Role-Based Access Control**: Granular permissions for different user roles
- **Authentication Options**: Local accounts and potential OAuth/LDAP integration
- **Audit Trail**: Comprehensive logging of all user actions
- **Password Policies**: Configurable password requirements and rotation policies
- **Two-Factor Authentication**: Additional security layer for administrative access
- **API Tokens**: Management of scoped API tokens for programmatic access

**User Roles (Proposed):**
- **Administrator**: Full system access
- **Operator**: Can manage DNS records but not system settings
- **Viewer**: Read-only access to records and statistics
- **API Client**: Programmatic access via API tokens

**We welcome community input on:**
- Authentication mechanisms
- Additional role definitions and permission scopes
- UI/UX design for user management interfaces
- Enterprise integration requirements

Please open an issue with the tag `user-management` to share your feedback and requirements.

---

## DNSSEC Support

- **Key Management**: DNSSEC keys are loaded from environment-configured paths.
- **Record Signing**: Supports RRSIG, DS, and DNSKEY records for secure, signed DNS responses.
- **Preprocessing**: Key files can be preprocessed using provided scripts before deployment.

---

## How to Create `DNSSEC_KEY_FILE`

To enable DNSSEC for `nx9-dns-server`, you need to generate a DNSSEC key pair and provide the public key file to the server via the `DNSSEC_KEY_FILE` environment variable. Here's how you can do it using [BIND's dnssec-keygen tool](https://bind9.readthedocs.io/en/latest/reference.html#dnssec-keygen):

### 1. Install `dnssec-keygen`

On most Linux systems, you can install it via the package manager:

```bash
sudo apt-get install bind9-dnsutils   # Debian/Ubuntu
# or
sudo yum install bind-utils           # CentOS/RHEL
```

### 2. Generate DNSSEC Key Pair

Run the following command to generate a 2048-bit RSA key for your domain (replace `anydomain.tld` with your actual domain):

```bash
dnssec-keygen -a RSASHA256 -b 2048 -n ZONE anydomain.tld
```

- This will produce two files in your current directory:
  - `K.+008+.key` (public key)
  - `K.+008+.private` (private key)

### 3. Set the `DNSSEC_KEY_FILE` Environment Variable

Copy the public key file (`.key`) to your server's key directory (e.g., `/var/nx9-dns-server/`):

```bash
cp Kanydomain.tld.+008+24550.key /var/nx9-dns-server/
```

Then, set the environment variable in your deployment environment or systemd service:

```bash
export DNSSEC_KEY_FILE="/var/nx9-dns-server/Kanydomain.tld.+008+24550.key"
```

Or in your systemd unit file:
```
Environment="DNSSEC_KEY_FILE=/var/nx9-dns-server/Kanydomain.tld.+008+24550.key"
```

### 4. (Optional) Preprocess the Key

If your deployment uses a preprocessing script (as referenced in your `deploy.sh`), run:

```bash
sudo chmod +x /var/nx9-dns-server/preprocess-key.sh
sudo -u dnsuser /var/nx9-dns-server/preprocess-key.sh
```
This may normalize the key format or permissions as required by your server.

### 5. Restart the DNS Server

After setting the key file, restart your DNS server to load the new key:

```bash
sudo systemctl restart dns-server.service
```

### 6. Verify DNSSEC is Working

Use the provided `dnscheck.sh` script or `dig` to verify DNSSEC records:

```bash
bash dnscheck.sh
# or manually:
dig @localhost anydomain.tld DNSKEY +dnssec
```

**Note:**  
- Keep your `.private` key file secure and never expose it publicly.
- Only the `.key` (public) file should be referenced by the server.
- The server will load and use the public key for signing DNS responses.

---

## Deployment

Deployment is automated and robust, using the provided [`deploy.sh`](deploy.sh) script. This script handles permissions, key preprocessing, SOA updates, binary replacement, and service management.

**Typical deployment steps:**
```bash
#!/bin/bash

set -e

SRC_BIN="/home/youruser/apps/your-ddns/dns_server"
DEST_DIR="/var/nx9-dns-server"
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
- `WEB_UI_BIND`: Bind address for web interface (default: `127.0.0.1:8080`)
- `API_BIND`: Bind address for API service (default: `127.0.0.1:8081`)
- `AUTH_SECRET`: Secret key for JWT token signing
- `ADMIN_PASSWORD`: Initial admin password (only used if no users exist)

**Example:**
```bash
export DNS_BIND="0.0.0.0:53"
export DNS_DB_PATH="/var/nx9-dns-server/dns.db"
export DNSSEC_KEY_FILE="/var/nx9-dns-server/Kanydomain.tld.+008+24550.key"
export DNS_FORWARDERS="8.8.8.8:53,1.1.1.1:53"
export DNS_NS_RECORDS="ns1.anydomain.tld.,ns2.anydomain.tld."
export WEB_UI_BIND="0.0.0.0:8080"
export API_BIND="0.0.0.0:8081"
export AUTH_SECRET="your-secure-random-string-here"
```

---

## Testing & Diagnostics

A suite of shell scripts is provided for diagnostics and record verification:

- **dnscheck.sh**: Runs a series of `dig` queries for all major record types and DNSSEC.
- **dns_dump.sh**: Dumps all record types for a given domain.
- **api_test.sh**: (Coming soon) Tests the API endpoints with sample requests.
- **performance_test.sh**: (Coming soon) Benchmarks server performance under load.

**Example usage:**
```bash
bash dnscheck.sh
bash dns_dump.sh anydomain.tld
```

---

## Roadmap

Our planned features and improvements:

### Short-term (1-3 months)
- [x] Core DNS server functionality
- [x] DNSSEC implementation
- [ ] Web UI development (in progress)
- [ ] RESTful API service (in progress)
- [ ] User management system (planning)
- [ ] Docker container support

### Medium-term (3-6 months)
- [ ] Clustered deployment support
- [ ] Metrics and monitoring integration (Prometheus)
- [ ] Zone transfer (AXFR/IXFR) support
- [ ] Dynamic DNS update protocol (RFC 2136)
- [ ] DNSSEC key rotation automation

### Long-term (6+ months)
- [ ] Secondary/slave DNS server support
- [ ] Geo-based DNS responses
- [ ] DNS over HTTPS (DoH) support
- [ ] DNS over TLS (DoT) support
- [ ] Record templating system

---

## Contributing

Contributions, bug reports, and feature requests are welcome! Please open issues or pull requests via GitHub.

### Priority Contribution Areas
We're actively seeking contributions in these areas:

1. **Web UI Development**: Frontend components and integration with the backend
2. **API Service**: RESTful API implementation for DNS record management
3. **User Management**: Authentication, authorization, and user interface
4. **Documentation**: Improving guides and examples
5. **Testing**: Unit tests, integration tests, and automated CI pipelines

### How to Contribute
1. Fork the repository
2. Create a feature branch: `git checkout -b feature/amazing-feature`
3. Commit your changes: `git commit -m 'Add some amazing feature'`
4. Push to the branch: `git push origin feature/amazing-feature`
5. Open a Pull Request

Please see [CONTRIBUTING.md](CONTRIBUTING.md) for detailed contribution guidelines.

---

## License

This project is licensed under the [GNU General Public License v3.0 (GPLv3)](LICENSE).

---

## Acknowledgements

- [Tokio](https://tokio.rs/) for async runtime
- [rusqlite](https://crates.io/crates/rusqlite) for SQLite integration
- [dig](https://linux.die.net/man/1/dig) for DNS diagnostics
- Community contributors and supporters

---

**nx9-dns-server** is developed and maintained by [Your Name or Organization].  
For more information, see the source code or contact the maintainer via GitHub.

---

**Tip:**  
Replace `anydomain.tld` with your actual domain throughout the configuration and database files.
