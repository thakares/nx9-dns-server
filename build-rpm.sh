#!/bin/bash

# Check for required tools
if ! command -v rpmbuild &> /dev/null; then
    echo "rpmbuild is required but not installed. Please install rpm-build package."
    exit 1
fi

# Package information
NAME="nx9-dns-server"
VERSION=$(grep '^version =' Cargo.toml | cut -d '"' -f2)
RELEASE="1"

# Create RPM build directory structure
mkdir -p ~/rpmbuild/{BUILD,RPMS,SOURCES,SPECS,SRPMS}

# Create the spec file
cat > ~/rpmbuild/SPECS/$NAME.spec << EOF
Name:           $NAME
Version:        $VERSION
Release:        $RELEASE%{?dist}
Summary:        High-performance DNS server with DNSSEC support
License:        MIT
URL:            https://github.com/yourusername/nx9-dns-server
BuildRequires:  cargo, gcc
Requires:       systemd

%description
NX9 DNS Server is a high-performance DNS server implementation
with full DNSSEC support. It provides both authoritative and
recursive DNS services with modern features like cache management
and metrics export.

%prep
%setup -q -c -T
cp -r %{_sourcedir}/* .

%build
cargo build --release

%install
rm -rf %{buildroot}
mkdir -p %{buildroot}/usr/local/bin
mkdir -p %{buildroot}/etc/nx9-dns-server
mkdir -p %{buildroot}/usr/lib/systemd/system
mkdir -p %{buildroot}/var/lib/nx9-dns-server
mkdir -p %{buildroot}/usr/share/doc/%{name}

# Install binary
install -m 755 target/release/nx9-dns-server %{buildroot}/usr/local/bin/

# Install config
install -m 644 config/nx9-dns-server.conf %{buildroot}/etc/nx9-dns-server/ || :

# Install docs
install -m 644 README.md LICENSE %{buildroot}/usr/share/doc/%{name}/ || :

# Install systemd service
cat > %{buildroot}/usr/lib/systemd/system/nx9-dns-server.service << 'EOL'
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
EOL

%pre
# Create nx9-dns user if it doesn't exist
if ! getent passwd nx9-dns > /dev/null; then
    useradd -r -s /sbin/nologin nx9-dns
fi

%post
%systemd_post nx9-dns-server.service
chown -R nx9-dns:nx9-dns /var/lib/nx9-dns-server
chmod 755 /var/lib/nx9-dns-server

%preun
%systemd_preun nx9-dns-server.service

%postun
%systemd_postun_with_restart nx9-dns-server.service

%files
%attr(755,root,root) /usr/local/bin/nx9-dns-server
%dir %attr(755,nx9-dns,nx9-dns) /var/lib/nx9-dns-server
%dir %attr(755,root,root) /etc/nx9-dns-server
%config(noreplace) /etc/nx9-dns-server/nx9-dns-server.conf
/usr/lib/systemd/system/nx9-dns-server.service
%doc /usr/share/doc/%{name}/*

%changelog
* $(date '+%a %b %d %Y') $USER <$USER@$(hostname)> - $VERSION-$RELEASE
- Initial RPM release
EOF

# Create source tarball
mkdir -p ~/rpmbuild/SOURCES/$NAME-$VERSION
cp -r * ~/rpmbuild/SOURCES/$NAME-$VERSION/
cd ~/rpmbuild/SOURCES
tar czf $NAME-$VERSION.tar.gz $NAME-$VERSION
rm -rf $NAME-$VERSION

# Build the RPM
rpmbuild -ba ~/rpmbuild/SPECS/$NAME.spec

echo "RPM package created in ~/rpmbuild/RPMS/" 