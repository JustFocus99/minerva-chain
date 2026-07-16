# Structured Logging

## Purpose

This document defines how minerva-chain emits structured logs: which
events exist, what fields they carry, and what must never appear in them.
It is a design document for Day 4 Hour 6 — logging should be added to
match this document, not the other way around, the same convention every
other `docs/*.md` in this repository follows.

## Library vs. binary

Every crate that emits an event depends on `tracing` (the facade — event
macros, no output behavior of its own) and calls `tracing::info!`/`warn!`
directly at the point something happened. None of them depend on
`tracing-subscriber` or install a subscriber; a library that decided how
its own logs get formatted or filtered would make that decision for every
binary that ever links it. Only `crates/node` (the one binary in this
workspace) depends on `tracing-subscriber` and installs a subscriber, in
`main.rs`, before doing anything else. Running any other crate's tests
without a subscriber installed is fine — the `tracing` macros are no-ops
until something subscribes, so nothing needs a subscriber just to compile
or run.

## Event catalog

Each event is a single `tracing::info!`/`warn!` call with a fixed
message and a fixed set of fields — not free-form log text. `info!` is
used for events describing expected, successful progress; `warn!` for
events describing an expected rejection (a bad transaction, a block that
failed validation, a corrupted log tail) — neither is a crash or a bug,
just something worth being able to find later.

| Event | Level | Where | Fields |
|---|---|---|---|
| `transaction_submitted` | info | `mempool::TransactionPool::submit_transaction` | `tx_id`, `account_id` |
| `transaction_rejected` | warn | `mempool::TransactionPool::submit_transaction` | `tx_id`, `account_id`, `error` |
| `block_validation_started` | info | `state::ChainState::execute_block` | `height`, `block_hash`, `parent_hash` |
| `block_validation_failed` | warn | `state::ChainState::execute_block` | `height`, `block_hash`, `error` |
| `block_imported` | info | `chain::Chain::import_block` | `height`, `block_hash`, `parent_hash`, `state_root` |
| `storage_record_appended` | info | `storage::AppendOnlyBlockStore::append_block` | `height`, `block_hash`, `parent_hash` |
| `storage_recovery_started` | info | `storage::recovery::recover` | (none) |
| `storage_recovery_completed` | info | `storage::recovery::recover` | `valid_records`, `truncated_bytes`, `height`, `block_hash`, `error` |
| `replay_started` | info | `execution::replay_chain` | `block_count` |
| `replay_failed` | warn | `execution::replay_chain` | `index`, `height`, `block_hash`, `error` |
| `replay_completed` | info | `execution::replay_chain` | `blocks_replayed`, `state_root`, `height`, `block_hash` |
| `fork_choice_updated` | info | `block::ForkTree::insert_block` | `block_hash`, `height`, `parent_hash` |

`block_validation_started`/`block_validation_failed` bracket the same
call `docs/block-validation.md`'s pipeline runs inside — they don't
duplicate that pipeline's per-step checks, they mark its boundary.
Likewise `storage_recovery_started`/`completed` bracket
`docs/storage.md`'s recovery scan, and `replay_started`/`failed`/
`completed` bracket `docs/replay.md`'s block-by-block replay.
`fork_choice_updated` only fires when `insert_block` actually changes the
canonical tip — inserting a block onto a branch that loses the fork-choice
comparison is not itself a tip change (see `docs/fork-choice.md`).

## Field meanings

- `height` — a block's `BlockHeader::height` (`u64`).
- `block_hash` / `parent_hash` / `state_root` — 32-byte hashes,
  lower-hex-encoded via `primitives::to_hex` so every event formats them
  the same way. Never logged as raw bytes or via `{:?}` on the array
  (illegible, and inconsistent between events).
- `tx_id` — a transaction's `TransactionId`, also hex-encoded.
- `account_id` — a 32-byte `AccountId`, also hex-encoded. This is a
  public identifier, not secret material — see below.
- `error` — the rejecting/failing error's `Display` output (every error
  type involved already implements `std::error::Error` via `thiserror`).

## What must never be logged

No event may log a private key, a signature, or any other secret
material. Concretely: `SignedTransaction::signature` and
`SignedTransaction::public_key` never appear in a log field, even though
this codebase's placeholder signature scheme
(`crypto::signature::sign_message`) has no real secret behind it today —
the rule is about what a real signing scheme would make secret, not about
whether today's placeholder happens to be sensitive. Only `account_id`
(a public identifier a transaction's `from`/`to` already reveals) and
`tx_id` (a public hash of the transaction) are logged. This is a
guardrail worth naming even though no key management exists yet
(`crates/node/README.md`'s "Non-goals"), so it isn't reconsidered the
first time real keys show up.

## Non-goals

- **A subscriber configuration story beyond `RUST_LOG`.** `minerva-node`
  defaults to `info` level and otherwise respects `RUST_LOG`
  (`tracing_subscriber::EnvFilter`); there is no structured sink, no log
  rotation, no JSON output format decided yet.
- **Tracing spans / distributed tracing.** Every event here is a single
  point-in-time `info!`/`warn!` call, not a span covering a request's
  lifetime. There's no cross-process correlation to carry, since there's
  no networking yet (`docs/architecture.md`'s non-goals).
- **Asserting on log output in tests.** Nothing in this workspace's test
  suite captures or asserts against `tracing` output; these events exist
  for a human (or a log aggregator) reading `minerva-node`'s output, not
  for test correctness.
