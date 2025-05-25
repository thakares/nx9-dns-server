# NX9 DNS Server Installation Guide

## Installation Methods

The NX9 DNS Server provides three flexible installation methods:

### 1. Direct Installation Script

The `install.sh` script provides a customizable installation process:

```bash
# Default installation to /usr/local
./install.sh

# Custom installation prefix
./install.sh --prefix=/opt/nx9-dns
```

#### Features:
- Configurable installation prefix
- Automatic service setup
- Documentation and man page installation
- Proper permission handling

### 2. Debian Package Installation

For Debian-based systems (Ubuntu, Debian, etc.):

```bash
# Build the package
./build-deb.sh

# Install the generated package
sudo dpkg -i nx9-dns-server-*.deb
```

#### Package Features:
- Proper dependency management
- System user creation
- Systemd service integration
- Configuration file management
- Clean uninstallation support

### 3. RPM Package Installation

For RPM-based systems (RHEL, CentOS, Fedora):

```bash
# Build the package
./build-rpm.sh

# Install the generated package
sudo rpm -i nx9-dns-server-*.rpm
```

#### Package Features:
- Automatic dependency resolution
- System service management
- Standard RPM filesystem hierarchy
- Built-in upgrade support

## Directory Structure

The installation creates the following directory structure:

```
${PREFIX}/
├── bin/
│   └── nx9-dns-server
├── lib/
│   └── nx9-dns-server/
├── etc/
│   └── nx9-dns-server/
│       └── nx9-dns-server.conf
├── share/
│   ├── doc/
│   │   └── nx9-dns-server/
│   │       ├── README.md
│   │       └── LICENSE
│   └── man/
│       └── man1/
│           └── nx9-dns-server.1.gz
└── var/
    └── lib/
        └── nx9-dns-server/
            └── dns.db
```

## System Integration

### Service Management
After installation, the server runs as a systemd service:

```bash
# Start the service
sudo systemctl start nx9-dns-server

# Enable auto-start at boot
sudo systemctl enable nx9-dns-server

# Check service status
sudo systemctl status nx9-dns-server
```

### Security Features
- Dedicated system user (`nx9-dns`)
- Proper file permissions
- Confined service execution

### Configuration
- Main configuration: `/etc/nx9-dns-server/nx9-dns-server.conf`
- Database location: `/var/lib/nx9-dns-server/dns.db`
- DNSSEC key file: `/etc/nx9-dns-server/dnssec.key`

## Uninstallation

### Using Installation Script
```bash
# If installed with install.sh, manually remove files from prefix
sudo rm -rf ${PREFIX}/{bin,lib,etc,share}/nx9-dns-server
```

### Package-based Removal
```bash
# Debian systems
sudo apt remove nx9-dns-server

# RPM systems
sudo rpm -e nx9-dns-server
```

## Notes
- All installation methods require root/sudo privileges
- The installation preserves existing configuration files
- Service user is created automatically
- Logs are integrated with systemd journal

This installation system follows standard Unix filesystem hierarchy and packaging conventions, making it suitable for both development and production environments. 