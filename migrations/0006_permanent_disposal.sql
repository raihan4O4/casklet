ALTER TABLE removal_plans ADD COLUMN permanent INTEGER NOT NULL DEFAULT 0 CHECK (permanent IN (0,1));
CREATE TABLE removal_plan_items_v6 (
    id TEXT PRIMARY KEY,
    plan_id TEXT NOT NULL REFERENCES removal_plans(id),
    sequence INTEGER NOT NULL,
    resource_id TEXT NOT NULL REFERENCES resources(id),
    name TEXT NOT NULL,
    action TEXT NOT NULL CHECK (action IN ('TRASH','DELETE_PERMANENTLY','DETACH','PRESERVE')),
    reason_code TEXT NOT NULL,
    expected_size_bytes INTEGER,
    expected_path TEXT NOT NULL,
    expected_kind TEXT NOT NULL,
    expected_identity TEXT,
    resource_revision INTEGER NOT NULL,
    association_revision INTEGER NOT NULL,
    state TEXT NOT NULL DEFAULT 'PENDING' CHECK (state IN ('PENDING','SUCCEEDED','PRESERVED','FAILED','UNKNOWN')),
    run_id TEXT REFERENCES removal_runs(id),
    started_at_ms INTEGER,
    finished_at_ms INTEGER,
    result_code TEXT,
    other_associations INTEGER NOT NULL DEFAULT 0,
    UNIQUE(plan_id,sequence),
    UNIQUE(plan_id,resource_id)
) STRICT;
INSERT INTO removal_plan_items_v6 SELECT id,plan_id,sequence,resource_id,name,action,reason_code,
    expected_size_bytes,expected_path,expected_kind,expected_identity,resource_revision,
    association_revision,state,run_id,started_at_ms,finished_at_ms,result_code,other_associations
    FROM removal_plan_items;
DROP TABLE removal_plan_items;
ALTER TABLE removal_plan_items_v6 RENAME TO removal_plan_items;
UPDATE removal_plans SET state='INVALIDATED' WHERE state IN ('DRAFT','APPROVED');
