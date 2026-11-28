CREATE TABLE resource_ownership_grants (
    resource_id TEXT PRIMARY KEY REFERENCES resources(id),
    cask_id TEXT NOT NULL REFERENCES casks(id),
    exact_path TEXT NOT NULL,
    identity_key TEXT NOT NULL,
    consent_at_ms INTEGER NOT NULL
) STRICT;
ALTER TABLE removal_plans ADD COLUMN cleanup_only INTEGER NOT NULL DEFAULT 0 CHECK (cleanup_only IN (0,1));
UPDATE removal_plans SET state='INVALIDATED',revision=revision+1 WHERE state IN ('DRAFT','APPROVED');
