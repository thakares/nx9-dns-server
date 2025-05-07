/*
# Copyright (C) 2025 Sunil Purushottam Thakare <sunil@thakares.com>
#
# This file (main.rs) is part of nx9_dns_server.
#
# nx9_dns_server is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.
#
# dns_server is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
# GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License
# along with dns_server.  If not, see <https://www.gnu.org/licenses/>.
*/

use std::{
    collections::HashMap,
    env,
    net::SocketAddr,
    str,
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, SystemTime},
    fs,
};
use std::net::{Ipv4Addr, SocketAddrV4};
use log::{debug, error, info, warn};
use rusqlite::{params, Connection};
use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream, UdpSocket},
    signal,
    task,
};
use hex;
use base64;
use base64::Engine;

const DEFAULT_TTL: u64 = 600;
const MAX_PACKET_SIZE: usize = 4096;
const CACHE_CLEANUP_INTERVAL: Duration = Duration::from_secs(300);

#[derive(Error, Debug)]
pub enum DnsError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("Invalid DNS packet: {0}")]
    Protocol(String),
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("Parse error: {0}")]
    Parse(#[from] std::num::ParseIntError),
    #[error("Base64 error: {0}")]
    Base64(String),
    #[error("Shutdown signal received")]
    Shutdown,
}

#[derive(Debug, Clone)]
struct ServerConfig {
    bind_addr: SocketAddr,
    db_path: String,
    cache_ttl: u64,
    enable_ipv6: bool,
    max_packet_size: usize,
    authoritative: bool,
    ns_records: Vec<String>,
    default_domain: String,
    default_ip: String,
    forwarders: Vec<SocketAddr>,
    ds_records: Vec<String>,
    dnskey_records: Vec<String>,
}

impl ServerConfig {
    fn from_env() -> Result<Self, DnsError> {
        let bind_addr = env::var("DNS_BIND")
            .unwrap_or_else(|_| "0.0.0.0:53".into())
            .parse()
            .map_err(|_| DnsError::Config("Invalid DNS_BIND address".into()))?;

        let forwarders = env::var("DNS_FORWARDERS")
            .unwrap_or_else(|_| "8.8.8.8:53,1.1.1.1:53,9.9.9.9:53".into())
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();

        let key_path = env::var("DNSSEC_KEY_FILE").unwrap_or_else(|_| "Kbzo.in.+008+24550.key".to_string());
        let dnskey_records = match fs::read_to_string(&key_path) {
            Ok(content) => {
                info!("Loaded DNSSEC key from {}", key_path);
                vec![content.trim().to_string()]
            },
            Err(e) => {
                error!("Failed to load DNSSEC key from {}: {}", key_path, e);
                vec![]
            }
        };

        Ok(Self {
            bind_addr,
            db_path: env::var("DNS_DB_PATH").unwrap_or_else(|_| "dns.db".into()),
            cache_ttl: env::var("DNS_CACHE_TTL")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(DEFAULT_TTL),
            enable_ipv6: env::var("DNS_ENABLE_IPV6")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
            max_packet_size: env::var("DNS_MAX_PACKET_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(MAX_PACKET_SIZE),
            authoritative: env::var("DNS_AUTHORITATIVE")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
            ns_records: env::var("DNS_NS_RECORDS")
                .map(|v| v.split(',').map(|s| s.trim().to_string()).collect())
                .unwrap_or_else(|_| vec!["ns1.bzo.in.".into(), "ns2.bzo.in.".into()]),
            default_domain: env::var("DNS_DEFAULT_DOMAIN").unwrap_or_else(|_| "bzo.in".into()),
            default_ip: env::var("DNS_DEFAULT_IP").unwrap_or_else(|_| "60.254.61.33".into()),
            ds_records: vec![
                "bzo.in. IN DS 24550 8 2 1F21CA282945434EE0662805430599CB2A6C479D9F934087150901CE2DA580A0".to_string()
            ],
            dnskey_records,
            forwarders,
        })
    }
}

#[derive(Debug, Clone)]
struct CacheEntry {
    ip: String,
    inserted: SystemTime,
    ttl: u64,
}

#[derive(Debug)]
#[derive(Clone)]
struct DnsCache {
    entries: Arc<Mutex<HashMap<String, CacheEntry>>>,
    ns_records: Vec<String>,
}

impl DnsCache {
    fn new(ns_records: Vec<String>) -> Self {
        Self {
            entries: Arc::new(Mutex::new(HashMap::new())),
            ns_records,
        }
    }

    fn get(&self, domain: &str) -> Option<(String, u64)> {
        let mut cache = self.entries.lock().unwrap();
        if let Some(entry) = cache.get(domain) {
            if entry.inserted.elapsed().unwrap_or_default().as_secs() <= entry.ttl {
                return Some((entry.ip.clone(), entry.ttl));
            }
            cache.remove(domain);
        }
        None
    }

    fn set(&self, domain: String, ip: String, ttl: u64) {
        let mut cache = self.entries.lock().unwrap();
        cache.insert(
            domain,
            CacheEntry {
                ip,
                inserted: SystemTime::now(),
                ttl,
            },
        );
    }

    fn cleanup(&self) {
        let mut cache = self.entries.lock().unwrap();
        cache.retain(|_, entry| {
            entry.inserted.elapsed().map(|d| d.as_secs() <= entry.ttl).unwrap_or(true)
        });
    }
}

static CACHE: OnceLock<DnsCache> = OnceLock::new();

#[tokio::main]
async fn main() -> Result<(), DnsError> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_micros()
        .init();

    let config = ServerConfig::from_env()?;
    // Initialize cache with NS records from config
    let cache = CACHE.get_or_init(|| DnsCache::new(config.ns_records.clone()));

    init_db(&config.db_path, &config.default_domain, &config.default_ip)?;

    let cache_cleanup = task::spawn({
        let cache = cache.clone();
        async move {
            let mut interval = tokio::time::interval(CACHE_CLEANUP_INTERVAL);
            loop {
                interval.tick().await;
                cache.cleanup();
                debug!("Cache cleanup completed");
            }
        }
    });

    let shutdown_signal = async {
        signal::ctrl_c().await.expect("Failed to listen for shutdown signal");
        info!("Shutdown signal received");
    };

    let udp_server = run_udp_server(config.clone());
    let tcp_server = run_tcp_server(config.clone());

    tokio::select! {
        _ = shutdown_signal => {
            info!("Initiating graceful shutdown...");
            cache_cleanup.abort();
            Ok(())
        },
        res = udp_server => res,
        res = tcp_server => res,
    }
}

fn encode_rrsig_rr(
    rrsig_record: &str,
    ttl: u64,
) -> Result<Vec<u8>, DnsError> {
    // Example: "bzo.in. 3600 IN RRSIG DNSKEY 8 2 3600 20250601000000 20240501000000 24550 bzo.in. Q3N9z2n...base64..."
    let parts: Vec<&str> = rrsig_record.split_whitespace().collect();
    if parts.len() < 10 {
        return Err(DnsError::Config(format!(
            "Malformed RRSIG record - expected at least 10 parts, got {}",
            parts.len()
        )));
    }

    let type_covered = match parts[4] {
        "DNSKEY" => 48u16,
        "DS" => 43u16,
        "A" => 1u16,
        "NS" => 2u16,
        "SOA" => 6u16,
        _ => return Err(DnsError::Config(format!("Unsupported type_covered: {}", parts[4]))),
    };
    let algorithm = parts[5].parse::<u8>()?;
    let labels = parts[6].parse::<u8>()?;
    let orig_ttl = parts[7].parse::<u32>()?;
    let sig_exp = parse_sig_time(parts[8])?;
    let sig_inc = parse_sig_time(parts[9])?;
    let key_tag = parts[10].parse::<u16>()?;
    let signer_name = parts[11];
    let signature_b64 = parts[12..].join(""); // join in case signature is split

    // Encode signer_name as DNS name
    let mut signer_name_wire = Vec::new();
    for label in signer_name.trim_end_matches('.').split('.') {
        signer_name_wire.push(label.len() as u8);
        signer_name_wire.extend_from_slice(label.as_bytes());
    }
    signer_name_wire.push(0); // root

    let signature = base64::engine::general_purpose::STANDARD
        .decode(signature_b64)
        .map_err(|e| DnsError::Base64(e.to_string()))?;

    let mut rr = Vec::new();
    rr.extend_from_slice(&[0xc0, 0x0c]); // Name pointer to QNAME
    rr.extend_from_slice(&[0x00, 0x2e]); // TYPE = RRSIG (46)
    rr.extend_from_slice(&[0x00, 0x01]); // CLASS = IN
    rr.extend_from_slice(&(ttl as u32).to_be_bytes());

    // RDATA
    let mut rdata = Vec::new();
    rdata.extend_from_slice(&type_covered.to_be_bytes());
    rdata.push(algorithm);
    rdata.push(labels);
    rdata.extend_from_slice(&orig_ttl.to_be_bytes());
    rdata.extend_from_slice(&sig_exp.to_be_bytes());
    rdata.extend_from_slice(&sig_inc.to_be_bytes());
    rdata.extend_from_slice(&key_tag.to_be_bytes());
    rdata.extend_from_slice(&signer_name_wire);
    rdata.extend_from_slice(&signature);

    rr.extend_from_slice(&(rdata.len() as u16).to_be_bytes());
    rr.extend_from_slice(&rdata);

    Ok(rr)
}

// Helper: Parse YYYYMMDDHHMMSS to u32 (seconds since epoch)
fn parse_sig_time(s: &str) -> Result<u32, DnsError> {
    use chrono::{NaiveDateTime, TimeZone, Utc};
    let dt = NaiveDateTime::parse_from_str(s, "%Y%m%d%H%M%S")
        .map_err(|e| DnsError::Config(format!("Invalid sigtime: {e}")))?;
    Ok(Utc.from_utc_datetime(&dt).timestamp() as u32)
}

async fn run_udp_server(config: ServerConfig) -> Result<(), DnsError> {
    let socket = UdpSocket::bind(config.bind_addr).await?;
    info!("UDP DNS server listening on {}", config.bind_addr);
    let socket = Arc::new(socket);
    let mut buf = vec![0u8; config.max_packet_size];

    loop {
        match socket.recv_from(&mut buf).await {
            Ok((amt, src)) => {
                let query = buf[..amt].to_vec();
                let socket = socket.clone();
                let config = config.clone();
                task::spawn(async move {
                    if let Err(e) = handle_udp_query(query, src, socket, config).await {
                        warn!("UDP query error: {}", e);
                    }
                });
            }
            Err(e) => error!("UDP receive error: {}", e),
        }
    }
}

async fn handle_udp_query(
    query: Vec<u8>,
    src: SocketAddr,
    socket: Arc<UdpSocket>,
    config: ServerConfig,
) -> Result<(), DnsError> {
    if query.len() < 12 {
        debug!("Received malformed query from {}", src);
        return Ok(());
    }

    let opcode = (query[2] & 0x78) >> 3;
    if opcode != 0 {
        if let Some(response) = build_not_implemented_response(&query, config.authoritative) {
            socket.send_to(&response, src).await?;
        }
        return Ok(());
    }

    let domain = match extract_domain(&query) {
        Some(d) => d,
        None => {
            info!("Failed to extract domain from query");
            return Ok(());
        }
    };

    debug!("UDP query for {} from {}", domain, src);
    info!("Processing query for domain: {}", domain);

    let response = match generate_dns_response(&query, domain.clone(), &config).await {
        Ok(resp) => resp,
        Err(_) => {
            build_nxdomain_response(&query, config.authoritative)
                .ok_or(DnsError::Protocol("NXDOMAIN".into()))?
        }
    };

    socket.send_to(&response, src).await?;
    Ok(())
}

async fn forward_to_resolvers(query: &[u8], forwarders: &[SocketAddr]) -> Option<Vec<u8>> {
    for &forwarder in forwarders {
        info!("Forwarding query to resolver: {}", forwarder);
        if let Ok(resp) = forward_request_udp(forwarder, query).await {
            info!("Received response from resolver: {}", forwarder);
            return Some(resp);
        }
    }
    None
}

async fn forward_request_tcp(forwarder: SocketAddr, query: &[u8]) -> std::io::Result<Vec<u8>> {
    // Connect to the forwarder using TCP
    let mut stream = TcpStream::connect(forwarder).await?;

    // Write the query with a 2-byte length prefix (per DNS over TCP)
    let query_len = query.len() as u16;
    stream.write_all(&query_len.to_be_bytes()).await?;
    stream.write_all(query).await?;

    // Read the 2-byte length prefix of the response
    let mut len_buf = [0u8; 2];
    stream.read_exact(&mut len_buf).await?;
    let resp_len = u16::from_be_bytes(len_buf) as usize;

    // Read the response
    let mut resp_buf = vec![0u8; resp_len];
    stream.read_exact(&mut resp_buf).await?;

    Ok(resp_buf)
}

/// Tries each forwarder over TCP and returns the first valid response.
async fn forward_to_resolvers_tcp(query: &[u8], forwarders: &[SocketAddr]) -> Option<Vec<u8>> {
    for &forwarder in forwarders {
        if let Ok(resp) = forward_request_tcp(forwarder, query).await {
            return Some(resp);
        }
    }
    None
}

async fn forward_request_udp(forwarder: SocketAddr, query: &[u8]) -> std::io::Result<Vec<u8>> {
    let local = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0));
    let sock = UdpSocket::bind(local).await?;
    sock.send_to(query, forwarder).await?;
    let mut buf = [0u8; 4096];
    let (amt, _) = sock.recv_from(&mut buf).await?;
    Ok(buf[..amt].to_vec())
}

async fn run_tcp_server(config: ServerConfig) -> Result<(), DnsError> {
    let listener = TcpListener::bind(config.bind_addr).await?;
    info!("TCP DNS server listening on {}", config.bind_addr);

    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                let config = config.clone();
                task::spawn(async move {
                    if let Err(e) = handle_tcp_connection(stream, addr, config).await {
                        warn!("TCP connection error: {}", e);
                    }
                });
            }
            Err(e) => error!("TCP accept error: {}", e),
        }
    }
}

async fn handle_tcp_connection(
    mut stream: TcpStream,
    addr: SocketAddr,
    config: ServerConfig,
) -> Result<(), DnsError> {
    // Read the 2-byte length prefix
    let len = match stream.read_u16().await {
        Ok(len) => len,
        Err(_) => return Ok(()), // Connection closed or error
    };
    let mut buf = vec![0u8; len as usize];
    stream.read_exact(&mut buf).await?;

    // Check for minimum DNS header length
    if buf.len() < 12 {
        debug!("Received malformed TCP query from {}", addr);
        return Ok(());
    }

    // Check for standard query (opcode 0)
    let opcode = (buf[2] & 0x78) >> 3;
    if opcode != 0 {
        if let Some(response) = build_not_implemented_response(&buf, config.authoritative) {
            send_tcp_response(&mut stream, &response).await?;
        }
        return Ok(());
    }

    // Extract domain from query
    let domain = match extract_domain(&buf) {
        Some(d) => d,
        None => {
            info!("Failed to extract domain from TCP query");
            return Ok(());
        }
    };

    debug!("TCP query for {} from {}", domain, addr);
    info!("Processing TCP query for domain: {}", domain);

    // Try local/cache resolution first
    let response = match generate_dns_response(&buf, domain.clone(), &config).await {
        Ok(resp) => resp,
        Err(_) => {
            // Try TCP forwarding to upstream resolvers
            if !config.forwarders.is_empty() {
                if let Some(resp) = forward_to_resolvers_tcp(&buf, &config.forwarders).await {
                    send_tcp_response(&mut stream, &resp).await?;
                    return Ok(());
                }
            }
            // Fallback: NXDOMAIN
            let nxdomain = build_nxdomain_response(&buf, config.authoritative)
                .ok_or(DnsError::Protocol("NXDOMAIN".into()))?;
            send_tcp_response(&mut stream, &nxdomain).await?;
            return Ok(());
        }
    };

    // Send the response (local/cache answer)
    send_tcp_response(&mut stream, &response).await?;
    Ok(())
}

async fn send_tcp_response(stream: &mut TcpStream, response: &[u8]) -> std::io::Result<()> {
    stream.write_all(&(response.len() as u16).to_be_bytes()).await?;
    stream.write_all(response).await
}

fn extract_domain(query: &[u8]) -> Option<String> {
    if query.len() < 12 {
        return None; // DNS header is 12 bytes
    }

    let mut pos = 12; // Start after header
    let mut domain = String::new();

    // Extract QNAME (domain)
    loop {
        let len = query[pos] as usize;
        if len == 0 {
            break; // End of QNAME
        }
        pos += 1;

        if pos + len > query.len() {
            return None; // Invalid length
        }

        if !domain.is_empty() {
            domain.push('.');
        }

        let label = match str::from_utf8(&query[pos..pos + len]) {
            Ok(l) => l,
            Err(_) => return None, // Invalid UTF-8
        };
        domain.push_str(label);
        pos += len;
    }

    // Skip QTYPE and QCLASS (4 bytes)
    pos += 4;

    // Verify we have enough data for at least QTYPE/QCLASS
    if pos > query.len() {
        return None;
    }

    Some(domain)
}

async fn generate_dns_response(
    query: &[u8],
    domain: String,
    config: &ServerConfig,
) -> Result<Vec<u8>, DnsError> {
    let query_type = extract_query_type(query).unwrap_or(1);

    // Handle DNSKEY queries first
    if query_type == 48 {
        if let Some(dnskey) = config.dnskey_records.first() {
            return build_dnskey_response(query, dnskey, 3600, config);
        }
        return build_nxdomain_response(&query, config.authoritative)
            .ok_or(DnsError::Protocol("No DNSKEY record".into()));
    }

    // Handle DS queries (type 43)
    if query_type == 43 {
        if let Some(ds) = config.ds_records.first() {
            return build_ds_response(query, ds, 3600, config);
        }
        return build_nxdomain_response(&query, config.authoritative)
            .ok_or(DnsError::Protocol("NXDOMAIN".into()));
    }

    // Check cache for A/AAAA queries
    if query_type == 1 || query_type == 28 {
        if let Some((ip, ttl)) = CACHE.get().unwrap().get(&domain) {
            return build_dns_response(&query, &ip, ttl, config);
        }
    }

    // Lookup records in database
    let records = lookup_records(&config.db_path, &domain);
    let requested_type = match query_type {
        1 => "A",
        2 => "NS",
        5 => "CNAME",
        6 => "SOA",
        12 => "PTR",
        15 => "MX",
        16 => "TXT",
        28 => "AAAA",
        _ => "",
    };

    // Try exact match first
    if let Some((value, ttl, _)) = records.iter()
        .find(|(_, _, rtype)| rtype == requested_type)
        .cloned()
    {
        return match requested_type {
            "SOA" => build_soa_response(&query, &value, ttl, domain, config),
            "NS" => build_ns_response(&query, &records, ttl, domain, config),
            "MX" | "TXT" | "CNAME" | "PTR" => {
                build_generic_record_response(&query, &value, ttl, domain, query_type, config)
            },
            "A" | "AAAA" => {
                CACHE.get().unwrap().set(domain.clone(), value.clone(), ttl);
                build_dns_response(&query, &value, ttl, config)
            },
            _ => Err(DnsError::Protocol("Unsupported record type".into()))
        };
    }

    // Fallback for A/AAAA queries
    if query_type == 1 || query_type == 28 {
        if let Some((ip, ttl, _)) = records.iter()
            .find(|(_, _, rtype)| rtype == "A" || rtype == "AAAA")
            .cloned()
        {
            CACHE.get().unwrap().set(domain, ip.clone(), ttl);
            return build_dns_response(&query, &ip, ttl, config);
        }
    }

    // Authority and forwarding logic
    let zones = get_authoritative_zones(&config.db_path);
    let is_authoritative = find_closest_parent_zone(&domain, &zones).is_some() && config.authoritative;

    if !is_authoritative && !config.forwarders.is_empty() {
        if let Some(resp) = forward_to_resolvers(query, &config.forwarders).await {
            let mut response = resp.clone();
            if response.len() >= 3 {
                response[2] &= 0xFB; // Clear AA bit
            }
            return Ok(response);
        }
    }

    build_nxdomain_response(&query, is_authoritative)
        .ok_or(DnsError::Protocol("NXDOMAIN".into()))
}

/// Build a DNS response for a DS record (in the AUTHORITY section).
fn build_ds_response(
    query: &[u8],
    ds_record: &str,
    ttl: u64,
    config: &ServerConfig,
) -> Result<Vec<u8>, DnsError> {
    // Parse: "bzo.in. IN DS 24550 8 2 1F21CA282945434EE0662805430599CB2A6C479D9F934087150901CE2DA580A0"
    let parts: Vec<&str> = ds_record.splitn(7, ' ').collect();
    if parts.len() != 7 {
        return Err(DnsError::Config(format!("Malformed DS record: {:?}", ds_record)));
    }
    let key_tag = parts[3].parse::<u16>()?;
    let algorithm = parts[4].parse::<u8>()?;
    let digest_type = parts[5].parse::<u8>()?;
    let digest = hex::decode(parts[6]).map_err(|e| DnsError::Config(format!("DS digest hex error: {e}")))?;

    let mut response = Vec::with_capacity(512);
    // Header (see build_dnskey_response for details)
    response.extend_from_slice(&query[..2]);
    let qr = 0x80;
    let aa = if config.authoritative { 0x04 } else { 0x00 };
    let rd = query[2] & 0x01;
    let ra = 0x80;
    response.push(qr | aa | rd);
    response.push(ra);
    response.extend_from_slice(&[0x00, 0x01]); // QDCOUNT
    response.extend_from_slice(&[0x00, 0x00]); // ANCOUNT
    response.extend_from_slice(&[0x00, 0x01]); // NSCOUNT
    response.extend_from_slice(&[0x00, 0x00]); // ARCOUNT

    // Copy question section
    let qname_start = 12;
    let qname_end = qname_start + query[qname_start..]
        .iter()
        .position(|&b| b == 0)
        .ok_or(DnsError::Protocol("Invalid question section".into()))?;
    let question_len = (qname_end - qname_start) + 1 + 2 + 2;
    response.extend_from_slice(&query[qname_start..qname_start + question_len]);

    // Authority section: DS RR
    response.extend_from_slice(&[0xc0, 0x0c]); // Name pointer
    response.extend_from_slice(&[0x00, 0x2b]); // TYPE = DS (43)
    response.extend_from_slice(&[0x00, 0x01]); // CLASS = IN
    response.extend_from_slice(&(ttl as u32).to_be_bytes());

    let mut rdata = Vec::with_capacity(4 + digest.len());
    rdata.extend_from_slice(&key_tag.to_be_bytes());
    rdata.push(algorithm);
    rdata.push(digest_type);
    rdata.extend_from_slice(&digest);
    response.extend_from_slice(&(rdata.len() as u16).to_be_bytes());
    response.extend_from_slice(&rdata);

    Ok(response)
}

fn build_dnskey_response(
    query: &[u8],
    dnskey_record: &str,
    ttl: u64,
    config: &ServerConfig,
) -> Result<Vec<u8>, DnsError> {
    // Parse the record (format: "bzo.in. IN DNSKEY 256 3 8 AwEAA...")
    let parts: Vec<&str> = dnskey_record.splitn(7, ' ').collect();
    if parts.len() != 7 {
        return Err(DnsError::Config(format!(
            "Malformed DNSKEY record - expected 7 space-separated parts, got {}",
            parts.len()
        )));
    }

    // Extract components
    let flags = parts[3].parse::<u16>()?;
    let protocol = parts[4].parse::<u8>()?;
    let algorithm = parts[5].parse::<u8>()?;
    let public_key = base64::engine::general_purpose::STANDARD
        .decode(parts[6])
        .map_err(|e| DnsError::Base64(e.to_string()))?;

    // Build response
    let mut response = Vec::with_capacity(512);

    // Header (12 bytes)
    response.extend_from_slice(&query[..2]); // ID

    // Flags: QR (response), AA (authoritative), RD (recursion desired), RA (recursion available)
    let qr = 0x80;
    let aa = if config.authoritative { 0x04 } else { 0x00 };
    let rd = query[2] & 0x01;
    let ra = 0x80;
    response.push(qr | aa | rd);
    response.push(ra);

    // QDCOUNT (1), ANCOUNT (1), NSCOUNT (0), ARCOUNT (0)
    response.extend_from_slice(&[0x00, 0x01]); // QDCOUNT
    response.extend_from_slice(&[0x00, 0x01]); // ANCOUNT
    response.extend_from_slice(&[0x00, 0x00]); // NSCOUNT
    response.extend_from_slice(&[0x00, 0x00]); // ARCOUNT

    // Copy question section exactly (QNAME + zero + QTYPE + QCLASS)
    let qname_start = 12;
    let qname_end = qname_start + query[qname_start..]
        .iter()
        .position(|&b| b == 0)
        .ok_or(DnsError::Protocol("Invalid question section".into()))?;
    let question_len = (qname_end - qname_start) + 1 + 2 + 2;
    response.extend_from_slice(&query[qname_start..qname_start + question_len]);

    // Answer section: DNSKEY RR
    response.extend_from_slice(&[0xc0, 0x0c]); // Name pointer to QNAME
    response.extend_from_slice(&[0x00, 0x30]); // TYPE = DNSKEY (48)
    response.extend_from_slice(&[0x00, 0x01]); // CLASS = IN (1)
    response.extend_from_slice(&(ttl as u32).to_be_bytes()); // TTL (4 bytes)

    // RDATA: flags (2), protocol (1), algorithm (1), public key (variable)
    let mut rdata = Vec::with_capacity(4 + public_key.len());
    rdata.extend_from_slice(&flags.to_be_bytes());
    rdata.push(protocol);
    rdata.push(algorithm);
    rdata.extend_from_slice(&public_key);

    // RDLENGTH
    response.extend_from_slice(&(rdata.len() as u16).to_be_bytes());
    response.extend_from_slice(&rdata);

    Ok(response)
}


// Function to build response for SOA records
fn build_soa_response(
    query: &[u8],
    soa_value: &str,
    ttl: u64,
    domain: String,
    config: &ServerConfig,
) -> Result<Vec<u8>, DnsError> {
    let mut response = Vec::with_capacity(512);

    // Copy transaction ID and question from query
    response.extend_from_slice(&query[..2]);

    // Set flags
    // QR = 1 (response)
    // OPCODE = 0 (standard query)
    // AA = 1 if authoritative
    // TC = 0 (not truncated)
    // RD = copy from query
    // RA = 1 (recursion available)
    // Z = 0
    // RCODE = 0 (no error)
    let flags1 = 0x80 | (query[2] & 0x01); // Set QR and preserve RD
    let flags2 = 0x80; // Set RA

    response.extend_from_slice(&[
        if config.authoritative { flags1 | 0x04 } else { flags1 }, // Set AA if authoritative
        flags2,
    ]);

    // Copy QDCOUNT from query
    response.extend_from_slice(&query[4..6]);

    // Set ANCOUNT to 1
    response.extend_from_slice(&[0x00, 0x01]);

    // Set NSCOUNT (2 if authoritative, else 0)
    response.extend_from_slice(&[0x00, if config.authoritative { 0x02 } else { 0x00 }]);

    // Set ARCOUNT to 0
    response.extend_from_slice(&[0x00, 0x00]);

    // Copy question section from query
    let qname_end = query[12..].iter().position(|&b| b == 0)
        .ok_or_else(|| DnsError::Protocol("Invalid question format".into()))? + 13;
    response.extend_from_slice(&query[12..qname_end + 4]);

    // Add answer section for SOA
    // Name pointer to question
    response.extend_from_slice(&[0xc0, 0x0c]);

    // Type SOA (0x0006)
    response.extend_from_slice(&[0x00, 0x06]);

    // Class IN (0x0001)
    response.extend_from_slice(&[0x00, 0x01]);

    // TTL (32 bits)
    response.extend_from_slice(&(ttl as u32).to_be_bytes());

    // Parse and encode SOA record
    let soa_parts: Vec<&str> = soa_value.split_whitespace().collect();
    if soa_parts.len() >= 7 {
        // MNAME (primary nameserver)
        let mname = encode_dns_name(soa_parts[0]);
        // RNAME (responsible person mailbox)
        let rname = encode_dns_name(soa_parts[1]);

        // Calculate RDLENGTH (mname + rname + 5 x 32-bit integers)
        let rdlength = mname.len() + rname.len() + 20;
        response.extend_from_slice(&(rdlength as u16).to_be_bytes());

        // Add MNAME and RNAME
        response.extend_from_slice(&mname);
        response.extend_from_slice(&rname);

        // Add the 5 SOA integers (SERIAL, REFRESH, RETRY, EXPIRE, MINIMUM)
        for i in 2..7 {
            if let Ok(val) = soa_parts[i].parse::<u32>() {
                response.extend_from_slice(&val.to_be_bytes());
            } else {
                // Default values if parsing fails
                response.extend_from_slice(&match i {
                    2 => 1u32, // SERIAL
                    3 => 10800u32, // REFRESH
                    4 => 3600u32, // RETRY
                    5 => 604800u32, // EXPIRE
                    _ => 86400u32, // MINIMUM
                }.to_be_bytes());
            }
        }
    } else {
        // If SOA format is invalid, add a minimal valid SOA record
        let ns_records = &config.ns_records;
        let mname = encode_dns_name(&ns_records.first().cloned().unwrap_or_else(|| "ns1.example.com.".to_string()));
        let rname = encode_dns_name(&format!("hostmaster.{}", domain));

        // Calculate RDLENGTH
        let rdlength = mname.len() + rname.len() + 20;
        response.extend_from_slice(&(rdlength as u16).to_be_bytes());

        // MNAME and RNAME
        response.extend_from_slice(&mname);
        response.extend_from_slice(&rname);

        // SOA integers with reasonable defaults
        response.extend_from_slice(&1u32.to_be_bytes()); // SERIAL
        response.extend_from_slice(&10800u32.to_be_bytes()); // REFRESH
        response.extend_from_slice(&3600u32.to_be_bytes()); // RETRY
        response.extend_from_slice(&604800u32.to_be_bytes()); // EXPIRE
        response.extend_from_slice(&86400u32.to_be_bytes()); // MINIMUM
    }

    // Add authoritative NS records if configured
    if config.authoritative {
        for ns in &config.ns_records {
            // Name pointer to question
            response.extend_from_slice(&[0xc0, 0x0c]);

            // Type NS (0x0002)
            response.extend_from_slice(&[0x00, 0x02]);

            // Class IN (0x0001)
            response.extend_from_slice(&[0x00, 0x01]);

            // TTL (use same as A record)
            response.extend_from_slice(&(ttl as u32).to_be_bytes());

            // RDLENGTH and RDATA (NS name)
            let ns_data = encode_dns_name(ns);
            response.extend_from_slice(&((ns_data.len() as u16).to_be_bytes()));
            response.extend_from_slice(&ns_data);
        }
    }

    Ok(response)
}

// Function to build response for NS records
fn build_ns_response(
    query: &[u8],
    records: &[(String, u64, String)],
    ttl: u64,
    domain: String,
    config: &ServerConfig,
) -> Result<Vec<u8>, DnsError> {
    let mut response = Vec::with_capacity(512);

    // Copy transaction ID and question from query
    response.extend_from_slice(&query[..2]);

    // Set flags
    let flags1 = 0x80 | (query[2] & 0x01); // Set QR and preserve RD
    let flags2 = 0x80; // Set RA

    response.extend_from_slice(&[
        if config.authoritative { flags1 | 0x04 } else { flags1 }, // Set AA if authoritative
        flags2,
    ]);

    // Filter NS records from database
    let db_ns_records: Vec<(String, u64, String)> = records.iter()
        .filter(|(_, _, rtype)| rtype == "NS")
        .cloned()
        .collect();

    // Use database records if available, otherwise fall back to config if authoritative
    let ns_records: Vec<(String, u64, String)> = if db_ns_records.is_empty() && config.authoritative {
        config.ns_records.iter()
            .map(|ns| (ns.clone(), config.cache_ttl, "NS".to_string()))
            .collect()
    } else {
        db_ns_records
    };

    // Copy QDCOUNT from query
    response.extend_from_slice(&query[4..6]);

    // Set ANCOUNT to number of NS records
    response.extend_from_slice(&(ns_records.len() as u16).to_be_bytes());

    // Set NSCOUNT to 0 (we're putting all NS records in answer section)
    response.extend_from_slice(&[0x00, 0x00]);

    // Set ARCOUNT to 0
    response.extend_from_slice(&[0x00, 0x00]);

    // Copy question section from query
    let qname_end = query[12..].iter().position(|&b| b == 0)
        .ok_or_else(|| DnsError::Protocol("Invalid question format".into()))? + 13;
    response.extend_from_slice(&query[12..qname_end + 4]);

    // Add answer section for each NS record
    for (value, record_ttl, _) in ns_records {
        // Name pointer to question
        response.extend_from_slice(&[0xc0, 0x0c]);

        // Type NS (0x0002)
        response.extend_from_slice(&[0x00, 0x02]);

        // Class IN (0x0001)
        response.extend_from_slice(&[0x00, 0x01]);

        // TTL (32 bits)
        response.extend_from_slice(&(record_ttl as u32).to_be_bytes());

        // RDLENGTH and RDATA (NS name)
        let ns_data = encode_dns_name(&value);
        response.extend_from_slice(&((ns_data.len() as u16).to_be_bytes()));
        response.extend_from_slice(&ns_data);
    }

    Ok(response)
}

// Function to build responses for MX, TXT, CNAME records
fn build_generic_record_response(
    query: &[u8],
    value: &str,
    ttl: u64,
    domain: String,
    query_type: u16,
    config: &ServerConfig,
) -> Result<Vec<u8>, DnsError> {
    let mut response = Vec::with_capacity(512);

    // Copy transaction ID and question from query
    response.extend_from_slice(&query[..2]);

    // Set flags
    let flags1 = 0x80 | (query[2] & 0x01); // Set QR and preserve RD
    let flags2 = 0x80; // Set RA

    response.extend_from_slice(&[
        if config.authoritative { flags1 | 0x04 } else { flags1 }, // Set AA if authoritative
        flags2,
    ]);

    // Copy QDCOUNT from query
    response.extend_from_slice(&query[4..6]);

    // Set ANCOUNT to 1
    response.extend_from_slice(&[0x00, 0x01]);

    // Set NSCOUNT (2 if authoritative, else 0)
    response.extend_from_slice(&[0x00, if config.authoritative { 0x02 } else { 0x00 }]);

    // Set ARCOUNT to 0
    response.extend_from_slice(&[0x00, 0x00]);

    // Copy question section from query
    let qname_end = query[12..].iter().position(|&b| b == 0)
        .ok_or_else(|| DnsError::Protocol("Invalid question format".into()))? + 13;
    response.extend_from_slice(&query[12..qname_end + 4]);

    // Add answer section
    // Name pointer to question
    response.extend_from_slice(&[0xc0, 0x0c]);

    // Record Type (2 bytes)
    response.extend_from_slice(&query_type.to_be_bytes());

    // Class IN (0x0001)
    response.extend_from_slice(&[0x00, 0x01]);

    // TTL (32 bits)
    response.extend_from_slice(&(ttl as u32).to_be_bytes());

    // Format based on record type
    match query_type {
        12 => { // PTR record
            // Encode the PTR target name
            let ptr_data = encode_dns_name(value);

            // Add RDLENGTH
            response.extend_from_slice(&(ptr_data.len() as u16).to_be_bytes());

            // Add the PTR data
            response.extend_from_slice(&ptr_data);
        },
        15 => { // MX record
            // Parse MX record format: "10 mail.example.com."
            let parts: Vec<&str> = value.split_whitespace().collect();
            if parts.len() >= 2 {
                let preference = parts[0].parse::<u16>().unwrap_or(10);
                let mx_host = parts[1];

                // Encode the data
                let encoded_name = encode_dns_name(mx_host);
                let rdlength = 2 + encoded_name.len(); // preference (2 bytes) + name

                // Add RDLENGTH
                response.extend_from_slice(&(rdlength as u16).to_be_bytes());

                // Add preference
                response.extend_from_slice(&preference.to_be_bytes());

                // Add exchange domain
                response.extend_from_slice(&encoded_name);
            } else {
                return Err(DnsError::Protocol("Invalid MX record format".into()));
            }
        },
        16 => { // TXT record
            // Strip quotes if present
            let txt_value = if value.starts_with('"') && value.ends_with('"') {
                &value[1..value.len()-1]
            } else {
                value
            };

            // TXT records are length-prefixed character strings
            let bytes = txt_value.as_bytes();
            let mut txt_data = Vec::with_capacity(bytes.len() + 1);

            // Add the length byte
            txt_data.push(bytes.len() as u8);
            // Add the text
            txt_data.extend_from_slice(bytes);

            // Add RDLENGTH
            response.extend_from_slice(&(txt_data.len() as u16).to_be_bytes());

            // Add the TXT data
            response.extend_from_slice(&txt_data);
        },
        5 => { // CNAME record
            // Encode the canonical name
            let cname_data = encode_dns_name(value);

            // Add RDLENGTH
            response.extend_from_slice(&(cname_data.len() as u16).to_be_bytes());

            // Add the CNAME data
            response.extend_from_slice(&cname_data);
        },
        _ => {
            return Err(DnsError::Protocol(format!("Unsupported record type: {}", query_type)));
        }
    }

    // Add authoritative NS records if configured
    if config.authoritative {
        for ns in &config.ns_records {
            // Name pointer to question
            response.extend_from_slice(&[0xc0, 0x0c]);

            // Type NS (0x0002)
            response.extend_from_slice(&[0x00, 0x02]);

            // Class IN (0x0001)
            response.extend_from_slice(&[0x00, 0x01]);

            // TTL (use same as record TTL)
            response.extend_from_slice(&(ttl as u32).to_be_bytes());

            // RDLENGTH and RDATA (NS name)
            let ns_data = encode_dns_name(ns);
            response.extend_from_slice(&((ns_data.len() as u16).to_be_bytes()));
            response.extend_from_slice(&ns_data);
        }
    }

    Ok(response)
}

fn lookup_records(db_path: &str, domain: &str) -> Vec<(String, u64, String)> {
    let conn = Connection::open(db_path);
    match conn {
        Ok(conn) => {
            match conn.prepare(
                "SELECT value, ttl, record_type FROM dns_records WHERE domain = ?"
            ) {
                Ok(mut stmt) => {
                    match stmt.query_map(params![domain], |row| {
                        Ok((
                            row.get(0).unwrap_or_default(),
                            row.get(1).unwrap_or_default(),
                            row.get(2).unwrap_or_default(),
                        ))
                    }) {
                        Ok(rows) => rows.filter_map(Result::ok).collect(),
                        Err(_) => Vec::new(),
                    }
                }
                Err(_) => Vec::new(),
            }
        }
        Err(_) => Vec::new(),
    }
}

// First, we need to add a function to parse the query type from the DNS packet
fn extract_query_type(query: &[u8]) -> Option<u16> {
    if query.len() < 12 {
        return None; // DNS header is 12 bytes
    }

    let mut pos = 12; // Start after header

    // Skip QNAME
    loop {
        if pos >= query.len() {
            return None;
        }

        let len = query[pos] as usize;
        if len == 0 {
            pos += 1;
            break; // End of QNAME
        }

        pos += len + 1;
    }

    // Get QTYPE (2 bytes after QNAME)
    if pos + 1 < query.len() {
        Some(((query[pos] as u16) << 8) | query[pos + 1] as u16)
    } else {
        None
    }
}

fn build_dns_response(
    query: &[u8],
    ip: &str,
    ttl: u64,
    config: &ServerConfig,
) -> Result<Vec<u8>, DnsError> {
    let mut response = Vec::with_capacity(512);

    // Copy transaction ID and question from query
    response.extend_from_slice(&query[..2]);

    // Set flags
    // QR = 1 (response)
    // OPCODE = 0 (standard query)
    // AA = 1 if authoritative
    // TC = 0 (not truncated)
    // RD = copy from query
    // RA = 1 (recursion available)
    // Z = 0
    // RCODE = 0 (no error)
    let flags1 = 0x80 | (query[2] & 0x01); // Set QR and preserve RD
    let flags2 = 0x80; // Set RA

    response.extend_from_slice(&[
        if config.authoritative { flags1 | 0x04 } else { flags1 }, // Set AA if authoritative
        flags2,
    ]);

    // Copy QDCOUNT from query
    response.extend_from_slice(&query[4..6]);

    // Set ANCOUNT to 1
    response.extend_from_slice(&[0x00, 0x01]);

    // Set NSCOUNT (2 if authoritative, else 0)
    response.extend_from_slice(&[0x00, if config.authoritative { 0x02 } else { 0x00 }]);

    // Set ARCOUNT to 0
    response.extend_from_slice(&[0x00, 0x00]);

    // Copy question section from query
    let qname_end = query[12..].iter().position(|&b| b == 0)
        .ok_or_else(|| DnsError::Protocol("Invalid question format".into()))? + 13;
    response.extend_from_slice(&query[12..qname_end + 4]);

    // Add answer section
    // Name pointer to question
    response.extend_from_slice(&[0xc0, 0x0c]);

    // Type A (0x0001)
    response.extend_from_slice(&[0x00, 0x01]);

    // Class IN (0x0001)
    response.extend_from_slice(&[0x00, 0x01]);

    // TTL (32 bits)
    response.extend_from_slice(&(ttl as u32).to_be_bytes());

    // RDLENGTH (4 for IPv4)
    response.extend_from_slice(&[0x00, 0x04]);

    // RDATA (IPv4 address)
    let ip_parts: Vec<u8> = ip.split('.')
        .map(|s| s.parse::<u8>().unwrap_or(0))
        .collect();
    if ip_parts.len() == 4 {
        response.extend_from_slice(&ip_parts);
    } else {
        return Err(DnsError::Protocol("Invalid IP address format".into()));
    }

    // Add authoritative NS records if configured
    if config.authoritative {
        for ns in &config.ns_records {
            // Name pointer to question
            response.extend_from_slice(&[0xc0, 0x0c]);

            // Type NS (0x0002)
            response.extend_from_slice(&[0x00, 0x02]);

            // Class IN (0x0001)
            response.extend_from_slice(&[0x00, 0x01]);

            // TTL (use same as A record)
            response.extend_from_slice(&(ttl as u32).to_be_bytes());

            // RDLENGTH and RDATA (NS name)
            let ns_data = encode_dns_name(ns);
            response.extend_from_slice(&((ns_data.len() as u16).to_be_bytes()));
            response.extend_from_slice(&ns_data);
        }
    }

    Ok(response)
}

// Add a function to handle NOTIMP responses properly
fn build_not_implemented_response(query: &[u8], authoritative: bool) -> Option<Vec<u8>> {
    let mut resp = Vec::with_capacity(512);
    resp.extend_from_slice(&query[0..2]); // Transaction ID

    // Get original flags but set QR bit and preserve RD and OPCODE
    let flags1 = 0x80 | (query[2] & 0x79); // QR=1, OPCODE preserved, AA=0, TC=0, RD=preserved
    let flags2 = 0x84; // RA=1, Z=0, RCODE=4 (Not Implemented)

    resp.extend_from_slice(&[
        flags1,
        if authoritative { flags2 | 0x04 } else { flags2 }, // Set AA bit if authoritative
    ]);

    resp.extend_from_slice(&query[4..6]); // QDCOUNT
    resp.extend_from_slice(&[0x00, 0x00]); // ANCOUNT
    resp.extend_from_slice(&[0x00, 0x00]); // NSCOUNT

    // Check for EDNS
    let has_edns = has_opt_record(query);
    resp.extend_from_slice(&(if has_edns { 1u16 } else { 0u16 }).to_be_bytes()); // ARCOUNT

    // Question Section (if available)
    let qname_end = query[12..].iter().position(|&b| b == 0);
    if let Some(end) = qname_end {
        if 12 + end + 5 <= query.len() {
            resp.extend_from_slice(&query[12..12 + end + 5]);
        }
    }

    // Handle EDNS in NOTIMP response
    if has_edns {
        let opt_payload_size = extract_edns_payload_size(query).unwrap_or(4096);

        // Copy DO bit if present in request
        let do_bit = if query.len() >= 17 {
            extract_do_bit(query)
        } else {
            false
        };

        // Add OPT record for EDNS
        resp.extend_from_slice(&[0x00]); // Root domain
        resp.extend_from_slice(&[0x00, 0x29]); // TYPE OPT
        resp.extend_from_slice(&opt_payload_size.to_be_bytes()); // UDP payload size from request
        resp.extend_from_slice(&[0x00]); // Extended RCODE
        resp.extend_from_slice(&[0x00]); // EDNS version

        if do_bit {
            resp.extend_from_slice(&[0x80, 0x00]); // Flags with DO bit set
        } else {
            resp.extend_from_slice(&[0x00, 0x00]); // Flags with DO bit clear
        }

        resp.extend_from_slice(&[0x00, 0x00]); // RDATA length
    }

    Some(resp)
}

// Check if the query has an OPT record (EDNS)
fn has_opt_record(query: &[u8]) -> bool {
    if query.len() < 12 {
        return false;
    }

    // Get the number of additional records (ARCOUNT)
    let ar_count = ((query[10] as u16) << 8) | query[11] as u16;
    if ar_count == 0 {
        return false;
    }

    // Skip the header and question section to find additional records
    let mut pos = 12;

    // Skip QNAME
    loop {
        if pos >= query.len() {
            return false;
        }

        let len = query[pos] as usize;
        if len == 0 {
            pos += 1;
            break;
        }

        pos += len + 1;
    }

    // Skip QTYPE and QCLASS
    pos += 4;

    // Check if there's an OPT record (type 41) in additional records
    for _ in 0..ar_count {
        if pos + 2 >= query.len() {
            return false;
        }

        // Check for root domain (0x00) and OPT record type (0x0029)
        if query[pos] == 0x00 && pos + 5 < query.len() && query[pos+1] == 0x00 && query[pos+2] == 0x29 {
            return true;
        }

        // Skip this record
        // For non-OPT records, we need to skip to the next record, which is complex
        // This is a simplified approach
        pos += 1;

        // Find the end of the name field
        while pos < query.len() && query[pos] != 0 {
            // Handle compression pointers
            if (query[pos] & 0xC0) == 0xC0 {
                pos += 2;
                break;
            }

            // Regular label
            let label_len = query[pos] as usize;
            pos += label_len + 1;
        }

        // Skip type, class, TTL, and RDLENGTH + RDATA
        if pos + 10 <= query.len() {
            let rd_length = ((query[pos+8] as u16) << 8) | query[pos+9] as u16;
            pos += 10 + rd_length as usize;
        } else {
            return false;
        }
    }

    false
}

// Extract the payload size from an EDNS query
fn extract_edns_payload_size(query: &[u8]) -> Option<u16> {
    if query.len() < 12 {
        return None;
    }

    // Get the number of additional records (ARCOUNT)
    let ar_count = ((query[10] as u16) << 8) | query[11] as u16;
    if ar_count == 0 {
        return None;
    }

    // Skip the header and question section to find additional records
    let mut pos = 12;

    // Skip QNAME
    loop {
        if pos >= query.len() {
            return None;
        }

        let len = query[pos] as usize;
        if len == 0 {
            pos += 1;
            break;
        }

        pos += len + 1;
    }

    // Skip QTYPE and QCLASS
    pos += 4;

    // Look for OPT record
    for _ in 0..ar_count {
        if pos + 2 >= query.len() {
            return None;
        }

        // Check for root domain (0x00) and OPT record type (0x0029)
        if query[pos] == 0x00 && pos + 5 < query.len() && query[pos+1] == 0x00 && query[pos+2] == 0x29 {
            // Extract UDP payload size (CLASS field in OPT)
            if pos + 4 < query.len() {
                return Some(((query[pos+3] as u16) << 8) | query[pos+4] as u16);
            }
            return Some(4096); // Default if we can't read it
        }

        // Skip this record (simplified approach)
        pos += 1;

        // Find the end of the name field
        while pos < query.len() && query[pos] != 0 {
            // Handle compression pointers
            if (query[pos] & 0xC0) == 0xC0 {
                pos += 2;
                break;
            }

            // Regular label
            let label_len = query[pos] as usize;
            pos += label_len + 1;
        }

        // Skip type, class, TTL, and RDLENGTH + RDATA
        if pos + 10 <= query.len() {
            let rd_length = ((query[pos+8] as u16) << 8) | query[pos+9] as u16;
            pos += 10 + rd_length as usize;
        } else {
            return None;
        }
    }

    None
}

fn extract_do_bit(query: &[u8]) -> bool {
    // The DO bit is the highest bit in the second byte of the OPT record flags
    // Find the OPT record first (Type = 41 = 0x29)
    let mut pos = 12; // Skip header

    // Skip question section
    while pos < query.len() && query[pos] != 0 {
        pos += 1;
    }
    pos += 5; // Skip null terminator (1) + QTYPE (2) + QCLASS (2)

    // Look for OPT record
    while pos + 11 <= query.len() {
        if query[pos] == 0 && // Root domain name
            query[pos + 1] == 0 && query[pos + 2] == 0x29 { // Type OPT
            // The DO bit is in the flags section, 9 bytes after the start of the OPT record
            return pos + 9 < query.len() && (query[pos + 9] & 0x80) != 0;
        }
        // Skip this record
        if pos + 10 >= query.len() { break; }
        let rdlength = ((query[pos + 8] as usize) << 8) | query[pos + 9] as usize;
        pos += 10 + rdlength;
    }

    false // No OPT record found or no DO bit set
}

/// Helper function to encode domain names
fn encode_dns_name(name: &str) -> Vec<u8> {
    let mut out = Vec::new();
    for part in name.trim_end_matches('.').split('.') {
        if part.len() > 63 {
            continue; // Skip invalid labels
        }
        out.push(part.len() as u8);
        out.extend_from_slice(part.as_bytes());
    }
    out.push(0); // Null terminator
    out
}

// Add this new struct to properly track zone information
#[derive(Debug, Clone)]
struct ZoneInfo {
    name: String,
    ns_records: Vec<String>,
    soa_record: Option<String>,
}

// Add this function to extract zones from database
fn get_authoritative_zones(db_path: &str) -> Vec<ZoneInfo> {
    let mut zones = Vec::new();

    if let Ok(conn) = Connection::open(db_path) {
        // Find all domains with NS records (these are zones)
        if let Ok(mut stmt) = conn.prepare(
            "SELECT DISTINCT domain FROM dns_records WHERE record_type = 'NS'"
        ) {
            if let Ok(rows) = stmt.query_map([], |row| {
                Ok(row.get::<_, String>(0)?)
            }) {
                for domain_result in rows {
                    if let Ok(domain) = domain_result {
                        let mut zone_info = ZoneInfo {
                            name: domain.clone(),
                            ns_records: Vec::new(),
                            soa_record: None,
                        };

                        // Get NS records for this zone
                        if let Ok(mut ns_stmt) = conn.prepare(
                            "SELECT value FROM dns_records WHERE domain = ? AND record_type = 'NS'"
                        ) {
                            if let Ok(ns_rows) = ns_stmt.query_map([&domain], |row| {
                                Ok(row.get::<_, String>(0)?)
                            }) {
                                zone_info.ns_records = ns_rows.filter_map(Result::ok).collect();
                            }
                        }

                        // Get SOA record if exists
                        if let Ok(mut soa_stmt) = conn.prepare(
                            "SELECT value FROM dns_records WHERE domain = ? AND record_type = 'SOA' LIMIT 1"
                        ) {
                            if let Ok(mut soa_rows) = soa_stmt.query_map([&domain], |row| {
                                Ok(row.get::<_, String>(0)?)
                            }) {
                                zone_info.soa_record = soa_rows.next().and_then(|r| r.ok());
                            }
                        }

                        zones.push(zone_info);
                    }
                }
            }
        }
    }

    // Add default zone information from config
    if zones.is_empty() {
        if let Ok(config) = ServerConfig::from_env() {
            let default_zone = ZoneInfo {
                name: config.default_domain.clone(),
                ns_records: config.ns_records.clone(),
                soa_record: Some(format!(
                    "{} hostmaster.{} 1 10800 3600 604800 86400",
                    config.ns_records.first().unwrap_or(&String::from("ns1.example.com.")),
                    config.default_domain
                )),
            };
            zones.push(default_zone);
        }
    }

    zones
}

// Find the closest parent zone for a given domain
fn find_closest_parent_zone(domain: &str, zones: &[ZoneInfo]) -> Option<ZoneInfo> {
    let domain_parts: Vec<&str> = domain.split('.').collect();

    // Try progressively shorter parent domains
    for i in 0..domain_parts.len() {
        let candidate = domain_parts[i..].join(".");

        // Exact match
        if let Some(zone) = zones.iter().find(|z| z.name == candidate) {
            return Some(zone.clone());
        }
    }

    // Check if domain is a subdomain of any zone we're authoritative for
    for zone in zones {
        if domain.ends_with(&format!(".{}", zone.name)) {
            return Some(zone.clone());
        }
    }

    // If no match found, return None - we are not authoritative for this domain
    None
}

// Improved NXDOMAIN response builder with proper authority section
// Modified build_nxdomain_response function to correctly handle authority
fn build_nxdomain_response(query: &[u8], authoritative: bool) -> Option<Vec<u8>> {
    let mut resp = Vec::with_capacity(512);
    resp.extend_from_slice(&query[0..2]); // Transaction ID

    // Extract domain from query for authority section reference
    let domain = match extract_domain(query) {
        Some(d) => d,
        None => return None,
    };

    // Get zones we're authoritative for
    let config = match ServerConfig::from_env() {
        Ok(c) => c,
        Err(_) => return None,
    };

    let zones = get_authoritative_zones(&config.db_path);
    let parent_zone = find_closest_parent_zone(&domain, &zones);

    // Determine if we're actually authoritative for this domain
    let is_authoritative = parent_zone.is_some() && authoritative;

    // Get original flags but set QR bit and preserve RD
    let flags1 = 0x80 | (query[2] & 0x01); // QR=1, OPCODE=0, AA=0, TC=0, RD=preserved
    let flags2 = 0x83; // RA=1, Z=0, RCODE=3 (NXDOMAIN)

    resp.extend_from_slice(&[
        if is_authoritative { flags1 | 0x04 } else { flags1 }, // Set AA bit ONLY if truly authoritative
        flags2,
    ]);

    // Determine record counts for header
    let ns_count = if is_authoritative {
        parent_zone.as_ref().map(|z| z.ns_records.len()).unwrap_or(0)
    } else {
        0
    };

    let soa_count = if is_authoritative && parent_zone.as_ref().and_then(|z| z.soa_record.as_ref()).is_some() {
        1
    } else {
        0
    };

    let authority_count = ns_count + soa_count;

    resp.extend_from_slice(&query[4..6]); // QDCOUNT
    resp.extend_from_slice(&[0x00, 0x00]); // ANCOUNT
    resp.extend_from_slice(&(authority_count as u16).to_be_bytes()); // NSCOUNT

    // Check for EDNS
    let has_edns = has_opt_record(query);
    resp.extend_from_slice(&(if has_edns { 1u16 } else { 0u16 }).to_be_bytes()); // ARCOUNT

    // Question Section
    let qname_end = query[12..].iter().position(|&b| b == 0);
    if let Some(end) = qname_end {
        if 12 + end + 5 <= query.len() {
            resp.extend_from_slice(&query[12..12 + end + 5]);
        } else {
            return None;
        }
    } else {
        return None;
    }

    // Add Authority Section if we're authoritative
    if is_authoritative && parent_zone.is_some() {
        let zone = parent_zone.unwrap();
        let zone_name = encode_dns_name(&zone.name);

        // Add SOA record if available (recommended by RFC 2308)
        if let Some(soa) = zone.soa_record {
            // Name of the zone
            resp.extend_from_slice(&zone_name);

            // Type SOA (0x0006)
            resp.extend_from_slice(&[0x00, 0x06]);

            // Class IN (0x0001)
            resp.extend_from_slice(&[0x00, 0x01]);

            // TTL
            resp.extend_from_slice(&(DEFAULT_TTL as u32).to_be_bytes());

            // Parse and encode SOA record
            let soa_parts: Vec<&str> = soa.split_whitespace().collect();
            if soa_parts.len() >= 7 {
                // MNAME (primary nameserver)
                let mname = encode_dns_name(soa_parts[0]);
                // RNAME (responsible person mailbox)
                let rname = encode_dns_name(soa_parts[1]);

                // Calculate RDLENGTH (mname + rname + 5 x 32-bit integers)
                let rdlength = mname.len() + rname.len() + 20;
                resp.extend_from_slice(&(rdlength as u16).to_be_bytes());

                // Add MNAME and RNAME
                resp.extend_from_slice(&mname);
                resp.extend_from_slice(&rname);

                // Add the 5 SOA integers (SERIAL, REFRESH, RETRY, EXPIRE, MINIMUM)
                for i in 2..7 {
                    if let Ok(val) = soa_parts[i].parse::<u32>() {
                        resp.extend_from_slice(&val.to_be_bytes());
                    } else {
                        // Default values if parsing fails
                        resp.extend_from_slice(&match i {
                            2 => 1u32, // SERIAL
                            3 => 10800u32, // REFRESH
                            4 => 3600u32, // RETRY
                            5 => 604800u32, // EXPIRE
                            _ => 86400u32, // MINIMUM
                        }.to_be_bytes());
                    }
                }
            } else {
                // If SOA format is invalid, add a minimal valid SOA record
                let mname = encode_dns_name(&zone.ns_records.first().cloned().unwrap_or_else(|| "ns1.example.com.".to_string()));
                let rname = encode_dns_name(&format!("hostmaster.{}", zone.name));

                // Calculate RDLENGTH
                let rdlength = mname.len() + rname.len() + 20;
                resp.extend_from_slice(&(rdlength as u16).to_be_bytes());

                // MNAME and RNAME
                resp.extend_from_slice(&mname);
                resp.extend_from_slice(&rname);

                // SOA integers with reasonable defaults
                resp.extend_from_slice(&1u32.to_be_bytes()); // SERIAL
                resp.extend_from_slice(&10800u32.to_be_bytes()); // REFRESH
                resp.extend_from_slice(&3600u32.to_be_bytes()); // RETRY
                resp.extend_from_slice(&604800u32.to_be_bytes()); // EXPIRE
                resp.extend_from_slice(&86400u32.to_be_bytes()); // MINIMUM
            }
        }

        // Add NS records
        for ns in &zone.ns_records {
            // Name of the zone
            resp.extend_from_slice(&zone_name);

            // Type NS (0x0002)
            resp.extend_from_slice(&[0x00, 0x02]);

            // Class IN (0x0001)
            resp.extend_from_slice(&[0x00, 0x01]);

            // TTL
            resp.extend_from_slice(&(DEFAULT_TTL as u32).to_be_bytes());

            // RDLENGTH and RDATA (NS name)
            let ns_data = encode_dns_name(ns);
            resp.extend_from_slice(&(ns_data.len() as u16).to_be_bytes());
            resp.extend_from_slice(&ns_data);
        }
    }

    // Also need to fix the generate_dns_response function to check for proper authority
    // This part would go in generate_dns_response:
    /*
    // If not authoritative for this domain, don't set the AA flag
    if !config.authoritative || find_closest_parent_zone(&domain, &get_authoritative_zones(&config.db_path)).is_none() {
        // Remove the AA flag when forwarding
        return build_forwarded_response(query, &config.forwarders).await;
    }
    */

    // Handle EDNS in NXDOMAIN response
    if has_edns {
        let opt_payload_size = extract_edns_payload_size(query).unwrap_or(4096);

        // Copy DO bit if present in request
        let do_bit = extract_do_bit(query);

        // Add OPT record for EDNS
        resp.extend_from_slice(&[0x00]); // Root domain
        resp.extend_from_slice(&[0x00, 0x29]); // TYPE OPT
        resp.extend_from_slice(&opt_payload_size.to_be_bytes()); // UDP payload size from request
        resp.extend_from_slice(&[0x00]); // Extended RCODE
        resp.extend_from_slice(&[0x00]); // EDNS version

        if do_bit {
            resp.extend_from_slice(&[0x80, 0x00]); // Flags with DO bit set
        } else {
            resp.extend_from_slice(&[0x00, 0x00]); // Flags with DO bit clear
        }

        resp.extend_from_slice(&[0x00, 0x00]); // RDATA length
    }

    Some(resp)
}

// Add SOA record support to the database initialization
fn init_db(db_path: &str, default_domain: &str, default_ip: &str) -> Result<(), DnsError> {
    let conn = Connection::open(db_path)?;

    // Updated schema to allow multiple NS records
    conn.execute(
        "CREATE TABLE IF NOT EXISTS dns_records (
            domain TEXT NOT NULL,
            record_type TEXT NOT NULL CHECK(record_type IN (
                'A','AAAA','MX','TXT','NS','CNAME','PTR','SOA',
                'SRV','CAA','NAPTR','DS','DNSKEY','RRSIG','NSEC',
                'TLSA','SSHFP'
            )),
            value TEXT NOT NULL,
            ttl INTEGER DEFAULT 3600,
            PRIMARY KEY (domain, record_type, value)  -- Now allows multiple NS records
        ) WITHOUT ROWID",
        [],
    )?;

    let count: i64 = conn.query_row("SELECT COUNT(*) FROM dns_records", [], |row| row.get(0))?;

    if count == 0 && !default_ip.is_empty() {
        let mail_domain = format!("mail.{}", default_domain);
        let ns1 = format!("ns1.{}", default_domain);
        let ns2 = format!("ns2.{}", default_domain);
        let soa_record = format!("{} hostmaster.{} 1 10800 3600 604800 86400", ns1, default_domain);

        conn.execute_batch(
            &format!(
                r#"
                INSERT OR IGNORE INTO dns_records VALUES('{0}', 'A', ?, 3600);
                INSERT OR IGNORE INTO dns_records VALUES('www.{0}', 'A', ?, 3600);
                INSERT OR IGNORE INTO dns_records VALUES('api.{0}', 'A', ?, 3600);
                INSERT OR IGNORE INTO dns_records VALUES('mail.{0}', 'A', ?, 3600);
                INSERT OR IGNORE INTO dns_records VALUES('ns1.{0}', 'A', ?, 3600);
                INSERT OR IGNORE INTO dns_records VALUES('ns2.{0}', 'A', ?, 3600);
                INSERT OR IGNORE INTO dns_records VALUES('{0}', 'MX', '10 {1}', 3600);
                INSERT OR IGNORE INTO dns_records VALUES('{0}', 'TXT', '\"v=spf1 a mx ~all\"', 3600);
                INSERT OR IGNORE INTO dns_records VALUES('{0}', 'NS', '{2}', 3600);
                INSERT OR IGNORE INTO dns_records VALUES('{0}', 'NS', '{3}', 3600);
                INSERT OR IGNORE INTO dns_records VALUES('{0}', 'SOA', '{4}', 3600);
                "#,
                default_domain, mail_domain, ns1, ns2, soa_record
            ),
        )?;
    }

    Ok(())
}

// List line of the code: main.rs //