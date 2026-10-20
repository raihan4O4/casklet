# ADR 0034: Idempotent resource intake and explicit reference adoption

Status: Superseded in part by ADR 0035: duplicate adds now always preserve policy; policy changes require the dedicated command. Original decision recorded 2026-10-20.

Reference intake reuses an active item at the same canonical path in the destination Cask, including an already owned item. It never downgrades ownership or changes category on a retry. Persistence returns the actual stable resource ID. Immediate SQLite transactions serialize lookup and registration across processes; no filesystem work occurs inside those transactions.

Explicit AdoptResource consent may promote an exact external USER_REFERENCE in the same Cask, retaining its ID/category and adding verified ownership evidence. New source inspection, location validation, identity revalidation, locks, and other-Cask/ancestor/descendant overlap checks remain mandatory. An existing explicitly adopted file or folder may renew evidence through the same command. Installation artifacts are not eligible for this conversion: installer-file content and installation tracking remain distinct.

Historical same-Cask duplicate USER_REFERENCE associations at that path are retired transactionally when the path is added again or explicitly adopted. Audit resource records and file bytes remain intact; projection work is queued. Other Casks, application artifact records, and ownership grants remain dependencies. A failed promotion rolls back duplicate retirement too. Existing removal plans must pass their normal association/identity revalidation.

This applies to GUI, CLI and Explorer intake for files, folders and EXE/MSI content. It does not execute installers or automatically migrate old references into ownership. No schema migration or destructive file operation is introduced.
