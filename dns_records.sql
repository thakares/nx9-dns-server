-- dns.query - SQL commands to populate DNS records
BEGIN TRANSACTION;

-- First modify the table schema to allow multiple NS records
CREATE TABLE IF NOT EXISTS dns_records (
    domain TEXT NOT NULL,
    record_type TEXT NOT NULL,
    value TEXT NOT NULL,
    ttl INTEGER DEFAULT 3600,
    PRIMARY KEY (domain, record_type, value)
) WITHOUT ROWID;

-- Copy existing data to new table
INSERT INTO dns_records_new SELECT * FROM dns_records;

-- Replace the old table
DROP TABLE dns_records;
ALTER TABLE dns_records_new RENAME TO dns_records;

-- Now insert all DNS records
INSERT OR REPLACE INTO dns_records VALUES
    ('33.61.254.60.in-addr.arpa', 'PTR', 'ns1.yourdomain.tld', 3600),
    ('admin.yourdomain.tld', 'A', '60.254.61.33', 3600),
    ('api.yourdomain.tld', 'A', '60.254.61.33', 3600),
    ('yourdomain.tld', 'A', '60.254.61.33', 3600),
    ('yourdomain.tld', 'MX', '10 mail.yourdomain.tld', 3600),
    ('yourdomain.tld', 'NS', 'ns1.yourdomain.tld', 3600),
    ('yourdomain.tld', 'NS', 'ns2.yourdomain.tld', 3600),  -- This will now work with the new schema
    ('yourdomain.tld', 'SOA', 'ns1.yourdomain.tld hostmaster.yourdomain.tld 1 10800 3600 604800 86400', 3600),
    ('yourdomain.tld', 'TXT', '"v=spf1 a mx ~all"', 3600),
    ('ddns.yourdomain.tld', 'A', '60.254.61.33', 3600),
    ('ns1.yourdomain.tld', 'A', '60.254.61.33', 3600),
    ('ns2.yourdomain.tld', 'A', '60.254.61.33', 3600),
    ('www.yourdomain.tld', 'A', '60.254.61.33', 3600);

COMMIT;
