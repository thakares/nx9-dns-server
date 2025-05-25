#!/bin/bash

# Check for required tools
if ! command -v dpkg-deb &> /dev/null; then
    echo "dpkg-deb is required but not installed. Please install dpkg-dev package."
    exit 1
fi

# Package information
PACKAGE="nx9-dns-server"
VERSION=$(grep '^version =' Cargo.toml | cut -d '"' -f2)
ARCH=$(dpkg --print-architecture)
MAINTAINER="Sunil Purushottam Thakare"
DESCRIPTION="High-performance DNS server with DNSSEC support"

# Create package directory structure
PKGDIR="$PACKAGE-$VERSION"
mkdir -p "$PKGDIR/DEBIAN"
mkdir -p "$PKGDIR/usr/local/bin"
mkdir -p "$PKGDIR/etc/nx9-dns-server"
mkdir -p "$PKGDIR/usr/lib/systemd/system"
mkdir -p "$PKGDIR/var/lib/nx9-dns-server"
mkdir -p "$PKGDIR/usr/share/doc/$PACKAGE"

# Build the project
cargo build --release

# Create control file
cat > "$PKGDIR/DEBIAN/control" << EOF
Package: $PACKAGE
Version: $VERSION
Architecture: $ARCH
Maintainer: $MAINTAINER
Depends: libc6 (>= 2.17), adduser
Section: net
Priority: optional
Homepage: https://github.com/yourusername/nx9-dns-server
Description: $DESCRIPTION
 NX9 DNS Server is a high-performance DNS server implementation
 with full DNSSEC support. It provides both authoritative and
 recursive DNS services with modern features like cache management
 and metrics export.
EOF

# Create postinst script
cat > "$PKGDIR/DEBIAN/postinst" << EOF
#!/bin/sh
set -e

# Create nx9-dns user if it doesn't exist
if ! getent passwd nx9-dns > /dev/null; then
    adduser --system --group --no-create-home --home /nonexistent nx9-dns
fi

# Set permissions
chown -R nx9-dns:nx9-dns /var/lib/nx9-dns-server
chmod 755 /var/lib/nx9-dns-server

# Enable and start service
if [ -x "/bin/systemctl" ]; then
    systemctl daemon-reload
    systemctl enable nx9-dns-server.service || true
    systemctl start nx9-dns-server.service || true
fi
EOF

# Create prerm script
cat > "$PKGDIR/DEBIAN/prerm" << EOF
#!/bin/sh
set -e

if [ -x "/bin/systemctl" ]; then
    systemctl stop nx9-dns-server.service || true
    systemctl disable nx9-dns-server.service || true
fi
EOF

# Set permissions for maintainer scripts
chmod 755 "$PKGDIR/DEBIAN/postinst"
chmod 755 "$PKGDIR/DEBIAN/prerm"

# Copy files
cp target/release/nx9-dns-server "$PKGDIR/usr/local/bin/"
cp config/nx9-dns-server.conf "$PKGDIR/etc/nx9-dns-server/" 2>/dev/null || echo "No config file found, skipping..."
cp README.md LICENSE "$PKGDIR/usr/share/doc/$PACKAGE/" 2>/dev/null || echo "No documentation files found, skipping..."

# Create systemd service file
cat > "$PKGDIR/usr/lib/systemd/system/nx9-dns-server.service" << EOF
[Unit]
Description=NX9 DNS Server
After=network.target

[Service]
ExecStart=/usr/local/bin/nx9-dns-server
Environment=DNS_DB_PATH=/var/lib/nx9-dns-server/dns.db
Environment=DNSSEC_KEY_FILE=/etc/nx9-dns-server/dnssec.key
Restart=always
User=nx9-dns

[Install]
WantedBy=multi-user.target
EOF

# Build the package
dpkg-deb --build "$PKGDIR"

# Cleanup
rm -rf "$PKGDIR"

echo "Debian package created: ${PKGDIR}.deb" 