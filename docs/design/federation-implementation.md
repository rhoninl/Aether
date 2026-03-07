# Federation Model (task-021)

Added federation contracts for self-hosted world registration, asset hash policies, token trust, and central gates.

## Implemented API surface

- Added crate `aether-federation`.
- Added self-hosted world registration states and registry contract.
- Added asset hash verification policy and integrity mismatch actions.
- Added auth mode and central trust gate descriptors.

## Remaining implementation work

- Implement runtime federation server behaviors and world manifest propagation.
- Implement periodic approval and drift checks.
