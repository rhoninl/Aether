---
id: task-012
title: 'Backend Services: Economy & Ledger'
status: In Progress
assignee: []
created_date: '2026-03-07 13:18'
updated_date: '2026-03-07 14:58'
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

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add `aether-economy` crate exposing ledger entry primitives, transaction envelopes, anti-fraud traits, and payout channels.
2. Add persistence-oriented idempotency constraints and pending transaction state machine.
3. Add async/sync path protocol records for RPC and settlement events.
4. Add design notes for constraints and retention strategy.
<!-- SECTION:PLAN:END -->
