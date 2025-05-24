//! Error types for the DNS server
#![allow(dead_code)]
#[allow(unused_variables)]

use std::io;
use rusqlite;
use thiserror::Error;

/// Errors that can occur in the DNS server
#[derive(Debug, Error)]
pub enum DnsError {
    /// I/O errors from the underlying system
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    
    /// Database errors from SQLite operations
    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),
    
    /// Protocol errors related to DNS message format or content
    #[error("Protocol error: {0}")]
    Protocol(String),
    
    /// Configuration errors from invalid settings
    #[error("Configuration error: {0}")]
    Config(String),
    
    /// Parse errors from string to number conversions
    #[error("Parse error: {0}")]
    Parse(#[from] std::num::ParseIntError),
    
    /// Base64 decoding errors
    #[error("Base64 error: {0}")]
    Base64(String),
    
    /// Shutdown signal received
    #[error("Shutdown signal received")]
    Shutdown,
}