//! Error types for the DNS server.
//!
//! This module defines the error types used throughout the DNS server implementation.
#![allow(dead_code)]
#[allow(unused_variables)]

use thiserror::Error;

/// Represents errors that can occur in the DNS server.
#[derive(Error, Debug)]
pub enum DnsError {
    /// I/O errors from the standard library.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    
    /// Database errors from rusqlite.
    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),
    
    /// Errors related to DNS protocol parsing or formatting.
    #[error("Invalid DNS packet: {0}")]
    Protocol(String),
    
    /// Configuration errors.
    #[error("Configuration error: {0}")]
    Config(String),
    
    /// Integer parsing errors.
    #[error("Parse error: {0}")]
    Parse(#[from] std::num::ParseIntError),
    
    /// Base64 decoding errors.
    #[error("Base64 error: {0}")]
    Base64(String),
    
    /// Shutdown signal received.
    #[error("Shutdown signal received")]
    Shutdown,
}