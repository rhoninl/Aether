# API Gateway & Voice Relay (task-027)

Added gateway and relay contracts for auth/rate-limits, routing, and STUN/TURN support.

## Implemented API surface

- Added crate `aether-gateway`.
- Added auth validation, rate limiting, route selection, and relay profile models.
- Added NAT mode, relay session, and regional routing contracts.
- Updated workspace membership for this crate.

## Remaining implementation work

- Implement edge service runtime, certificates, and monitoring for throttling and failures.
- Implement relay pool provisioning and failover.
