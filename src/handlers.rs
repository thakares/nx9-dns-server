//! Request handlers for the DNS server.
//!
//! This module provides functions for handling DNS requests over UDP and TCP.
#![allow(dead_code)]
#[allow(unused_variables)]

use std::net::SocketAddr;
use std::sync::Arc;
use log::{debug, error, info, warn};
use tokio::{
    io::AsyncReadExt,
    net::{TcpListener, TcpStream, UdpSocket},
    task,
};

use crate::errors::DnsError;
use crate::config::ServerConfig;
use crate::utils::extract_domain;
use crate::dns::{
    build_not_implemented_response, build_nxdomain_response, generate_dns_response,
    send_tcp_response,
};

/// Run the UDP DNS server.
///
/// # Arguments
/// * `config` - The server configuration.
///
/// # Returns
/// A `Result` indicating success or failure.
pub async fn run_udp_server(config: ServerConfig) -> Result<(), DnsError> {
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

/// Handle a UDP DNS query.
///
/// # Arguments
/// * `query` - The DNS query.
/// * `src` - The source address of the query.
/// * `socket` - The UDP socket to send the response on.
/// * `config` - The server configuration.
///
/// # Returns
/// A `Result` indicating success or failure.
pub async fn handle_udp_query(
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

/// Run the TCP DNS server.
///
/// # Arguments
/// * `config` - The server configuration.
///
/// # Returns
/// A `Result` indicating success or failure.
pub async fn run_tcp_server(config: ServerConfig) -> Result<(), DnsError> {
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

/// Handle a TCP DNS connection.
///
/// # Arguments
/// * `stream` - The TCP stream.
/// * `addr` - The client address.
/// * `config` - The server configuration.
///
/// # Returns
/// A `Result` indicating success or failure.
pub async fn handle_tcp_connection(
    mut stream: TcpStream,
    addr: SocketAddr,
    config: ServerConfig,
) -> Result<(), DnsError> {
    // Read the 2-byte length prefix
    let mut len_buf = [0u8; 2];
    stream.read_exact(&mut len_buf).await?;
    let len = u16::from_be_bytes(len_buf) as usize;

    // Read the DNS query
    let mut query = vec![0u8; len];
    stream.read_exact(&mut query).await?;

    if query.len() < 12 {
        debug!("Received malformed TCP query from {}", addr);
        return Ok(());
    }

    let opcode = (query[2] & 0x78) >> 3;
    if opcode != 0 {
        if let Some(response) = build_not_implemented_response(&query, config.authoritative) {
            send_tcp_response(&mut stream, &response).await?;
        }
        return Ok(());
    }

    let domain = match extract_domain(&query) {
        Some(d) => d,
        None => {
            info!("Failed to extract domain from TCP query");
            return Ok(());
        }
    };

    debug!("TCP query for {} from {}", domain, addr);
    info!("Processing TCP query for domain: {}", domain);

    let response = match generate_dns_response(&query, domain.clone(), &config).await {
        Ok(resp) => resp,
        Err(_) => {
            build_nxdomain_response(&query, config.authoritative)
                .ok_or(DnsError::Protocol("NXDOMAIN".into()))?
        }
    };

    // Send the response (local/cache answer)
    send_tcp_response(&mut stream, &response).await?;
    Ok(())
}