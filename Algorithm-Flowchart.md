
# DNS Server Algorithm & Flowchart

This document outlines the algorithm and flowchart for a DNS server implementation compliant with RFC 1035 (DNS) and RFC 4034 (DNSSEC).

---

## ✅ Server Algorithm

### 1. Server Initialization

1. Load configuration from environment variables.
2. Initialize the logging system.
3. Create SQLite database connection and initialize schema.
4. Initialize cache with NS records.
5. Start periodic cache cleanup task (every 5 minutes).
6. Bind and listen on UDP and TCP sockets.

### 2. Query Handling Flow

#### Upon Receiving a DNS Query:

1. Validate DNS query packet.
2. Parse header and extract domain name and query type.
3. If query type is `DNSKEY` or `DS`, return signed records.
4. Check DNS cache:
    - If **hit**, build and return response.
    - If **miss**, lookup in database:
        - If found, respond and cache it.
        - If not found:
            - If authoritative, return `NXDOMAIN`.
            - Else, forward to upstream resolvers.
5. Add DNSSEC signatures if applicable.
6. Send response to the client.

### 3. DNSSEC Signing Process

1. Load DNSSEC key from configured file.
2. For each relevant record:
    - Generate `RRSIG`.
    - Encode signature (Base64).
    - Calculate key tag and signature expiration.
3. Add `RRSIG` to the answer section.
4. Include `DNSKEY` in the authority section if needed.

### 4. Response Generation Logic

1. Construct response header:
    - Set QR flag and response code.
    - Include Authoritative Answer (AA) if authoritative.
2. Attach original question section.
3. Populate:
    - **Answer** section: with resolved records.
    - **Authority** section: with NS and DS records.
    - **Additional** section: with glue records, DNSKEY if required.

---

## 📊 Flowchart

Below is the visual representation of the DNS query handling logic:

```

+---------------------+
|   Start DNS Server  |
+---------------------+
           |
           v
+---------------------+
|  Receive DNS Query  |
+---------------------+
           |
           v
+---------------------+
| Parse Header and    |
| Extract Domain &    |
| Query Type          |
+---------------------+
           |
           +---------------------+
           |                     |
           v                     v
+---------------------+  +---------------------+
| Is Query Type       |  | Use Cache           |
| DNSKEY/DS?          |  |                     |
+---------------------+  +---------------------+
           |                     |
    Yes    |                     |
           v                     v
+---------------------+  +---------------------+
| Return              |  | Lookup in SQLite DB |
| DNSSEC Record       |  |                     |
+---------------------+  +---------------------+
                                   |
                                   v
                        +---------------------+
                        | Is Authoritative    |
                        | Zone?               |
                        +---------------------+
                                   |
                            No     |
                                   v
                        +---------------------+
                        | Return NXDOMAIN     |
                        +---------------------+
                                   |
                                   v
                        +---------------------+
                        | Add GSSEC           |
                        +---------------------+
                                   |
                                   v
                        +---------------------+
                        | Send Response       |
                        +---------------------+
                                   |
                                   v
                        +---------------------+
                        | End                |
                        +---------------------+

```


## 🧩 Key Components

| Component      | Purpose                    | Details                                  |
|----------------|----------------------------|------------------------------------------|
| `DnsCache`     | DNS Response Cache         | Thread-safe HashMap with TTL             |
| `ServerConfig` | Server Configuration       | Loaded via environment variables         |
| `rusqlite`     | Record Storage             | SQLite database backend                  |
| `tokio`        | Async I/O Runtime          | UDP/TCP async handlers and tasks         |
| `DNSSEC`       | Secure DNS Signing         | RSA-SHA256 with Base64-encoded keys      |

---

## ⚠️ Error Handling Strategy

- Custom `DnsError` enum via `thiserror`
- Graceful shutdown via `SIGINT`
- Cache cleanup every 5 minutes
- Fallback to resolver forwarding
- Detailed logging at every stage

---
## API Reference

#### Get all items

```http
  GET /api/items
```

| Parameter | Type     | Description                |
| :-------- | :------- | :------------------------- |
| `api_key` | `string` | **Required**. Your API key |

#### Get item

```http
  GET /api/items/${id}
```

| Parameter | Type     | Description                       |
| :-------- | :------- | :-------------------------------- |
| `id`      | `string` | **Required**. Id of item to fetch |

#### add(num1, num2)

Takes two numbers and returns the sum.

