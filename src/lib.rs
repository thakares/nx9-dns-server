//! NX9 DNS Server Library
//! 
//! This library provides functionality for a DNS server implementation.
//! It handles DNS queries over UDP and TCP, supports various record types,
//! and can forward queries to upstream DNS servers.

#![allow(dead_code)]
#[allow(unused_variables)]

// Define modules
pub mod errors;
pub mod config;
pub mod cache;
pub mod db;
pub mod dns;
pub mod handlers;
pub mod utils;
mod error;

// Re-export commonly used items
pub use errors::DnsError;
pub use config::ServerConfig;
pub use cache::DnsCache;