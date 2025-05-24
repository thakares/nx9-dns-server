## DNS Server Algorithm

**1. Server Initialization**

```rust
1.1 Load configuration from environment variables
1.2 Initialize logging system
1.3 Create SQLite database connection
1.4 Initialize cache with NS records
1.5 Start periodic cache cleanup task
1.6 Bind UDP and TCP sockets on specified port
```

**2. Query Handling Flow**

```
                    Start
                      │
                      ▼
               Receive DNS Query
                      │
                      ▼
            Parse Query Header/Question
                      │
                      ▼
          Check Cache for Domain Record
              ┌───────┴───────┐
              ▼               ▼
         Cache Hit      Cache Miss
              │               │
              ▼               ▼
       Build Response    Query Database
                              │
                              ▼
                   Check Authoritative Flag
                      ┌────────┴────────┐
                      ▼                 ▼
                Record Found      Record Not Found
                      │                 │
                      ▼                 ▼
               Build Response    Forward to Resolvers
                      │                 │
                      ▼                 ▼
                  Add DNSSEC           ▼
                 Signatures       Receive Forwarded Response
                      │                 │
                      ▼                 ▼
               Send Response to Client
                      │
                      ▼
                     End
```

**3. DNSSEC Signing Process**

```rust
3.1 Load DNSSEC key from configured file
3.2 For each relevant DNS record:
    3.2.1 Generate RRSIG record
    3.2.2 Encode signature using Base64
    3.2.3 Calculate key tag and signature expiration
3.3 Add RRSIG records to DNS response
3.4 Include DNSKEY records in authority section
```

**4. Response Generation Logic**

```
4.1 Create response header with:
    - Original query ID
    - QR flag set to response
    - Authoritative Answer flag
    - Appropriate response code (NOERROR/NXDOMAIN)
    
4.2 Add original question section

4.3 Populate answer section with:
    - Resource records from cache/database
    - TTL values from configuration
    
4.4 Add authority section with:
    - NS records
    - DS records for DNSSEC
    
4.5 Include additional section with:
    - A records for NS names
    - DNSKEY records when applicable
```


## Key Data Flow Components

| Component | Purpose | Implementation Details |
| :-- | :-- | :-- |
| `DnsCache` | Response caching | Mutex-protected HashMap with TTL |
| `ServerConfig` | Runtime configuration | Environment variables parsing |
| `rusqlite` | Persistent storage | SQLite database with DNS records |
| `tokio` | Async I/O handling | UDP/TCP listeners with task spawning |
| `DNSSEC` | Response signing | RSA-SHA256 with preloaded keys |

## Error Handling Strategy

```rust
- Use custom DnsError enum with thiserror crate
- Graceful shutdown on SIGINT
- Automatic cache cleanup every 5 minutes
- Fallback to forwarding when local resolution fails
- Comprehensive logging at all stages
```

The server implements RFC 1035 (DNS) and RFC 4034 (DNSSEC) specifications with a focus on:

1. Async I/O using Tokio runtime
2. Thread-safe caching with atomic reference counting
3. Configurable forwarding and fallback mechanisms
4. DNSSEC signing capability for authoritative responses
5. SQLite-based record storage with schema versioning

<div style="text-align: center">⁂</div>
