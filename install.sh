#!/bin/bash

# Default installation prefix
PREFIX=${PREFIX:-/usr/local}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --prefix=*)
        PREFIX="${1#*=}"
        shift
        ;;
        --help)
        echo "Usage: $0 [--prefix=/installation/path]"
        echo "Default prefix is /usr/local"
        exit 0
        ;;
        *)
        echo "Unknown option: $1"
        exit 1
        ;;
    esac
done

# Build the project
echo "Building nx9-dns-server..."
cargo build --release

# Create necessary directories
echo "Creating directories in $PREFIX..."
sudo mkdir -p "$PREFIX/bin"
sudo mkdir -p "$PREFIX/lib/nx9-dns-server"
sudo mkdir -p "$PREFIX/etc/nx9-dns-server"
sudo mkdir -p "$PREFIX/share/doc/nx9-dns-server"
sudo mkdir -p "$PREFIX/share/man/man1"

# Install binary
echo "Installing binary..."
sudo install -m 755 target/release/nx9-dns-server "$PREFIX/bin/"

# Install configuration
echo "Installing configuration files..."
sudo install -m 644 config/nx9-dns-server.conf "$PREFIX/etc/nx9-dns-server/" 2>/dev/null || echo "No config file found, skipping..."

# Install documentation
echo "Installing documentation..."
sudo install -m 644 README.md "$PREFIX/share/doc/nx9-dns-server/" 2>/dev/null || echo "No README found, skipping..."
sudo install -m 644 LICENSE "$PREFIX/share/doc/nx9-dns-server/" 2>/dev/null || echo "No LICENSE found, skipping..."

# Install man page if it exists
if [ -f doc/nx9-dns-server.1 ]; then
    echo "Installing man page..."
    sudo install -m 644 doc/nx9-dns-server.1 "$PREFIX/share/man/man1/"
    sudo gzip -f "$PREFIX/share/man/man1/nx9-dns-server.1"
fi

# Create systemd service file
echo "Creating systemd service file..."
cat > nx9-dns-server.service << EOF
[Unit]
Description=NX9 DNS Server
After=network.target

[Service]
ExecStart=$PREFIX/bin/nx9-dns-server
Environment=DNS_DB_PATH=/var/lib/nx9-dns-server/dns.db
Environment=DNSSEC_KEY_FILE=/etc/nx9-dns-server/dnssec.key
Restart=always
User=nx9-dns

[Install]
WantedBy=multi-user.target
EOF

sudo install -m 644 nx9-dns-server.service /lib/systemd/system/
rm nx9-dns-server.service

echo "Installation completed successfully!"
echo "To start the service:"
echo "1. Create nx9-dns user: sudo useradd -r -s /bin/false nx9-dns"
echo "2. Enable the service: sudo systemctl enable nx9-dns-server"
echo "3. Start the service: sudo systemctl start nx9-dns-server" 