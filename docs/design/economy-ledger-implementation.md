# Economy & Ledger (task-012)

Implemented a foundational economy crate with transaction, wallet, ledger, payout, and anti-fraud primitives.

## Implemented API surface

- Added crate `aether-economy` with modules:
  - `ledger`: double-entry record structs (`LedgerEntry`, `LedgerRecord`).
  - `transaction`: sync/async transaction envelopes and idempotency record types.
  - `wallet`: wallet operations with non-negative invariant at execution time.
  - `payout`: payout records and async settlement stream models.
  - `fraud`: anti-fraud anomaly and velocity signal types.
- Updated workspace membership to include `aether-economy`.

## Mapping to acceptance criteria

- `#1` Double-entry primitives defined in `ledger`.
- `#2` `IdempotencyRecord` includes UUID-v7-style key field and retention metadata.
- `#3` In-memory pending semantics represented by `TransactionCoordinator` and state fields.
- `#4` Sync transaction path represented by `TransactionKind::Sync` and `EconomyTransaction` payload.
- `#5` Async settlement path represented by `TransactionKind::AsyncSettlement` and `SettlementStream`.
- `#6` `WalletConstraint` enforces minimum balance of zero by default.
- `#7` Fraud signals in `fraud.rs` for velocity and outlier detection.
- `#8` `IdempotencyRecord::ttl_days` supports retention planning and archive split designs.

## Remaining implementation work

- Replace in-memory vectors with persistence and indexed constraints.
- Add unique constraint enforcement for idempotency keys and retention jobs.
- Add platform-fee and creator payout accounting flows.
