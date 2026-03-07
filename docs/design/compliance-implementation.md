# Privacy & Data Compliance (task-020)

Added compliance-focused primitives for deletion, pseudonymization, legal hold, retention, and export.

## Implemented API surface

- Added crate `aether-compliance` with deletion, export, keystore, and retention modules.
- Added legal hold status and multi-scope deletion requests.
- Added export artifacts and retention state transition primitives.

## Remaining implementation work

- Implement secure keystore with audit logging and dual-approval enforcement.
- Implement financial-record retention schedule mechanics.
