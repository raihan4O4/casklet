CREATE TABLE applications (
    id TEXT PRIMARY KEY,
    canonical_name TEXT NOT NULL,
    publisher TEXT,
    created_at_ms INTEGER NOT NULL,
    revision INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE TABLE installations (
    id TEXT PRIMARY KEY,
    application_id TEXT NOT NULL REFERENCES applications(id),
    provider TEXT NOT NULL CHECK (provider IN ('PORTABLE_ZIP')),
    version TEXT,
    scope TEXT NOT NULL CHECK (scope IN ('CASK')),
    provenance TEXT NOT NULL CHECK (provenance IN ('INSTALLED_BY_CASKLET')),
    state TEXT NOT NULL CHECK (state IN ('INSTALLED','MISSING','CHANGED','REMOVED','RECOVERY_REQUIRED')),
    source_sha256 TEXT NOT NULL,
    executable_sha256 TEXT NOT NULL,
    executable_relative_path TEXT NOT NULL,
    resource_id TEXT NOT NULL UNIQUE REFERENCES resources(id),
    installed_at_ms INTEGER NOT NULL,
    last_verified_at_ms INTEGER,
    revision INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE TABLE installation_locations (
    id TEXT PRIMARY KEY,
    installation_id TEXT NOT NULL REFERENCES installations(id),
    path_display TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('ROOT','EXECUTABLE')),
    last_verified_at_ms INTEGER,
    UNIQUE(installation_id,role)
) STRICT;

CREATE TABLE cask_installations (
    id TEXT PRIMARY KEY,
    cask_id TEXT NOT NULL REFERENCES casks(id),
    installation_id TEXT NOT NULL REFERENCES installations(id),
    relationship TEXT NOT NULL CHECK (relationship IN ('INSTALLED_FOR_CASK','ASSOCIATED')),
    removal_policy TEXT NOT NULL CHECK (removal_policy IN ('PLAN_REQUIRED','DETACH_ONLY')),
    created_at_ms INTEGER NOT NULL,
    removed_at_ms INTEGER,
    revision INTEGER NOT NULL DEFAULT 0
) STRICT;
CREATE UNIQUE INDEX cask_installation_active ON cask_installations(cask_id,installation_id) WHERE removed_at_ms IS NULL;

CREATE TABLE portable_install_intents (
    operation_id TEXT PRIMARY KEY REFERENCES operations(id),
    cask_id TEXT NOT NULL REFERENCES casks(id),
    installation_id TEXT NOT NULL UNIQUE,
    resource_id TEXT NOT NULL UNIQUE,
    archive_path TEXT NOT NULL,
    archive_sha256 TEXT NOT NULL,
    executable_relative_path TEXT NOT NULL,
    staging_path TEXT NOT NULL,
    final_path TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('PLANNED','EXTRACTED','REGISTERED','COMPLETE','RECOVERY_REQUIRED'))
) STRICT;

CREATE TABLE portable_launches (
    id TEXT PRIMARY KEY,
    installation_id TEXT NOT NULL REFERENCES installations(id),
    state TEXT NOT NULL CHECK (state IN ('STARTED','FAILED')),
    at_ms INTEGER NOT NULL
) STRICT;
