//! Configuration for the DNS server.
//!
//! This module defines the configuration structure and methods to load
//! configuration from environment variables.
#![allow(dead_code)]
#[allow(unused_variables)]

use std::{env, fs, net::SocketAddr};
use log::{error, info};

use crate::errors::DnsError;

/// Default TTL for DNS records in seconds.
pub const DEFAULT_TTL: u64 = 600;

/// Maximum size of DNS packets in bytes.
pub const MAX_PACKET_SIZE: usize = 4096;

/// Server configuration loaded from environment variables.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Address to bind the DNS server to.
    pub bind_addr: SocketAddr,
    
    /// Path to the SQLite database file.
    pub db_path: String,
    
    /// Time-to-live for cached DNS records.
    pub cache_ttl: u64,
    
    /// Whether to enable IPv6 support.
    pub enable_ipv6: bool,
    
    /// Maximum size of DNS packets.
    pub max_packet_size: usize,
    
    /// Whether this server is authoritative for its zones.
    pub authoritative: bool,
    
    /// List of NS records for zones this server is authoritative for.
    pub ns_records: Vec<String>,
    
    /// Default domain for the server.
    pub default_domain: String,
    
    /// Default IP address for the server.
    pub default_ip: String,
    
    /// List of upstream DNS servers to forward queries to.
    pub forwarders: Vec<SocketAddr>,
    
    /// List of DS records for DNSSEC.
    pub ds_records: Vec<String>,
    
    /// List of DNSKEY records for DNSSEC.
    pub dnskey_records: Vec<String>,
}

impl ServerConfig {
    /// Load server configuration from environment variables.
    ///
    /// # Returns
    /// A `Result` containing either the loaded `ServerConfig` or a `DnsError`.
    pub fn from_env() -> Result<Self, DnsError> {
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
                .unwrap_or_else(|_| vec!["ns1.yourdomain.tld.".into(), "ns2.yourdomain.tld.".into()]),
            default_domain: env::var("DNS_DEFAULT_DOMAIN").unwrap_or_else(|_| "bzo.in".into()),
            default_ip: env::var("DNS_DEFAULT_IP").unwrap_or_else(|_| "<your-public-ip4-here>".into()),
            ds_records: vec![
                "yourdomain.tld. IN DS 24550 8 2 1F21CA282945434EE0662805430599CB2A6C479D9F934087150901CE2DA580A0".to_string()
            ],
            dnskey_records,
            forwarders,
        })
    }
}