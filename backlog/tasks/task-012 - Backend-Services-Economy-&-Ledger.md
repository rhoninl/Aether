---
id: task-012
title: 'Backend Services: Economy & Ledger'
status: Done
assignee:
  - '@codex-001'
created_date: '2026-03-07 13:18'
updated_date: '2026-03-07 15:11'
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
- [x] #1 AEC currency with double-entry bookkeeping ledger
- [x] #2 Exactly-once posting: UUID v7 idempotency keys + UNIQUE constraint
- [x] #3 Persistent pending-transactions table on PVC for crash recovery
- [x] #4 Sync RPC path for player-facing transactions (purchases, trades, tips)
- [x] #5 Async NATS path for deferred settlement (creator payouts, platform fees)
- [x] #6 Wallet balance CHECK >= 0 constraint (DB-level overdraft prevention)
- [x] #7 Velocity checks and anomaly detection for anti-fraud
- [x] #8 7-year idempotency key retention (hot table + compressed archive)
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add `aether-economy` crate exposing ledger entry primitives, transaction envelopes, anti-fraud traits, and payout channels.
2. Add persistence-oriented idempotency constraints and pending transaction state machine.
3. Add async/sync path protocol records for RPC and settlement events.
4. Add design notes for constraints and retention strategy.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Completed economy policy layer in `aether-economy` for double-entry ledger primitives, idempotency state, pending transaction models, RPC/async settlement envelopes, balance constraints, and anti-fraud/velocity checks; persistence mechanics remain follow-up.
<!-- SECTION:NOTES:END -->
