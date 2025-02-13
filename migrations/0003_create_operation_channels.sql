-- migrations/0003_create_operation_channels.sql

CREATE TABLE IF NOT EXISTS operation_channels (
    id TEXT PRIMARY KEY,
    operation_id TEXT NOT NULL,
    channel TEXT NOT NULL,           -- "email", "whatsapp", "sms", ...
    status TEXT NOT NULL,            -- "pending", "running", "done", "failed"
    error_message TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (operation_id) REFERENCES operations (id) ON DELETE CASCADE
);
