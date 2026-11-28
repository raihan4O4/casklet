CREATE TABLE installer_artifacts (
    id TEXT PRIMARY KEY,
    cask_id TEXT NOT NULL REFERENCES casks(id),
    resource_id TEXT NOT NULL UNIQUE REFERENCES resources(id),
    source_path TEXT NOT NULL,
    path TEXT NOT NULL,
    format TEXT NOT NULL CHECK(format IN ('MSI','EXE')),
    sha256 TEXT NOT NULL,
    size_bytes INTEGER NOT NULL CHECK(size_bytes >= 0),
    inspection_json TEXT NOT NULL,
    registered_at_ms INTEGER NOT NULL,
    revision INTEGER NOT NULL DEFAULT 0
) STRICT;
CREATE INDEX installer_artifacts_cask ON installer_artifacts(cask_id,registered_at_ms);

CREATE TABLE installer_operations (
    id TEXT PRIMARY KEY,
    artifact_id TEXT NOT NULL REFERENCES installer_artifacts(id),
    cask_id TEXT NOT NULL REFERENCES casks(id),
    state TEXT NOT NULL CHECK(state IN ('RUNNING','REVIEW_REQUIRED','RECOVERY_REQUIRED','CONFIRMED_ASSOCIATION','CLOSED_PRESERVED')),
    exit_code INTEGER,
    error_code TEXT,
    candidates_json TEXT NOT NULL DEFAULT '[]',
    confirmed_installation_id TEXT,
    cancellation_requested INTEGER NOT NULL DEFAULT 0 CHECK(cancellation_requested IN (0,1)),
    started_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    finished_at_ms INTEGER,
    revision INTEGER NOT NULL DEFAULT 0
) STRICT;
CREATE INDEX installer_operations_cask ON installer_operations(cask_id,started_at_ms DESC);

CREATE TABLE installer_locks (
    cask_id TEXT PRIMARY KEY REFERENCES casks(id),
    operation_id TEXT NOT NULL UNIQUE REFERENCES installer_operations(id),
    acquired_at_ms INTEGER NOT NULL
) STRICT;
