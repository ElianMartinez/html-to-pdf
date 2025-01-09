CREATE TABLE IF NOT EXISTS operations (
    id TEXT PRIMARY KEY,
    operation_type TEXT NOT NULL,    -- "send_email", "generate_pdf", "send_notification", etc.
    status TEXT NOT NULL,            -- "pending", "running", "done", "failed"
    error_message TEXT,              -- si falló
    is_async INTEGER NOT NULL,       -- 0 = false, 1 = true
    created_at TEXT NOT NULL,        -- ISO timestamp
    updated_at TEXT NOT NULL,        -- ISO timestamp
    metadata TEXT                    -- JSON o cualquier extra info
);
