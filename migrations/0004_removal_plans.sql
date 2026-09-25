ALTER TABLE resource_locations ADD COLUMN identity_key TEXT;

CREATE TABLE removal_plans (
    id TEXT PRIMARY KEY,
    cask_id TEXT NOT NULL REFERENCES casks(id),
    scope TEXT NOT NULL CHECK (scope IN ('CASK','CATEGORY','RESOURCES')),
    category_key TEXT,
    state TEXT NOT NULL CHECK (state IN ('DRAFT','APPROVED','EXECUTING','PARTIAL','COMPLETE','INVALIDATED')),
    based_on_cask_revision INTEGER NOT NULL,
    plan_hash TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    expires_at_ms INTEGER NOT NULL,
    approved_at_ms INTEGER,
    executing_operation_id TEXT REFERENCES operations(id),
    revision INTEGER NOT NULL DEFAULT 0
) STRICT;
CREATE INDEX removal_plans_by_cask ON removal_plans(cask_id,created_at_ms);

CREATE TABLE removal_plan_items (
    id TEXT PRIMARY KEY,
    plan_id TEXT NOT NULL REFERENCES removal_plans(id),
    sequence INTEGER NOT NULL,
    resource_id TEXT NOT NULL REFERENCES resources(id),
    name TEXT NOT NULL,
    action TEXT NOT NULL CHECK (action IN ('TRASH','DETACH','PRESERVE')),
    reason_code TEXT NOT NULL,
    expected_size_bytes INTEGER,
    expected_path TEXT NOT NULL,
    expected_kind TEXT NOT NULL,
    expected_identity TEXT,
    resource_revision INTEGER NOT NULL,
    association_revision INTEGER NOT NULL,
    state TEXT NOT NULL DEFAULT 'PENDING' CHECK (state IN ('PENDING','SUCCEEDED','PRESERVED','FAILED','UNKNOWN')),
    UNIQUE(plan_id,sequence),
    UNIQUE(plan_id,resource_id)
) STRICT;
