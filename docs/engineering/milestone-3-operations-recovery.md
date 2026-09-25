# Milestone 3 — Durable operations and recovery

Status: implementation complete locally on `codex/durable-operations-recovery`; awaiting user smoke testing and review.

Managed copy and move now create an operation, two ordered steps (`MATERIALIZE`, `REGISTER`), an intake snapshot, and a Cask mutation lock in one SQLite transaction before filesystem work. Progress and cancellation requests are persisted. Cancellation is checked between copy chunks, during directory traversal, and before move/rename boundaries. Interrupted or cancelled work retains candidate files and enters `RECOVERY_REQUIRED`; no recovery action deletes files.

Startup checks nonterminal operations. On Windows, it asks the OS whether each recorded owner PID still exists. An unknown answer is treated as possibly alive. A dead owner moves the operation to recovery-required; the lock remains until explicit resolution. This prevents another CLI or desktop process from starting managed intake in the same Cask while an operation is active or unresolved. The desktop polls operation and recovery status every three seconds in addition to receiving progress events.

The CLI exposes `casklet operation list|get|cancel|recover|preserve`, `casklet recovery status|inspect|retry-manifests`, and `casklet diagnostics bundle <output-path>`. The desktop shows running operations, cancellation, recovery status, stored paths on request, explicit recovery/preserve actions, manifest retry, and diagnostics save. Both use the same application service.

`operation recover` is an explicit acceptance of the currently observed staged or final item. It checks that recorded paths still match this Cask's allocation, that a moved source is absent, and that the candidate has the recorded kind and size with no unknown directory entries. A staged item is rechecked for root identity before and after rename. Registration and operation completion are transactional. Already completed recovery is idempotent. A legacy Milestone 2 intent without a full snapshot, partial stage, link, conflicting source, or ambiguous staging/final pair remains unresolved. `operation preserve` closes a recovery-required operation and releases its lock without deleting or moving any candidate file; the journal remains available for diagnostics.

Manifest projection jobs are queued with committed Cask/resource/category changes. A job reads committed SQLite state, writes and verifies a temporary manifest, swaps it into place, writes a SHA-256 sidecar, and marks only the projected revision complete. Failed jobs remain pending and are retried at startup or by an explicit command. Diagnostic events contain operation IDs and fixed codes; the JSON bundle includes bounded events, operation status, and pending-manifest count, without resource names or paths.

The Windows tests cover migration from Milestone 1 and 2 schemas, concurrent intake lock conflicts, cancellation during copy, worker interruption, partial staging, complete staged move, final file before registration, registration before completion, idempotent recovery, ambiguous move preservation, projection failure/retry, interrupted manifest replacement, and redacted diagnostics. A child-process fault matrix forces exit after intent, stage, final rename, registration, and completion, then checks recovery after restart. The existing link, junction, collision, and disk-full tests remain in the workspace gate.

The full `cargo xtask check` gate passed locally on Windows: Rust formatting, Clippy, tests and builds, plus frontend formatting, lint, type checks, component tests, and build.

Manual review point: verify GUI polling and cancellation during a real long copy on Windows. No removal command is introduced in this milestone.
