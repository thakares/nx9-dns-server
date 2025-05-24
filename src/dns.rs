//! DNS protocol implementation.
//!
//! This module provides functions for building and parsing DNS messages.
#![allow(dead_code)]
#[allow(unused_variables)]

use std::net::SocketAddr;
use std::io;
use log::info;
use tokio::net::{TcpStream, UdpSocket};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use base64::Engine;

use crate::errors::DnsError;
use crate::config::{ServerConfig, DEFAULT_TTL};
use crate::utils::{encode_dns_name, extract_domain, extract_query_type, has_opt_record, extract_edns_payload_size, extract_do_bit, parse_sig_time};
use crate::db::{lookup_records, get_authoritative_zones, find_closest_parent_zone};
use crate::cache::CACHE;

/// Encode a RRSIG record.
///
/// # Arguments
/// * `rrsig_record` - The RRSIG record string.
/// * `ttl` - Time-to-live in seconds.
///
/// # Returns
/// A `Result` containing the encoded RRSIG record or an error.
pub fn encode_rrsig_rr(
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

/// Forward a DNS query to upstream resolvers using UDP.
///
/// # Arguments
/// * `forwarder` - The upstream resolver to forward to.
/// * `query` - The DNS query to forward.
///
/// # Returns
/// A `Result` containing the response or an error.
pub async fn forward_request_udp(forwarder: SocketAddr, query: &[u8]) -> io::Result<Vec<u8>> {
    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    socket.send_to(query, forwarder).await?;

    let mut buf = vec![0u8; 4096];
    let (size, _) = socket.recv_from(&mut buf).await?;
    Ok(buf[..size].to_vec())
}

/// Forward a DNS query to upstream resolvers using TCP.
///
/// # Arguments
/// * `forwarder` - The upstream resolver to forward to.
/// * `query` - The DNS query to forward.
///
/// # Returns
/// A `Result` containing the response or an error.
pub async fn forward_request_tcp(forwarder: SocketAddr, query: &[u8]) -> io::Result<Vec<u8>> {
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

/// Forward a DNS query to upstream resolvers.
///
/// # Arguments
/// * `query` - The DNS query to forward.
/// * `forwarders` - List of upstream resolvers to try.
///
/// # Returns
/// An `Option` containing the response if successful.
pub async fn forward_to_resolvers(query: &[u8], forwarders: &[SocketAddr]) -> Option<Vec<u8>> {
    for &forwarder in forwarders {
        info!("Forwarding query to resolver: {}", forwarder);
        if let Ok(resp) = forward_request_udp(forwarder, query).await {
            info!("Received response from resolver: {}", forwarder);
            return Some(resp);
        }
    }
    None
}

/// Forward a DNS query to upstream resolvers using TCP.
///
/// # Arguments
/// * `query` - The DNS query to forward.
/// * `forwarders` - List of upstream resolvers to try.
///
/// # Returns
/// An `Option` containing the response if successful.
pub async fn forward_to_resolvers_tcp(query: &[u8], forwarders: &[SocketAddr]) -> Option<Vec<u8>> {
    for &forwarder in forwarders {
        if let Ok(resp) = forward_request_tcp(forwarder, query).await {
            return Some(resp);
        }
    }
    None
}

/// Send a DNS response over TCP.
///
/// # Arguments
/// * `stream` - The TCP stream to send the response on.
/// * `response` - The DNS response to send.
///
/// # Returns
/// A `Result` indicating success or failure.
pub async fn send_tcp_response(stream: &mut TcpStream, response: &[u8]) -> io::Result<()> {
    stream.write_all(&(response.len() as u16).to_be_bytes()).await?;
    stream.write_all(response).await
}

/// Generate a DNS response for a query.
///
/// # Arguments
/// * `query` - The DNS query.
/// * `domain` - The domain name from the query.
/// * `config` - The server configuration.
///
/// # Returns
/// A `Result` containing the response or an error.
pub async fn generate_dns_response(
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
        return build_nxdomain_response(query, config.authoritative)
            .ok_or(DnsError::Protocol("No DNSKEY record".into()));
    }

    // Handle DS queries (type 43)
    if query_type == 43 {
        if let Some(ds) = config.ds_records.first() {
            return build_ds_response(query, ds, 3600, config);
        }
        return build_nxdomain_response(query, config.authoritative)
            .ok_or(DnsError::Protocol("NXDOMAIN".into()));
    }

    // Check cache for A/AAAA queries
    if query_type == 1 || query_type == 28 {
        if let Some((ip, ttl)) = CACHE.get().unwrap().get(&domain) {
            return build_dns_response(query, &ip, ttl, config);
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
            "SOA" => build_soa_response(query, &value, ttl, domain, config),
            "NS" => build_ns_response(query, &records, ttl, domain, config),
            "MX" | "TXT" | "CNAME" | "PTR" => {
                build_generic_record_response(query, &value, ttl, domain, query_type, config)
            },
            "A" | "AAAA" => {
                CACHE.get().unwrap().set(domain.clone(), value.clone(), ttl);
                build_dns_response(query, &value, ttl, config)
            },
            _ => Err(DnsError::Protocol("Unsupported record type".into()))
        };
    }

    // Fallback for A/AAAA queries
    if query_type == 1 || query_type == 28 {
        if let Some((ip, ttl, _)) = records.iter()
            .find(|(_, _, rtype)| rtype == "A")
            .cloned()
        {
            CACHE.get().unwrap().set(domain.clone(), ip.clone(), ttl);
            return build_dns_response(query, &ip, ttl, config);
        }
    }

    // If we're authoritative for this domain, return NXDOMAIN
    let zones = get_authoritative_zones(&config.db_path);
    if config.authoritative && find_closest_parent_zone(&domain, &zones).is_some() {
        return build_nxdomain_response(query, true)
            .ok_or(DnsError::Protocol("NXDOMAIN".into()));
    }

    // Forward to upstream resolvers
    if let Some(response) = forward_to_resolvers(query, &config.forwarders).await {
        Ok(response)
    } else if let Some(response) = forward_to_resolvers_tcp(query, &config.forwarders).await {
        Ok(response)
    } else {
        Err(DnsError::Protocol("Failed to resolve domain".into()))
    }
}

/// Build a DNS response for an A or AAAA record.
///
/// # Arguments
/// * `query` - The DNS query.
/// * `ip` - The IP address for the response.
/// * `ttl` - Time-to-live in seconds.
/// * `config` - The server configuration.
///
/// # Returns
/// A `Result` containing the response or an error.
pub fn build_dns_response(
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

    // TTL
    response.extend_from_slice(&(ttl as u32).to_be_bytes());

    // Parse IP address
    let octets: Vec<u8> = ip.split('.')
        .filter_map(|s| s.parse::<u8>().ok())
        .collect();

    if octets.len() != 4 {
        return Err(DnsError::Protocol(format!("Invalid IPv4 address: {}", ip)));
    }

    // RDLENGTH (4 for IPv4)
    response.extend_from_slice(&[0x00, 0x04]);

    // RDATA (IP address)
    response.extend_from_slice(&octets);

    // Add EDNS record if present in query
    if has_opt_record(query) {
        let opt_payload_size = extract_edns_payload_size(query).unwrap_or(4096);
        let do_bit = extract_do_bit(query);

        // Add OPT record
        response.extend_from_slice(&[0x00]); // Root domain
        response.extend_from_slice(&[0x00, 0x29]); // TYPE OPT
        response.extend_from_slice(&opt_payload_size.to_be_bytes()); // UDP payload size
        response.extend_from_slice(&[0x00]); // Extended RCODE
        response.extend_from_slice(&[0x00]); // EDNS version

        if do_bit {
            response.extend_from_slice(&[0x80, 0x00]); // Flags with DO bit set
        } else {
            response.extend_from_slice(&[0x00, 0x00]); // Flags with DO bit clear
        }

        response.extend_from_slice(&[0x00, 0x00]); // RDATA length
    }

    Ok(response)
}

/// Build a DNS response for a DS record.
///
/// # Arguments
/// * `query` - The DNS query.
/// * `ds_record` - The DS record string.
/// * `ttl` - Time-to-live in seconds.
/// * `config` - The server configuration.
///
/// # Returns
/// A `Result` containing the response or an error.
pub fn build_ds_response(
    query: &[u8],
    ds_record: &str,
    ttl: u64,
    config: &ServerConfig,
) -> Result<Vec<u8>, DnsError> {
    let mut response = Vec::with_capacity(512);

    // Copy transaction ID and question from query
    response.extend_from_slice(&query[..2]);

    // Set flags
    let flags1 = 0x84; // QR=1, AA=1, RD=0
    let flags2 = 0x00; // RA=0, Z=0, RCODE=0

    response.extend_from_slice(&[flags1, flags2]);

    // Copy QDCOUNT from query
    response.extend_from_slice(&query[4..6]);

    // Set ANCOUNT to 1
    response.extend_from_slice(&[0x00, 0x01]);

    // Set NSCOUNT to 0
    response.extend_from_slice(&[0x00, 0x00]);

    // Set ARCOUNT to 1 if EDNS is present
    let has_edns = has_opt_record(query);
    response.extend_from_slice(&[0x00, if has_edns { 0x01 } else { 0x00 }]);

    // Copy question section from query
    let qname_end = query[12..].iter().position(|&b| b == 0)
        .ok_or_else(|| DnsError::Protocol("Invalid question format".into()))? + 13;
    response.extend_from_slice(&query[12..qname_end + 4]);

    // Parse DS record
    // Format: "yourdomain.tld. IN DS 24550 8 2 1F21CA282945434EE0662805430599CB2A6C479D9F934087150901CE2DA580A0"
    let parts: Vec<&str> = ds_record.split_whitespace().collect();
    if parts.len() < 7 {
        return Err(DnsError::Config(format!("Invalid DS record format: {}", ds_record)));
    }

    let key_tag = parts[3].parse::<u16>()
        .map_err(|_| DnsError::Config(format!("Invalid key tag: {}", parts[3])))?;
    let algorithm = parts[4].parse::<u8>()
        .map_err(|_| DnsError::Config(format!("Invalid algorithm: {}", parts[4])))?;
    let digest_type = parts[5].parse::<u8>()
        .map_err(|_| DnsError::Config(format!("Invalid digest type: {}", parts[5])))?;
    let digest = hex::decode(parts[6])
        .map_err(|_| DnsError::Config(format!("Invalid digest: {}", parts[6])))?;

    // Add answer section
    // Name pointer to question
    response.extend_from_slice(&[0xc0, 0x0c]);

    // Type DS (0x002B)
    response.extend_from_slice(&[0x00, 0x2B]);

    // Class IN (0x0001)
    response.extend_from_slice(&[0x00, 0x01]);

    // TTL
    response.extend_from_slice(&(ttl as u32).to_be_bytes());

    // RDLENGTH
    let rdlength = 4 + digest.len(); // 2 + 1 + 1 + digest.len()
    response.extend_from_slice(&(rdlength as u16).to_be_bytes());

    // RDATA
    response.extend_from_slice(&key_tag.to_be_bytes());
    response.push(algorithm);
    response.push(digest_type);
    response.extend_from_slice(&digest);

    // Add EDNS record if present in query
    if has_edns {
        let opt_payload_size = extract_edns_payload_size(query).unwrap_or(4096);
        let do_bit = extract_do_bit(query);

        // Add OPT record
        response.extend_from_slice(&[0x00]); // Root domain
        response.extend_from_slice(&[0x00, 0x29]); // TYPE OPT
        response.extend_from_slice(&opt_payload_size.to_be_bytes()); // UDP payload size
        response.extend_from_slice(&[0x00]); // Extended RCODE
        response.extend_from_slice(&[0x00]); // EDNS version

        if do_bit {
            response.extend_from_slice(&[0x80, 0x00]); // Flags with DO bit set
        } else {
            response.extend_from_slice(&[0x00, 0x00]); // Flags with DO bit clear
        }

        response.extend_from_slice(&[0x00, 0x00]); // RDATA length
    }

    Ok(response)
}

/// Build a DNS response for a DNSKEY record.
///
/// # Arguments
/// * `query` - The DNS query.
/// * `dnskey_record` - The DNSKEY record string.
/// * `ttl` - Time-to-live in seconds.
/// * `config` - The server configuration.
///
/// # Returns
/// A `Result` containing the response or an error.
pub fn build_dnskey_response(
    query: &[u8],
    dnskey_record: &str,
    ttl: u64,
    config: &ServerConfig,
) -> Result<Vec<u8>, DnsError> {
    let mut response = Vec::with_capacity(512);

    // Copy transaction ID and question from query
    response.extend_from_slice(&query[..2]);

    // Set flags
    let flags1 = 0x84; // QR=1, AA=1, RD=0
    let flags2 = 0x00; // RA=0, Z=0, RCODE=0

    response.extend_from_slice(&[flags1, flags2]);

    // Copy QDCOUNT from query
    response.extend_from_slice(&query[4..6]);

    // Set ANCOUNT to 1
    response.extend_from_slice(&[0x00, 0x01]);

    // Set NSCOUNT to 0
    response.extend_from_slice(&[0x00, 0x00]);

    // Set ARCOUNT to 1 if EDNS is present, 0 otherwise
    let has_edns = has_opt_record(query);
    response.extend_from_slice(&[0x00, if has_edns { 0x01 } else { 0x00 }]);

    // Copy question section from query
    let qname_end = query[12..].iter().position(|&b| b == 0)
        .ok_or_else(|| DnsError::Protocol("Invalid question format".into()))? + 13;
    response.extend_from_slice(&query[12..qname_end + 4]);

    // Parse DNSKEY record
    // Format: "yourdomain.tld. IN DNSKEY 256 3 8 AwEAAb/xrM..."
    let parts: Vec<&str> = dnskey_record.split_whitespace().collect();
    if parts.len() < 7 {
        return Err(DnsError::Config(format!("Invalid DNSKEY record format: {}", dnskey_record)));
    }

    let flags = parts[3].parse::<u16>()
        .map_err(|_| DnsError::Config(format!("Invalid flags: {}", parts[3])))?;
    let protocol = parts[4].parse::<u8>()
        .map_err(|_| DnsError::Config(format!("Invalid protocol: {}", parts[4])))?;
    let algorithm = parts[5].parse::<u8>()
        .map_err(|_| DnsError::Config(format!("Invalid algorithm: {}", parts[5])))?;
    let public_key = parts[6..].join("");

    // Decode base64 public key
    let key_data = base64::engine::general_purpose::STANDARD
        .decode(&public_key)
        .map_err(|e| DnsError::Base64(e.to_string()))?;

    // Add answer section
    // Name pointer to question
    response.extend_from_slice(&[0xc0, 0x0c]);

    // Type DNSKEY (0x0030)
    response.extend_from_slice(&[0x00, 0x30]);

    // Class IN (0x0001)
    response.extend_from_slice(&[0x00, 0x01]);

    // TTL
    response.extend_from_slice(&(ttl as u32).to_be_bytes());

    // RDLENGTH
    let rdlength = 4 + key_data.len(); // 2 + 1 + 1 + key_data.len()
    response.extend_from_slice(&(rdlength as u16).to_be_bytes());

    // RDATA
    response.extend_from_slice(&flags.to_be_bytes());
    response.push(protocol);
    response.push(algorithm);
    response.extend_from_slice(&key_data);

    // Add EDNS record if present in query
    if has_edns {
        let opt_payload_size = extract_edns_payload_size(query).unwrap_or(4096);
        let do_bit = extract_do_bit(query);

        // Add OPT record
        response.extend_from_slice(&[0x00]); // Root domain
        response.extend_from_slice(&[0x00, 0x29]); // TYPE OPT
        response.extend_from_slice(&opt_payload_size.to_be_bytes()); // UDP payload size
        response.extend_from_slice(&[0x00]); // Extended RCODE
        response.extend_from_slice(&[0x00]); // EDNS version

        if do_bit {
            response.extend_from_slice(&[0x80, 0x00]); // Flags with DO bit set
        } else {
            response.extend_from_slice(&[0x00, 0x00]); // Flags with DO bit clear
        }

        response.extend_from_slice(&[0x00, 0x00]); // RDATA length
    }

    Ok(response)
}

/// Build a DNS response for a SOA record.
///
/// # Arguments
/// * `query` - The DNS query.
/// * `soa_value` - The SOA record string.
/// * `ttl` - Time-to-live in seconds.
/// * `domain` - The domain name from the query.
/// * `config` - The server configuration.
///
/// # Returns
/// A `Result` containing the response or an error.
pub fn build_soa_response(
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
    let flags1 = 0x84; // QR=1, AA=1, RD=0
    let flags2 = 0x00; // RA=0, Z=0, RCODE=0

    response.extend_from_slice(&[flags1, flags2]);

    // Copy QDCOUNT from query
    response.extend_from_slice(&query[4..6]);

    // Set ANCOUNT to 1
    response.extend_from_slice(&[0x00, 0x01]);

    // Set NSCOUNT to 0
    response.extend_from_slice(&[0x00, 0x00]);

    // Set ARCOUNT to 1 if EDNS is present, 0 otherwise
    let has_edns = has_opt_record(query);
    response.extend_from_slice(&[0x00, if has_edns { 0x01 } else { 0x00 }]);

    // Copy question section from query
    let qname_end = query[12..].iter().position(|&b| b == 0)
        .ok_or_else(|| DnsError::Protocol("Invalid question format".into()))? + 13;
    response.extend_from_slice(&query[12..qname_end + 4]);

    // Parse SOA record
    // Format: "ns1.example.com. hostmaster.example.com. 1 10800 3600 604800 86400"
    let parts: Vec<&str> = soa_value.split_whitespace().collect();
    if parts.len() < 7 {
        return Err(DnsError::Config(format!("Invalid SOA record format: {}", soa_value)));
    }

    let mname = parts[0]; // Primary nameserver
    let rname = parts[1]; // Hostmaster email
    let serial = parts[2].parse::<u32>()
        .map_err(|_| DnsError::Config(format!("Invalid serial: {}", parts[2])))?;
    let refresh = parts[3].parse::<u32>()
        .map_err(|_| DnsError::Config(format!("Invalid refresh: {}", parts[3])))?;
    let retry = parts[4].parse::<u32>()
        .map_err(|_| DnsError::Config(format!("Invalid retry: {}", parts[4])))?;
    let expire = parts[5].parse::<u32>()
        .map_err(|_| DnsError::Config(format!("Invalid expire: {}", parts[5])))?;
    let minimum = parts[6].parse::<u32>()
        .map_err(|_| DnsError::Config(format!("Invalid minimum: {}", parts[6])))?;

    // Add answer section
    // Name pointer to question
    response.extend_from_slice(&[0xc0, 0x0c]);

    // Type SOA (0x0006)
    response.extend_from_slice(&[0x00, 0x06]);

    // Class IN (0x0001)
    response.extend_from_slice(&[0x00, 0x01]);

    // TTL
    response.extend_from_slice(&(ttl as u32).to_be_bytes());

    // Encode MNAME and RNAME
    let mname_wire = encode_dns_name(mname);
    let rname_wire = encode_dns_name(rname);

    // RDLENGTH
    let rdlength = mname_wire.len() + rname_wire.len() + 20; // 5 32-bit integers
    response.extend_from_slice(&(rdlength as u16).to_be_bytes());

    // RDATA
    response.extend_from_slice(&mname_wire);
    response.extend_from_slice(&rname_wire);
    response.extend_from_slice(&serial.to_be_bytes());
    response.extend_from_slice(&refresh.to_be_bytes());
    response.extend_from_slice(&retry.to_be_bytes());
    response.extend_from_slice(&expire.to_be_bytes());
    response.extend_from_slice(&minimum.to_be_bytes());

    // Add EDNS record if present in query
    if has_edns {
        let opt_payload_size = extract_edns_payload_size(query).unwrap_or(4096);
        let do_bit = extract_do_bit(query);

        // Add OPT record
        response.extend_from_slice(&[0x00]); // Root domain
        response.extend_from_slice(&[0x00, 0x29]); // TYPE OPT
        response.extend_from_slice(&opt_payload_size.to_be_bytes()); // UDP payload size
        response.extend_from_slice(&[0x00]); // Extended RCODE
        response.extend_from_slice(&[0x00]); // EDNS version

        if do_bit {
            response.extend_from_slice(&[0x80, 0x00]); // Flags with DO bit set
        } else {
            response.extend_from_slice(&[0x00, 0x00]); // Flags with DO bit clear
        }

        response.extend_from_slice(&[0x00, 0x00]); // RDATA length
    }

    Ok(response)
}

/// Build a DNS response for NS records.
///
/// # Arguments
/// * `query` - The DNS query.
/// * `records` - The DNS records for the domain.
/// * `ttl` - Time-to-live in seconds.
/// * `domain` - The domain name from the query.
/// * `config` - The server configuration.
///
/// # Returns
/// A `Result` containing the response or an error.
pub fn build_ns_response(
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
    let flags1 = 0x84; // QR=1, AA=1, RD=0
    let flags2 = 0x00; // RA=0, Z=0, RCODE=0

    response.extend_from_slice(&[flags1, flags2]);

    // Copy QDCOUNT from query
    response.extend_from_slice(&query[4..6]);

    // Count NS records
    let ns_records: Vec<&(String, u64, String)> = records.iter()
        .filter(|(_, _, rtype)| rtype == "NS")
        .collect();

    // Set ANCOUNT to number of NS records
    response.extend_from_slice(&(ns_records.len() as u16).to_be_bytes());

    // Set NSCOUNT to 0
    response.extend_from_slice(&[0x00, 0x00]);

    // Set ARCOUNT to 1 if EDNS is present, 0 otherwise
    let has_edns = has_opt_record(query);
    response.extend_from_slice(&[0x00, if has_edns { 0x01 } else { 0x00 }]);

    // Copy question section from query
    let qname_end = query[12..].iter().position(|&b| b == 0)
        .ok_or_else(|| DnsError::Protocol("Invalid question format".into()))? + 13;
    response.extend_from_slice(&query[12..qname_end + 4]);

    // Add answer section for each NS record
    for (ns_value, ns_ttl, _) in ns_records {
        // Name pointer to question
        response.extend_from_slice(&[0xc0, 0x0c]);

        // Type NS (0x0002)
        response.extend_from_slice(&[0x00, 0x02]);

        // Class IN (0x0001)
        response.extend_from_slice(&[0x00, 0x01]);

        // TTL
        response.extend_from_slice(&(*ns_ttl as u32).to_be_bytes());

        // Encode NS name
        let ns_data = encode_dns_name(ns_value);

        // RDLENGTH
        response.extend_from_slice(&(ns_data.len() as u16).to_be_bytes());

        // RDATA (NS name)
        response.extend_from_slice(&ns_data);
    }

    // Add EDNS record if present in query
    if has_edns {
        let opt_payload_size = extract_edns_payload_size(query).unwrap_or(4096);
        let do_bit = extract_do_bit(query);

        // Add OPT record
        response.extend_from_slice(&[0x00]); // Root domain
        response.extend_from_slice(&[0x00, 0x29]); // TYPE OPT
        response.extend_from_slice(&opt_payload_size.to_be_bytes()); // UDP payload size
        response.extend_from_slice(&[0x00]); // Extended RCODE
        response.extend_from_slice(&[0x00]); // EDNS version

        if do_bit {
            response.extend_from_slice(&[0x80, 0x00]); // Flags with DO bit set
        } else {
            response.extend_from_slice(&[0x00, 0x00]); // Flags with DO bit clear
        }

        response.extend_from_slice(&[0x00, 0x00]); // RDATA length
    }

    Ok(response)
}

/// Build a DNS response for generic record types (MX, TXT, CNAME, PTR).
///
/// # Arguments
/// * `query` - The DNS query.
/// * `value` - The record value.
/// * `ttl` - Time-to-live in seconds.
/// * `domain` - The domain name from the query.
/// * `query_type` - The query type.
/// * `config` - The server configuration.
///
/// # Returns
/// A `Result` containing the response or an error.
pub fn build_generic_record_response(
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
    let flags1 = 0x84; // QR=1, AA=1, RD=0
    let flags2 = 0x00; // RA=0, Z=0, RCODE=0

    response.extend_from_slice(&[flags1, flags2]);

    // Copy QDCOUNT from query
    response.extend_from_slice(&query[4..6]);

    // Set ANCOUNT to 1
    response.extend_from_slice(&[0x00, 0x01]);

    // Set NSCOUNT to 0
    response.extend_from_slice(&[0x00, 0x00]);

    // Set ARCOUNT to 1 if EDNS is present, 0 otherwise
    let has_edns = has_opt_record(query);
    response.extend_from_slice(&[0x00, if has_edns { 0x01 } else { 0x00 }]);

    // Copy question section from query
    let qname_end = query[12..].iter().position(|&b| b == 0)
        .ok_or_else(|| DnsError::Protocol("Invalid question format".into()))? + 13;
    response.extend_from_slice(&query[12..qname_end + 4]);

    // Add answer section
    // Name pointer to question
    response.extend_from_slice(&[0xc0, 0x0c]);

    // Type
    response.extend_from_slice(&query_type.to_be_bytes());

    // Class IN (0x0001)
    response.extend_from_slice(&[0x00, 0x01]);

    // TTL
    response.extend_from_slice(&(ttl as u32).to_be_bytes());

    // RDATA depends on record type
    match query_type {
        // MX record
        15 => {
            // Parse MX record: "10 mail.example.com."
            let parts: Vec<&str> = value.split_whitespace().collect();
            if parts.len() < 2 {
                return Err(DnsError::Config(format!("Invalid MX record format: {}", value)));
            }

            let preference = parts[0].parse::<u16>()
                .map_err(|_| DnsError::Config(format!("Invalid MX preference: {}", parts[0])))?;
            let exchange = parts[1];

            // Encode exchange name
            let exchange_wire = encode_dns_name(exchange);

            // RDLENGTH
            let rdlength = 2 + exchange_wire.len(); // preference + exchange
            response.extend_from_slice(&(rdlength as u16).to_be_bytes());

            // RDATA
            response.extend_from_slice(&preference.to_be_bytes());
            response.extend_from_slice(&exchange_wire);
        },

        // TXT record
        16 => {
            // Remove quotes if present
            let txt_value = value.trim_matches('"');

            // RDLENGTH
            let rdlength = txt_value.len() + 1; // length byte + text
            response.extend_from_slice(&(rdlength as u16).to_be_bytes());

            // RDATA
            response.push(txt_value.len() as u8);
            response.extend_from_slice(txt_value.as_bytes());
        },

        // CNAME or PTR record
        5 | 12 => {
            // Encode target name
            let target_wire = encode_dns_name(value);

            // RDLENGTH
            response.extend_from_slice(&(target_wire.len() as u16).to_be_bytes());

            // RDATA
            response.extend_from_slice(&target_wire);
        },

        _ => return Err(DnsError::Protocol(format!("Unsupported record type: {}", query_type))),
    }

    // Add EDNS record if present in query
    if has_edns {
        let opt_payload_size = extract_edns_payload_size(query).unwrap_or(4096);
        let do_bit = extract_do_bit(query);

        // Add OPT record
        response.extend_from_slice(&[0x00]); // Root domain
        response.extend_from_slice(&[0x00, 0x29]); // TYPE OPT
        response.extend_from_slice(&opt_payload_size.to_be_bytes()); // UDP payload size
        response.extend_from_slice(&[0x00]); // Extended RCODE
        response.extend_from_slice(&[0x00]); // EDNS version

        if do_bit {
            response.extend_from_slice(&[0x80, 0x00]); // Flags with DO bit set
        } else {
            response.extend_from_slice(&[0x00, 0x00]); // Flags with DO bit clear
        }

        response.extend_from_slice(&[0x00, 0x00]); // RDATA length
    }

    Ok(response)
}

/// Build a DNS response for a "not implemented" error.
///
/// # Arguments
/// * `query` - The DNS query.
/// * `authoritative` - Whether this server is authoritative for the domain.
///
/// # Returns
/// An `Option` containing the response if successful.
pub fn build_not_implemented_response(query: &[u8], authoritative: bool) -> Option<Vec<u8>> {
    if query.len() < 12 {
        return None;
    }

    let mut resp = Vec::with_capacity(512);

    // Copy transaction ID from query
    resp.extend_from_slice(&query[0..2]);

    // Set flags
    // QR = 1 (response)
    // OPCODE = copy from query
    // AA = 0 or 1 depending on authoritative
    // TC = 0 (not truncated)
    // RD = copy from query
    // RA = 1 (recursion available)
    // Z = 0
    // RCODE = 4 (not implemented)
    let opcode = query[2] & 0x78; // Extract OPCODE
    let rd = query[2] & 0x01; // Extract RD
    let flags1 = 0x80 | opcode | rd; // QR=1, OPCODE=opcode, RD=rd
    let flags2 = 0x84; // RA=1, RCODE=4 (not implemented)

    resp.extend_from_slice(&[
        if authoritative { flags1 | 0x04 } else { flags1 }, // Set AA if authoritative
        flags2,
    ]);

    // Copy QDCOUNT from query
    resp.extend_from_slice(&query[4..6]);

    // Set ANCOUNT, NSCOUNT, ARCOUNT to 0
    resp.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);

    // Copy question section from query
    let mut pos = 12;
    loop {
        if pos >= query.len() {
            return None;
        }
        let len = query[pos] as usize;
        if len == 0 {
            // End of domain name
            pos += 1;
            break;
        }
        pos += len + 1;
    }

    // Include QTYPE and QCLASS (4 bytes)
    if pos + 4 > query.len() {
        return None;
    }
    pos += 4;

    // Copy question section
    resp.extend_from_slice(&query[12..pos]);

    // Check for EDNS
    let has_edns = has_opt_record(query);
    if has_edns {
        let opt_payload_size = extract_edns_payload_size(query).unwrap_or(4096);
        let do_bit = extract_do_bit(query);

        // Add OPT record
        resp.extend_from_slice(&[0x00]); // Root domain
        resp.extend_from_slice(&[0x00, 0x29]); // TYPE OPT
        resp.extend_from_slice(&opt_payload_size.to_be_bytes()); // UDP payload size
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

/// Build a DNS response for a "name error" (NXDOMAIN).
///
/// # Arguments
/// * `query` - The DNS query.
/// * `authoritative` - Whether this server is authoritative for the domain.
///
/// # Returns
/// An `Option` containing the response if successful.
pub fn build_nxdomain_response(query: &[u8], authoritative: bool) -> Option<Vec<u8>> {
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

    // Get authoritative zones
    let zones = get_authoritative_zones(&config.db_path);
    let zone = find_closest_parent_zone(&domain, &zones);

    // Set flags
    // QR = 1 (response)
    // OPCODE = 0 (standard query)
    // AA = 1 if authoritative
    // TC = 0 (not truncated)
    // RD = copy from query
    // RA = 1 (recursion available)
    // Z = 0
    // RCODE = 3 (name error)
    let rd = query[2] & 0x01; // Extract RD
    let flags1 = 0x80 | rd; // QR=1, RD=rd
    let flags2 = 0x83; // RA=1, RCODE=3 (name error)

    resp.extend_from_slice(&[
        if authoritative { flags1 | 0x04 } else { flags1 }, // Set AA if authoritative
        flags2,
    ]);

    // Copy QDCOUNT from query
    resp.extend_from_slice(&query[4..6]);

    // Set ANCOUNT to 0
    resp.extend_from_slice(&[0x00, 0x00]);

    // Set NSCOUNT to 1 if we have a zone, 0 otherwise
    resp.extend_from_slice(&[0x00, if zone.is_some() { 0x01 } else { 0x00 }]);

    // Check for EDNS
    let has_edns = has_opt_record(query);

    // Set ARCOUNT to 1 if EDNS, 0 otherwise
    resp.extend_from_slice(&[0x00, if has_edns { 0x01 } else { 0x00 }]);

    // Copy question section from query
    let qname_end = match query[12..].iter().position(|&b| b == 0) {
        Some(pos) => pos + 13,
        None => return None,
    };
    resp.extend_from_slice(&query[12..qname_end + 4]);

    // Add authority section if we have a zone
    if let Some(zone) = zone {
        // Add SOA record
        let zone_name = encode_dns_name(&zone.name);
        resp.extend_from_slice(&zone_name);

        // Type SOA (0x0006)
        resp.extend_from_slice(&[0x00, 0x06]);

        // Class IN (0x0001)
        resp.extend_from_slice(&[0x00, 0x01]);

        // TTL
        resp.extend_from_slice(&(DEFAULT_TTL as u32).to_be_bytes());

        // RDATA
        if let Some(soa) = zone.soa_record {
            // Parse SOA record
            let parts: Vec<&str> = soa.split_whitespace().collect();
            if parts.len() >= 7 {
                let mname = parts[0];
                let rname = parts[1];

                // Encode MNAME and RNAME
                let mname_wire = encode_dns_name(mname);
                let rname_wire = encode_dns_name(rname);

                // RDLENGTH
                let rdlength = mname_wire.len() + rname_wire.len() + 20; // 5 32-bit integers
                resp.extend_from_slice(&(rdlength as u16).to_be_bytes());

                // RDATA
                resp.extend_from_slice(&mname_wire);
                resp.extend_from_slice(&rname_wire);

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