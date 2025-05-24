//! NX9 DNS Server
//!
//! A DNS server implementation that supports various record types and can forward
//! queries to upstream DNS servers.
//!
//! Author: Sunil Purushottam Thakare
#![allow(dead_code)]
#[allow(unused_variables)]

use log::info;
use tokio::{signal, task};

use nx9_dns_server::{
    cache::{CACHE, CACHE_CLEANUP_INTERVAL},
    config::ServerConfig,
    db::init_db,
    errors::DnsError,
    handlers::{run_tcp_server, run_udp_server},
};

#[tokio::main]
async fn main() -> Result<(), DnsError> {
    // Initialize the logger
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_micros()
        .init();

    // Load configuration from environment variables
    let config = ServerConfig::from_env()?;
    
    // Initialize cache with NS records from config
    let cache = CACHE.get_or_init(|| nx9_dns_server::cache::DnsCache::new(config.ns_records.clone()));

    // Initialize the database
    init_db(&config.db_path, &config.default_domain, &config.default_ip)?;

    // Set up cache cleanup task
    let cache_cleanup = task::spawn({
        let cache = cache.clone();
        async move {
            let mut interval = tokio::time::interval(CACHE_CLEANUP_INTERVAL);
            loop {
                interval.tick().await;
                cache.cleanup();
            }
        }
    });

    // Set up shutdown signal handler
    let shutdown_signal = async {
        signal::ctrl_c().await.expect("Failed to listen for shutdown signal");
        info!("Shutdown signal received");
    };

    // Start UDP and TCP servers
    let udp_server = run_udp_server(config.clone());
    let tcp_server = run_tcp_server(config.clone());

    // Wait for either a shutdown signal or server error
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