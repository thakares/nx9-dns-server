//! Utility functions for DNS operations.
//!
//! This module provides helper functions for parsing and encoding DNS data.
#![allow(dead_code)]
#[allow(unused_variables)]

use std::str;
use chrono::{NaiveDateTime, TimeZone, Utc};

use crate::errors::DnsError;

/// Extract the domain name from a DNS query packet.
///
/// # Arguments
/// * `query` - The DNS query packet.
///
/// # Returns
/// An `Option` containing the domain name if successfully extracted.
pub fn extract_domain(query: &[u8]) -> Option<String> {
    if query.len() < 12 {
        return None; // DNS header is 12 bytes
    }

    let mut pos = 12; // Start after header
    let mut domain = String::new();

    // Extract QNAME (domain)
    loop {
        if pos >= query.len() {
            return None;
        }
        
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

/// Extract the query type from a DNS query packet.
///
/// # Arguments
/// * `query` - The DNS query packet.
///
/// # Returns
/// An `Option` containing the query type as a u16 if successfully extracted.
pub fn extract_query_type(query: &[u8]) -> Option<u16> {
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

/// Encode a domain name in DNS wire format.
///
/// # Arguments
/// * `name` - The domain name to encode.
///
/// # Returns
/// A vector of bytes containing the encoded domain name.
pub fn encode_dns_name(name: &str) -> Vec<u8> {
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

/// Parse a signature time in YYYYMMDDHHMMSS format to seconds since epoch.
///
/// # Arguments
/// * `s` - The signature time string.
///
/// # Returns
/// A `Result` containing the parsed time as a u32 or an error.
pub fn parse_sig_time(s: &str) -> Result<u32, DnsError> {
    let dt = NaiveDateTime::parse_from_str(s, "%Y%m%d%H%M%S")
        .map_err(|e| DnsError::Config(format!("Invalid sigtime: {e}")))?;
    Ok(Utc.from_utc_datetime(&dt).timestamp() as u32)
}

/// Check if a DNS query packet has an OPT record (EDNS).
///
/// # Arguments
/// * `query` - The DNS query packet.
///
/// # Returns
/// A boolean indicating whether the query has an OPT record.
pub fn has_opt_record(query: &[u8]) -> bool {
    if query.len() < 12 {
        return false;
    }

    // Get ARCOUNT (number of additional records)
    let arcount = ((query[10] as u16) << 8) | query[11] as u16;
    if arcount == 0 {
        return false;
    }

    // Skip header
    let mut pos = 12;

    // Skip question section
    // First skip QNAME
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

    // Skip answer and authority sections
    let ancount = ((query[6] as u16) << 8) | query[7] as u16;
    let nscount = ((query[8] as u16) << 8) | query[9] as u16;

    for _ in 0..(ancount + nscount) {
        // Skip name
        if pos >= query.len() {
            return false;
        }

        // Handle compression pointers
        if (query[pos] & 0xC0) == 0xC0 {
            pos += 2; // Skip compression pointer
        } else {
            // Skip labels
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
        }

        // Skip TYPE, CLASS, TTL, RDLENGTH, RDATA
        if pos + 10 > query.len() {
            return false;
        }
        let rdlength = ((query[pos + 8] as usize) << 8) | query[pos + 9] as usize;
        pos += 10 + rdlength;
    }

    // Check additional records for OPT
    for _ in 0..arcount {
        if pos >= query.len() {
            return false;
        }

        // OPT record has empty (root) name
        if query[pos] == 0 {
            // Check if TYPE is OPT (41)
            if pos + 2 < query.len() && query[pos + 1] == 0 && query[pos + 2] == 41 {
                return true;
            }
        }

        // Skip this record
        if pos + 10 >= query.len() { break; }
        let rdlength = ((query[pos + 8] as usize) << 8) | query[pos + 9] as usize;
        pos += 10 + rdlength;
    }

    false // No OPT record found
}

/// Extract the EDNS payload size from a DNS query packet.
///
/// # Arguments
/// * `query` - The DNS query packet.
///
/// # Returns
/// An `Option` containing the EDNS payload size if found.
pub fn extract_edns_payload_size(query: &[u8]) -> Option<u16> {
    if query.len() < 12 {
        return None;
    }

    // Get ARCOUNT (number of additional records)
    let arcount = ((query[10] as u16) << 8) | query[11] as u16;
    if arcount == 0 {
        return None;
    }

    // Skip header
    let mut pos = 12;

    // Skip question section
    // First skip QNAME
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

    // Skip answer and authority sections
    let ancount = ((query[6] as u16) << 8) | query[7] as u16;
    let nscount = ((query[8] as u16) << 8) | query[9] as u16;

    for _ in 0..(ancount + nscount) {
        // Skip name
        if pos >= query.len() {
            return None;
        }

        // Handle compression pointers
        if (query[pos] & 0xC0) == 0xC0 {
            pos += 2; // Skip compression pointer
        } else {
            // Skip labels
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
        }

        // Skip TYPE, CLASS, TTL, RDLENGTH, RDATA
        if pos + 10 > query.len() {
            return None;
        }
        let rdlength = ((query[pos + 8] as usize) << 8) | query[pos + 9] as usize;
        pos += 10 + rdlength;
    }

    // Check additional records for OPT
    for _ in 0..arcount {
        if pos >= query.len() {
            return None;
        }

        // OPT record has empty (root) name
        if query[pos] == 0 {
            // Check if TYPE is OPT (41)
            if pos + 5 < query.len() && query[pos + 1] == 0 && query[pos + 2] == 41 {
                // Extract UDP payload size (CLASS field in OPT record)
                return Some(((query[pos + 3] as u16) << 8) | query[pos + 4] as u16);
            }
        }

        // Skip this record
        if pos + 10 >= query.len() { break; }
        let rdlength = ((query[pos + 8] as usize) << 8) | query[pos + 9] as usize;
        pos += 10 + rdlength;
    }

    None // No OPT record found
}

/// Extract the DO (DNSSEC OK) bit from a DNS query packet.
///
/// # Arguments
/// * `query` - The DNS query packet.
///
/// # Returns
/// A boolean indicating whether the DO bit is set.
pub fn extract_do_bit(query: &[u8]) -> bool {
    if query.len() < 12 {
        return false;
    }

    // Get ARCOUNT (number of additional records)
    let arcount = ((query[10] as u16) << 8) | query[11] as u16;
    if arcount == 0 {
        return false;
    }

    // Skip header
    let mut pos = 12;

    // Skip question section
    // First skip QNAME
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

    // Skip answer and authority sections
    let ancount = ((query[6] as u16) << 8) | query[7] as u16;
    let nscount = ((query[8] as u16) << 8) | query[9] as u16;

    for _ in 0..(ancount + nscount) {
        // Skip name
        if pos >= query.len() {
            return false;
        }

        // Handle compression pointers
        if (query[pos] & 0xC0) == 0xC0 {
            pos += 2; // Skip compression pointer
        } else {
            // Skip labels
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
        }

        // Skip TYPE, CLASS, TTL, RDLENGTH, RDATA
        if pos + 10 > query.len() {
            return false;
        }
        let rdlength = ((query[pos + 8] as usize) << 8) | query[pos + 9] as usize;
        pos += 10 + rdlength;
    }

    // Check additional records for OPT
    for _ in 0..arcount {
        if pos >= query.len() {
            return false;
        }

        // OPT record has empty (root) name
        if query[pos] == 0 {
            // Check if TYPE is OPT (41)
            if pos + 7 < query.len() && query[pos + 1] == 0 && query[pos + 2] == 41 {
                // Check DO bit (bit 15 of TTL field, which is used for flags in OPT)
                return (query[pos + 6] & 0x80) != 0;
            }
        }

        // Skip this record
        if pos + 10 >= query.len() { break; }
        let rdlength = ((query[pos + 8] as usize) << 8) | query[pos + 9] as usize;
        pos += 10 + rdlength;
    }

    false // No OPT record found or no DO bit set
}