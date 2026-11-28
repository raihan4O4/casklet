CREATE TABLE windows_installations (
    id TEXT PRIMARY KEY,
    application_id TEXT NOT NULL REFERENCES applications(id),
    source_key TEXT NOT NULL UNIQUE,
    scope TEXT NOT NULL CHECK(scope IN ('USER','SYSTEM')),
    display_name TEXT NOT NULL,
    publisher TEXT,
    display_version TEXT,
    install_location TEXT,
    evidence_json TEXT NOT NULL,
    evidence_sha256 TEXT NOT NULL,
    state TEXT NOT NULL CHECK(state IN ('PRESENT','CHANGED','MISSING','AMBIGUOUS')),
    first_seen_at_ms INTEGER NOT NULL,
    last_seen_at_ms INTEGER NOT NULL,
    last_verified_at_ms INTEGER NOT NULL,
    revision INTEGER NOT NULL DEFAULT 0
) STRICT;
CREATE INDEX windows_installations_name ON windows_installations(display_name COLLATE NOCASE);

CREATE TABLE windows_cask_associations (
    id TEXT PRIMARY KEY,
    cask_id TEXT NOT NULL REFERENCES casks(id),
    installation_id TEXT NOT NULL REFERENCES windows_installations(id),
    relationship TEXT NOT NULL DEFAULT 'ASSOCIATED' CHECK(relationship='ASSOCIATED'),
    removal_policy TEXT NOT NULL DEFAULT 'DETACH_ONLY' CHECK(removal_policy='DETACH_ONLY'),
    created_at_ms INTEGER NOT NULL,
    removed_at_ms INTEGER,
    revision INTEGER NOT NULL DEFAULT 0
) STRICT;
CREATE UNIQUE INDEX windows_cask_active ON windows_cask_associations(cask_id,installation_id) WHERE removed_at_ms IS NULL;
