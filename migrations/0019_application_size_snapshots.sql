-- Presentation-only observations. Never cleanup authority or live verification.
CREATE TABLE application_size_snapshots (
    source_key TEXT PRIMARY KEY COLLATE NOCASE,
    evidence_sha256 TEXT NOT NULL,
    size_bytes INTEGER CHECK(size_bytes IS NULL OR size_bytes >= 0),
    estimated INTEGER NOT NULL CHECK(estimated IN (0,1)),
    measured_at_ms INTEGER NOT NULL
) STRICT;

CREATE INDEX windows_installations_source_key_nocase ON windows_installations(source_key COLLATE NOCASE);
