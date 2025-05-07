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
    ('33.61.254.60.in-addr.arpa', 'PTR', 'ns1.bzo.in', 3600),
    ('admin.bzo.in', 'A', '60.254.61.33', 3600),
    ('api.bzo.in', 'A', '60.254.61.33', 3600),
    ('bzo.in', 'A', '60.254.61.33', 3600),
    ('bzo.in', 'MX', '10 mail.bzo.in', 3600),
    ('bzo.in', 'NS', 'ns1.bzo.in', 3600),
    ('bzo.in', 'NS', 'ns2.bzo.in', 3600),  -- This will now work with the new schema
    ('bzo.in', 'SOA', 'ns1.bzo.in hostmaster.bzo.in 1 10800 3600 604800 86400', 3600),
    ('bzo.in', 'TXT', '"v=spf1 a mx ~all"', 3600),
    ('ddns.bzo.in', 'A', '60.254.61.33', 3600),
    ('ns1.bzo.in', 'A', '60.254.61.33', 3600),
    ('ns2.bzo.in', 'A', '60.254.61.33', 3600),
    ('www.bzo.in', 'A', '60.254.61.33', 3600);

COMMIT;
