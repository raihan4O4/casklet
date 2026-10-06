-- Extend the existing ownership/queue system; references are never promoted.
ALTER TABLE managed_msi_installations ADD COLUMN uninstaller_sha256 TEXT;
ALTER TABLE managed_msi_installations ADD COLUMN registration_json TEXT NOT NULL DEFAULT '{}';
UPDATE managed_msi_installations SET registration_json=(SELECT evidence_json FROM windows_installations WHERE id=installation_id);
ALTER TABLE msi_cleanup_steps RENAME TO legacy_msi_cleanup_steps;
CREATE TABLE msi_cleanup_steps (
 plan_id TEXT NOT NULL REFERENCES msi_cleanup_plans(id),
 installation_id TEXT NOT NULL REFERENCES windows_installations(id),
 state TEXT NOT NULL, result_code TEXT, at_ms INTEGER NOT NULL,
 PRIMARY KEY(plan_id,installation_id)
) STRICT;
INSERT INTO msi_cleanup_steps SELECT * FROM legacy_msi_cleanup_steps;
DROP TABLE legacy_msi_cleanup_steps;
UPDATE msi_cleanup_plans SET state='CLOSED_PRESERVED',plan_json=json_set(plan_json,'$.state','CLOSED_PRESERVED') WHERE state IN ('DRAFT','APPROVED');
