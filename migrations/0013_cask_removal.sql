CREATE TABLE cask_removals (
    cask_id TEXT PRIMARY KEY REFERENCES casks(id),
    removed_at_ms INTEGER NOT NULL,
    expected_revision INTEGER NOT NULL
) STRICT;
