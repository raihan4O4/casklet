-- Installation provenance is separate from automatic uninstall authority.
-- Legacy/manual references are deliberately not promoted.
ALTER TABLE installer_operations ADD COLUMN installed_for_cask INTEGER NOT NULL DEFAULT 0 CHECK(installed_for_cask IN (0,1));
