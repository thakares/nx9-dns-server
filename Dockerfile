# Build stage
FROM rust:1.72-slim-bookworm AS builder

# Install necessary build dependencies
RUN apt-get update && apt-get install -y \
    musl-tools \
    build-essential \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Add support for cross-compilation to Alpine
RUN rustup target add x86_64-unknown-linux-musl

# Create a new empty project
WORKDIR /app
COPY . .

# Build the project with musl target
RUN cargo build --target x86_64-unknown-linux-musl --release

# Runtime stage
FROM alpine:3.18

# Install runtime dependencies
RUN apk --no-cache add ca-certificates sqlite tzdata

# Create a non-root user for running the application
RUN addgroup -S dns && adduser -S dnsuser -G dns

# Create necessary directories
RUN mkdir -p /var/nx9-dns-server /var/log/nx9-dns-server /etc/nx9-dns-server
RUN chown -R dnsuser:dns /var/nx9-dns-server /var/log/nx9-dns-server /etc/nx9-dns-server

# Copy the compiled binary
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/dns_server /usr/local/bin/
RUN chmod +x /usr/local/bin/dns_server

# Copy configuration files
COPY --from=builder /app/conf/dns_records.sql /etc/nx9-dns-server/
COPY --from=builder /app/conf/dns.db.sample /etc/nx9-dns-server/

# Expose DNS ports
EXPOSE 53/udp 53/tcp
# Expose Web UI port
EXPOSE 8080/tcp
# Expose API port
EXPOSE 8081/tcp

# Set working directory
WORKDIR /var/nx9-dns-server

# Switch to non-root user
USER dnsuser

# Command to run the application
CMD ["/usr/local/bin/dns_server"]