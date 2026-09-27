CREATE TABLE winget_operations (
    id TEXT PRIMARY KEY,
    cask_id TEXT NOT NULL REFERENCES casks(id),
    action TEXT NOT NULL CHECK(action IN ('INSTALL','UNINSTALL')),
    package_id TEXT NOT NULL,
    package_version TEXT NOT NULL,
    state TEXT NOT NULL CHECK(state IN ('RUNNING','SUCCEEDED','RECOVERY_REQUIRED','CANCELLED','CLOSED_PRESERVED')),
    step TEXT NOT NULL,
    error_code TEXT,
    cancellation_requested INTEGER NOT NULL DEFAULT 0 CHECK(cancellation_requested IN (0,1)),
    started_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    finished_at_ms INTEGER,
    revision INTEGER NOT NULL DEFAULT 0
) STRICT;
CREATE INDEX winget_operations_cask ON winget_operations(cask_id,started_at_ms);

CREATE TABLE winget_locks (
    cask_id TEXT PRIMARY KEY REFERENCES casks(id),
    operation_id TEXT NOT NULL UNIQUE REFERENCES winget_operations(id),
    acquired_at_ms INTEGER NOT NULL
) STRICT;

CREATE TABLE winget_installations (
    id TEXT PRIMARY KEY,
    cask_id TEXT NOT NULL REFERENCES casks(id),
    package_id TEXT NOT NULL,
    package_version TEXT NOT NULL,
    registry_installation_id TEXT NOT NULL REFERENCES windows_installations(id),
    registry_evidence_sha256 TEXT NOT NULL,
    provider_sha256 TEXT NOT NULL,
    install_operation_id TEXT NOT NULL UNIQUE REFERENCES winget_operations(id),
    state TEXT NOT NULL CHECK(state IN ('INSTALLED','CHANGED','MISSING','REMOVED','RECOVERY_REQUIRED')),
    provenance TEXT NOT NULL DEFAULT 'INSTALLED_BY_CASKLET' CHECK(provenance='INSTALLED_BY_CASKLET'),
    installed_at_ms INTEGER NOT NULL,
    last_verified_at_ms INTEGER NOT NULL,
    revision INTEGER NOT NULL DEFAULT 0,
    UNIQUE(package_id,registry_installation_id)
) STRICT;
CREATE INDEX winget_installations_cask ON winget_installations(cask_id,installed_at_ms);

CREATE TABLE winget_uninstall_plans (
    id TEXT PRIMARY KEY,
    installation_id TEXT NOT NULL REFERENCES winget_installations(id),
    cask_id TEXT NOT NULL REFERENCES casks(id),
    package_id TEXT NOT NULL,
    package_version TEXT NOT NULL,
    registry_installation_id TEXT NOT NULL,
    registry_evidence_sha256 TEXT NOT NULL,
    provider_sha256 TEXT NOT NULL,
    installation_revision INTEGER NOT NULL,
    plan_hash TEXT NOT NULL,
    state TEXT NOT NULL CHECK(state IN ('DRAFT','APPROVED','EXECUTING','COMPLETE','INVALIDATED','RECOVERY_REQUIRED')),
    created_at_ms INTEGER NOT NULL,
    expires_at_ms INTEGER NOT NULL,
    approved_at_ms INTEGER,
    execution_operation_id TEXT REFERENCES winget_operations(id),
    revision INTEGER NOT NULL DEFAULT 0
) STRICT;
