CREATE TABLE categories (
    key TEXT PRIMARY KEY,
    cask_id TEXT REFERENCES casks(id),
    display_name TEXT NOT NULL,
    is_builtin INTEGER NOT NULL CHECK (is_builtin IN (0,1)),
    sort_order INTEGER NOT NULL,
    CHECK ((is_builtin = 1 AND cask_id IS NULL) OR (is_builtin = 0 AND cask_id IS NOT NULL))
) STRICT;
INSERT INTO categories (key,cask_id,display_name,is_builtin,sort_order) VALUES
 ('applications',NULL,'Applications',1,10), ('projects',NULL,'Projects',1,20),
 ('documents',NULL,'Documents',1,30), ('images',NULL,'Images',1,40),
 ('videos',NULL,'Videos',1,50), ('audio',NULL,'Audio',1,60),
 ('archives',NULL,'Archives',1,70), ('downloads',NULL,'Downloads',1,80),
 ('other',NULL,'Other',1,90);

CREATE TABLE resources (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('FILE','DIRECTORY')),
    display_name TEXT NOT NULL,
    management TEXT NOT NULL CHECK (management IN ('MANAGED','REFERENCED')),
    provenance TEXT NOT NULL,
    availability TEXT NOT NULL CHECK (availability IN ('PRESENT','MISSING','CHANGED','INACCESSIBLE','UNKNOWN')),
    size_bytes INTEGER CHECK (size_bytes >= 0),
    size_measurement TEXT NOT NULL CHECK (size_measurement IN ('EXACT','CALCULATED','ESTIMATED','UNKNOWN')),
    unknown_entries INTEGER NOT NULL CHECK (unknown_entries >= 0),
    created_at_ms INTEGER NOT NULL,
    revision INTEGER NOT NULL DEFAULT 0
) STRICT;
CREATE TABLE resource_locations (
    id TEXT PRIMARY KEY,
    resource_id TEXT NOT NULL REFERENCES resources(id),
    path_display TEXT NOT NULL,
    path_key TEXT NOT NULL,
    containment TEXT NOT NULL CHECK (containment IN ('INTERNAL','EXTERNAL')),
    is_primary INTEGER NOT NULL CHECK (is_primary IN (0,1)),
    first_seen_at_ms INTEGER NOT NULL,
    retired_at_ms INTEGER
) STRICT;
CREATE UNIQUE INDEX resource_one_primary_location ON resource_locations(resource_id) WHERE is_primary = 1 AND retired_at_ms IS NULL;

CREATE TABLE resource_associations (
    id TEXT PRIMARY KEY,
    cask_id TEXT NOT NULL REFERENCES casks(id),
    resource_id TEXT NOT NULL REFERENCES resources(id),
    category_key TEXT NOT NULL REFERENCES categories(key),
    relationship TEXT NOT NULL CHECK (relationship IN ('OWNED','REFERENCED')),
    removal_policy TEXT NOT NULL CHECK (removal_policy IN ('DELETE_WHEN_UNREFERENCED','DETACH_ONLY')),
    added_at_ms INTEGER NOT NULL,
    removed_at_ms INTEGER,
    revision INTEGER NOT NULL DEFAULT 0
) STRICT;
CREATE UNIQUE INDEX resource_active_association ON resource_associations(cask_id,resource_id) WHERE removed_at_ms IS NULL;
CREATE INDEX resource_by_cask ON resource_associations(cask_id,added_at_ms);

CREATE TABLE intake_operations (
    id TEXT PRIMARY KEY,
    cask_id TEXT NOT NULL REFERENCES casks(id),
    resource_id TEXT NOT NULL,
    mode TEXT NOT NULL CHECK (mode IN ('COPY','MOVE')),
    source_path TEXT NOT NULL,
    staging_path TEXT NOT NULL,
    final_path TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('PLANNED','REGISTERED','COMPLETE','RECOVERY_REQUIRED')),
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
) STRICT;
CREATE INDEX intake_pending ON intake_operations(state,cask_id);
