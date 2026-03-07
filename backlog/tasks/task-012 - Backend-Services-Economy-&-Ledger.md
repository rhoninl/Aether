---
id: task-012
title: 'Backend Services: Economy & Ledger'
status: To Do
assignee: []
created_date: '2026-03-07 13:18'
updated_date: '2026-03-07 14:13'
labels: []
dependencies:
  - task-011
priority: high
ordinal: 11000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement centralized economy service: AEC currency, double-entry bookkeeping, exactly-once transaction posting, wallet management, creator payouts, and anti-fraud.

Ref: docs/design/DESIGN.md Section 4.4
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 AEC currency with double-entry bookkeeping ledger
- [ ] #2 Exactly-once posting: UUID v7 idempotency keys + UNIQUE constraint
- [ ] #3 Persistent pending-transactions table on PVC for crash recovery
- [ ] #4 Sync RPC path for player-facing transactions (purchases, trades, tips)
- [ ] #5 Async NATS path for deferred settlement (creator payouts, platform fees)
- [ ] #6 Wallet balance CHECK >= 0 constraint (DB-level overdraft prevention)
- [ ] #7 Velocity checks and anomaly detection for anti-fraud
- [ ] #8 7-year idempotency key retention (hot table + compressed archive)
<!-- AC:END -->
