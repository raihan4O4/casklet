# ADR 0035: Explicit resource policy and recoverable native recycling

Status: Accepted for the user-requested ownership, restoration and categorization corrections, 2026-10-20.

## Ownership

This supersedes ADR 0034's reference promotion through duplicate Add as Owned. All four duplicate-add combinations preserve the existing association policy and category. Intake compares normalized canonical paths and native object identities independently of names; a file handle can also identify legacy records and equivalent paths. Native object keys exclude mutable content hashes. No identity observation itself grants deletion authority.

SetResourceOwnership is a dedicated shared-core command with Cask/resource IDs, the expected current policy and explicit ownership consent. The GUI exposes Change ownership in item details; the CLI exposes resource ownership. Promotion and folder renewal inspect and revalidate the source, check supported location/identity, refuse links, broad locations and conflicting dependencies, and commit a verified grant. Demotion removes the Cask's grant and changes the association to reference without requiring an accessible source or changing bytes. Original internal allocations retain their location and provenance and can be explicitly restored to ownership after validation. Installation artifact records remain provider-managed and cannot be changed through the content-policy command.

Policy writes are short immediate transactions with active-Cask, current-association and operation/cleanup/installer lock checks. Resource/association revisions invalidate older cleanup plans. Folder children inherit the parent policy without independent grants. Other Casks' policies and dependencies remain intact; no sharing grants ownership. Missing or stale requests fail clearly. Repeating Add never renews ownership evidence; the dedicated renewal action does.

## Windows Recycle Bin

The old trash path renamed the verified source into a .casklet-removal identifier before submitting it to Windows. Consequently Windows faithfully recorded the wrong original name and location. Recycle Bin handoff now revalidates the original source immediately before calling the existing trash adapter at its original path. That adapter uses Windows IFileOperation with undo and no-permanent-delete-warning handling, checks aborted operations, and returns failures. Casklet independently verifies source disappearance before recording success. Durable intent/locks remain established before handoff; exceptions and interruptions remain uncertain with associations and locks retained. No filesystem transaction is claimed across a Shell operation and SQLite. Restoration through Windows does not resurrect retired associations or grants.

Quarantine remains for permanent deletion and recovery of historical interrupted runs. No existing recovery data or Recycle Bin entries are rewritten. This fix applies to new recycling operations. A crash around native handoff is still an uncertain journal outcome that requires inspection, not an automatic replay.

Reference: [Microsoft IFileOperation::DeleteItem](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ifileoperation-deleteitem). The installed trash 5.2.9 Windows source also checks GetAnyOperationsAborted; it supplies FOF_ALLOWUNDO and FOF_WANTNUKEWARNING.

## Categorization

The shared resource-intake command resolves the reserved auto default into the existing built-in categories. EXE/MSI/MSIX/APPX use applications; documents, images, videos, audio and common archives use their existing categories. Extensions are case-insensitive; compound archives such as tar.gz resolve to archives. Unknown/extensions-free files and directories fall back to other. Explicit category choices win, and duplicate intake preserves existing manual categories. GUI/drop, CLI and Explorer use this same default. Installer content remains separate from installation execution/tracking.

## Evidence

Regression fixtures cover the ownership matrix for files/folders and EXE/MSI content, explicit transitions both ways, consent and cancellation, unchanged bytes/location, internal allocations, inherited folder contents, stale cleanup plans, equivalent native file objects/hard links, category defaults/manual overrides, and Windows native Restore for original names with spaces, Unicode and compound extensions. Restore fixtures target only their unique generated parent in the Recycle Bin. They use Windows' Restore verb because the trash crate exposes display names with hidden extensions, making its restore_all helper unsuitable for this acceptance test.
