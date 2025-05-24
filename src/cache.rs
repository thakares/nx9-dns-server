//! DNS cache implementation.
//!
//! This module provides a simple in-memory cache for DNS records to improve
//! performance by avoiding repeated database lookups for frequently accessed domains.
#![allow(dead_code)]
#[allow(unused_variables)]

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, SystemTime},
};
use log::debug;

/// Interval for cleaning up expired cache entries (in seconds).
pub const CACHE_CLEANUP_INTERVAL: Duration = Duration::from_secs(300);

/// Global cache instance.
pub static CACHE: OnceLock<DnsCache> = OnceLock::new();

/// An entry in the DNS cache.
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// The IP address for the domain.
    pub ip: String,
    
    /// When this entry was added to the cache.
    pub inserted: SystemTime,
    
    /// Time-to-live in seconds.
    pub ttl: u64,
}

/// Cache for DNS records to improve performance.
#[derive(Debug, Clone)]
pub struct DnsCache {
    /// Map of domain names to cache entries.
    pub entries: Arc<Mutex<HashMap<String, CacheEntry>>>,
    
    /// List of NS records for zones this server is authoritative for.
    pub ns_records: Vec<String>,
}

impl DnsCache {
    /// Create a new DNS cache.
    ///
    /// # Arguments
    /// * `ns_records` - List of NS records for zones this server is authoritative for.
    ///
    /// # Returns
    /// A new `DnsCache` instance.
    pub fn new(ns_records: Vec<String>) -> Self {
        Self {
            entries: Arc::new(Mutex::new(HashMap::new())),
            ns_records,
        }
    }

    /// Get a cached IP address for a domain.
    ///
    /// # Arguments
    /// * `domain` - The domain name to look up.
    ///
    /// # Returns
    /// An `Option` containing the IP address and TTL if found and not expired.
    pub fn get(&self, domain: &str) -> Option<(String, u64)> {
        let cache = self.entries.lock().unwrap();
        if let Some(entry) = cache.get(domain) {
            if entry.inserted.elapsed().map(|d| d.as_secs() <= entry.ttl).unwrap_or(true) {
                return Some((entry.ip.clone(), entry.ttl));
            }
        }
        None
    }

    /// Add or update a domain in the cache.
    ///
    /// # Arguments
    /// * `domain` - The domain name to cache.
    /// * `ip` - The IP address for the domain.
    /// * `ttl` - Time-to-live in seconds.
    pub fn set(&self, domain: String, ip: String, ttl: u64) {
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

    /// Remove expired entries from the cache.
    pub fn cleanup(&self) {
        let mut cache = self.entries.lock().unwrap();
        cache.retain(|_, entry| {
            entry.inserted.elapsed().map(|d| d.as_secs() <= entry.ttl).unwrap_or(true)
        });
        debug!("Cache cleanup completed");
    }
}