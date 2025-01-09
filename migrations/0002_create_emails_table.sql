-- migrations/0002_create_emails_table.sql
CREATE TABLE IF NOT EXISTS emails (
    id TEXT PRIMARY KEY,
    operation_id TEXT NOT NULL,   -- relación con la tabla operations
    recipient TEXT NOT NULL,
    subject TEXT NOT NULL,
    body TEXT NOT NULL,
    status TEXT NOT NULL,         -- "pending", "sent", "failed"
    error_message TEXT,
    created_at TEXT NOT NULL
);
