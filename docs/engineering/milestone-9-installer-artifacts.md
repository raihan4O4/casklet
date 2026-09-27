# Milestone 9: Installer artifacts and cautious correlation

## Workflow

1. Inspect an explicitly selected `.msi` or `.exe`. Show format, source path, SHA-256, size, Windows signature state, status detail and signer, product hints, and safety warning. Inspection does not execute the file.
2. Register a hash-matched managed copy under the Cask's Applications resources. The original file remains at its source location. Registration grants no installation ownership.
3. Review the registered copy and explicitly approve launch. EXE elevation is a separate choice; Windows UAC remains visible. Casklet itself stays unelevated.
4. A durable operation locks installation mutation, snapshots Windows uninstall registrations, launches the exact copy with typed arguments, and snapshots registrations afterward. New registrations are scored as candidates, not facts of ownership.
5. Explicitly confirm a candidate after reviewing it. Casklet rechecks its registry evidence and creates a detach-only association. The user can instead close the review while preserving the external result.

## Safety and recovery

The Windows adapter holds a read-only file handle denying write and rename, rehashes those bytes, and runs full-path `msiexec.exe` for MSI or the exact copied EXE, with no shell command string or inferred EXE arguments. Fixed PowerShell scripts perform read-only Authenticode/MSI inspection and optional EXE UAC launch; installer paths are passed as environment values. Output from metadata inspection is bounded and parsed as JSON. The process has a one-hour limit and cooperative cancellation. Timeout or cancellation kills the tracked parent; descendants can remain, so the outcome stays uncertain.

An MSI exit code 0 permits candidate review but never ownership. EXE exit 0 still requires recovery because child processes may continue. MSI 3010/1641 indicates reboot required and enters recovery. Nonzero exit, launch failure, changed bytes or signature, absent/ambiguous registration, and incomplete inventory never create ownership. Recovery locks remain until explicit close or evidence-matched association confirmation. Active installer locks block other provider mutations and destructive removal of the Cask.

## Acceptance evidence

| Criterion                                                    | Evidence                                                                                                                        |
| ------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------- |
| No arbitrary-installer cleanup guarantee                     | CLI/GUI and inspection warnings state this; confirmed candidates use `DETACH_ONLY`.                                             |
| Signer is evidence, not safety                               | Native Authenticode status/signer are shown with explicit warning; unsigned or unknown status is not called safe.               |
| Multiple, missing, or weak candidates never create ownership | Snapshot/correlation code only generates candidates; tests cover ambiguity, failure, and confirmation as association only.      |
| Elevation limited to installer                               | Optional EXE `Start-Process -Verb RunAs` targets the copy; MSI uses system `msiexec`; desktop stays unelevated.                 |
| Child-process/reboot uncertainty retained                    | EXE exit 0, timeouts, cancellations, failures, and MSI 3010/1641 enter durable recovery; fake-adapter tests cover these states. |

Automated validation uses a fake provider and a fake executable with metacharacters in its path; no real third-party installer is launched. Read-only native inspection was checked against a Windows signed system executable. Before public release, manually validate real MSI metadata, Windows Installer UI/UAC, optional EXE UAC, reboot behavior, and recovery in an isolated Windows 11 x64 environment.
