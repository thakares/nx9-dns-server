//! Database operations for the DNS server.
//!
//! This module provides functions for interacting with the SQLite database
//! that stores DNS records and zone information.

use rusqlite::{params, Connection};

use crate::errors::DnsError;
use crate::config::ServerConfig;

/// Information about a DNS zone.
#[derive(Debug, Clone)]
pub struct ZoneInfo {
    /// The domain name of the zone.
    pub name: String,
    
    /// List of NS records for the zone.
    pub ns_records: Vec<String>,
    
    /// SOA record for the zone, if available.
    pub soa_record: Option<String>,
}

/// Initialize the DNS database.
///
/// Creates the database schema if it doesn't exist and populates it with default records
/// if the database is empty.
///
/// # Arguments
/// * `db_path` - Path to the SQLite database file.
/// * `default_domain` - Default domain name to use for initial records.
/// * `default_ip` - Default IP address to use for initial records.
///
/// # Returns
/// A `Result` indicating success or failure.
pub fn init_db(db_path: &str, default_domain: &str, default_ip: &str) -> Result<(), DnsError> {
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

/// Look up DNS records for a domain.
///
/// # Arguments
/// * `db_path` - Path to the SQLite database file.
/// * `domain` - Domain name to look up.
///
/// # Returns
/// A vector of tuples containing (value, ttl, record_type) for each record found.
pub fn lookup_records(db_path: &str, domain: &str) -> Vec<(String, u64, String)> {
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

/// Get information about all zones for which this server is authoritative.
///
/// # Arguments
/// * `db_path` - Path to the SQLite database file.
///
/// # Returns
/// A vector of `ZoneInfo` structs containing information about each zone.
pub fn get_authoritative_zones(db_path: &str) -> Vec<ZoneInfo> {
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

/// Find the closest parent zone for a given domain.
///
/// # Arguments
/// * `domain` - Domain name to find the parent zone for.
/// * `zones` - List of zones to search in.
///
/// # Returns
/// An `Option` containing the closest parent zone, if found.
pub fn find_closest_parent_zone(domain: &str, zones: &[ZoneInfo]) -> Option<ZoneInfo> {
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