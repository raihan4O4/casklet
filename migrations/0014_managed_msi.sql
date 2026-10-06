CREATE TABLE installer_msi_baselines (
 operation_id TEXT PRIMARY KEY REFERENCES installer_operations(id),
 product_code TEXT NOT NULL,
 absent_before INTEGER NOT NULL CHECK(absent_before IN (0,1))
) STRICT;
CREATE TABLE managed_msi_installations (
 installation_id TEXT PRIMARY KEY REFERENCES windows_installations(id),
 cask_id TEXT NOT NULL REFERENCES casks(id),
 operation_id TEXT NOT NULL UNIQUE REFERENCES installer_operations(id),
 product_code TEXT NOT NULL,
 evidence_sha256 TEXT NOT NULL,
 display_name TEXT NOT NULL,
 state TEXT NOT NULL CHECK(state IN ('OWNED','REMOVED','RELEASED')),
 claimed_at_ms INTEGER NOT NULL
) STRICT;
CREATE TABLE msi_cleanup_plans (
 id TEXT PRIMARY KEY,
 cask_id TEXT NOT NULL REFERENCES casks(id),
 state TEXT NOT NULL CHECK(state IN ('DRAFT','APPROVED','EXECUTING','PARTIAL','COMPLETE','CLOSED_PRESERVED')),
 plan_hash TEXT NOT NULL,
 plan_json TEXT NOT NULL,
 updated_at_ms INTEGER NOT NULL
) STRICT;
CREATE INDEX msi_plans_cask ON msi_cleanup_plans(cask_id,updated_at_ms DESC);
CREATE TABLE msi_cleanup_locks (
 cask_id TEXT PRIMARY KEY REFERENCES casks(id),
 plan_id TEXT NOT NULL UNIQUE REFERENCES msi_cleanup_plans(id),
 owner_pid TEXT
) STRICT;
CREATE TABLE msi_cleanup_steps (
 plan_id TEXT NOT NULL REFERENCES msi_cleanup_plans(id),
 installation_id TEXT NOT NULL REFERENCES managed_msi_installations(installation_id),
 state TEXT NOT NULL,
 result_code TEXT,
 at_ms INTEGER NOT NULL,
 PRIMARY KEY(plan_id,installation_id)
) STRICT;
ALTER TABLE cask_removals ADD COLUMN storage_warning TEXT;
ALTER TABLE cask_removals ADD COLUMN storage_retired_at_ms INTEGER;
