CREATE TABLE removal_runs (
    id TEXT PRIMARY KEY,
    plan_id TEXT NOT NULL REFERENCES removal_plans(id),
    cask_id TEXT NOT NULL REFERENCES casks(id),
    state TEXT NOT NULL CHECK (state IN ('RUNNING','PARTIAL','SUCCEEDED','RECOVERY_REQUIRED','CLOSED_PRESERVED')),
    owner_pid TEXT NOT NULL,
    started_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    error_code TEXT
) STRICT;
CREATE UNIQUE INDEX removal_one_open_run ON removal_runs(plan_id) WHERE state IN ('RUNNING','PARTIAL','RECOVERY_REQUIRED');
CREATE TABLE removal_locks (
    cask_id TEXT PRIMARY KEY REFERENCES casks(id),
    run_id TEXT NOT NULL REFERENCES removal_runs(id),
    acquired_at_ms INTEGER NOT NULL
) STRICT;
ALTER TABLE removal_plan_items ADD COLUMN run_id TEXT REFERENCES removal_runs(id);
ALTER TABLE removal_plan_items ADD COLUMN started_at_ms INTEGER;
ALTER TABLE removal_plan_items ADD COLUMN finished_at_ms INTEGER;
ALTER TABLE removal_plan_items ADD COLUMN result_code TEXT;
ALTER TABLE removal_plan_items ADD COLUMN other_associations INTEGER NOT NULL DEFAULT 0;
UPDATE removal_plans SET state='INVALIDATED' WHERE state IN ('DRAFT','APPROVED');
