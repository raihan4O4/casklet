CREATE TABLE operations (
    id TEXT PRIMARY KEY,
    cask_id TEXT NOT NULL REFERENCES casks(id),
    type TEXT NOT NULL CHECK (type IN ('RESOURCE_INTAKE')),
    state TEXT NOT NULL CHECK (state IN ('RUNNING','SUCCEEDED','RECOVERY_REQUIRED','CANCELLED','CLOSED_PRESERVED')),
    progress_completed INTEGER NOT NULL DEFAULT 0 CHECK (progress_completed >= 0),
    progress_total INTEGER CHECK (progress_total >= 0),
    current_step_code TEXT NOT NULL,
    cancellation_requested INTEGER NOT NULL DEFAULT 0 CHECK (cancellation_requested IN (0,1)),
    error_code TEXT,
    started_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    finished_at_ms INTEGER,
    revision INTEGER NOT NULL DEFAULT 0
) STRICT;
CREATE INDEX operations_by_cask ON operations(cask_id,started_at_ms);

CREATE TABLE operation_steps (
    operation_id TEXT NOT NULL REFERENCES operations(id),
    sequence INTEGER NOT NULL,
    step_code TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('PENDING','RUNNING','SUCCEEDED','RECOVERY_REQUIRED','CANCELLED')),
    attempt_count INTEGER NOT NULL DEFAULT 0,
    started_at_ms INTEGER,
    finished_at_ms INTEGER,
    PRIMARY KEY (operation_id,sequence)
) STRICT;

CREATE TABLE operation_locks (
    lock_key TEXT PRIMARY KEY,
    operation_id TEXT NOT NULL REFERENCES operations(id),
    owner_instance_id TEXT NOT NULL,
    acquired_at_ms INTEGER NOT NULL,
    heartbeat_at_ms INTEGER NOT NULL
) STRICT;

CREATE TABLE projection_jobs (
    cask_id TEXT PRIMARY KEY REFERENCES casks(id),
    target_revision INTEGER NOT NULL DEFAULT 0,
    state TEXT NOT NULL CHECK (state IN ('PENDING','COMPLETE')),
    attempt_count INTEGER NOT NULL DEFAULT 0,
    last_error_code TEXT,
    updated_at_ms INTEGER NOT NULL
) STRICT;
CREATE TABLE diagnostic_events (
    id INTEGER PRIMARY KEY,
    operation_id TEXT REFERENCES operations(id),
    event_code TEXT NOT NULL,
    at_ms INTEGER NOT NULL
) STRICT;
CREATE INDEX diagnostic_events_by_operation ON diagnostic_events(operation_id,at_ms);
INSERT INTO projection_jobs(cask_id,target_revision,state,updated_at_ms)
    SELECT id,revision,'PENDING',updated_at_ms FROM casks;

ALTER TABLE intake_operations ADD COLUMN resource_name TEXT;
ALTER TABLE intake_operations ADD COLUMN resource_kind TEXT;
ALTER TABLE intake_operations ADD COLUMN category_key TEXT;
ALTER TABLE intake_operations ADD COLUMN size_bytes INTEGER;
ALTER TABLE intake_operations ADD COLUMN unknown_entries INTEGER;

INSERT INTO operations(id,cask_id,type,state,current_step_code,started_at_ms,updated_at_ms,finished_at_ms)
    SELECT id,cask_id,'RESOURCE_INTAKE',
      CASE WHEN state='COMPLETE' THEN 'SUCCEEDED' ELSE 'RECOVERY_REQUIRED' END,
      CASE WHEN state='COMPLETE' THEN 'COMPLETE' ELSE 'RECONCILE' END,
      created_at_ms,updated_at_ms,
      CASE WHEN state='COMPLETE' THEN updated_at_ms ELSE NULL END
    FROM intake_operations;
INSERT INTO operation_steps(operation_id,sequence,step_code,state,attempt_count)
    SELECT id,1,'MATERIALIZE',
      CASE WHEN state IN ('COMPLETE','REGISTERED') THEN 'SUCCEEDED' ELSE 'RECOVERY_REQUIRED' END,1
    FROM intake_operations;
INSERT INTO operation_steps(operation_id,sequence,step_code,state,attempt_count)
    SELECT id,2,'REGISTER',
      CASE WHEN state IN ('COMPLETE','REGISTERED') THEN 'SUCCEEDED' ELSE 'PENDING' END,
      CASE WHEN state IN ('COMPLETE','REGISTERED') THEN 1 ELSE 0 END
    FROM intake_operations;
UPDATE intake_operations SET state='RECOVERY_REQUIRED' WHERE state='PLANNED';
