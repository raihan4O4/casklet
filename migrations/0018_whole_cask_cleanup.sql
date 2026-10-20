-- Persist the reviewed intent to retire a Cask after its existing cleanup phases.
CREATE TABLE whole_cask_cleanup_intents (
    id TEXT PRIMARY KEY,
    cask_id TEXT NOT NULL REFERENCES casks(id),
    resource_plan_id TEXT REFERENCES removal_plans(id),
    application_plan_id TEXT REFERENCES msi_cleanup_plans(id),
    created_at_ms INTEGER NOT NULL,
    CHECK (resource_plan_id IS NOT NULL OR application_plan_id IS NOT NULL)
) STRICT;
CREATE INDEX whole_cleanup_by_cask ON whole_cask_cleanup_intents(cask_id,created_at_ms);
