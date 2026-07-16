# Minerva Chain

minerva-chain is an educational Rust blockchain execution prototype.
It focuses on account-based state, signed transactions, deterministic execution,
block validation, state commitments, and adversarial testing.
It is not a production blockchain, not a consensus protocol, and not a cryptocurrency.

## Purpose

This repository is meant to explore how a simple blockchain execution engine might be structured in Rust.
It is intended for learning, discussion, and experimentation rather than deployment.

## Current status

The project is in an early stage. The repository contains notes, architecture documents, and a small Rust codebase that is still being shaped.

## Design goals

- Keep the implementation understandable and explicit.
- Model core execution concepts in a minimal way.
- Favor deterministic behavior where possible.
- Use tests to document assumptions and guard against regressions.

## Non-goals

- Building a production-ready blockchain.
- Implementing a consensus protocol.
- Creating a cryptocurrency or token economy.
- Supporting real-world network deployment.

## Repository structure

- crates/ - core Rust crates for the system.
- docs/ - architecture and invariant notes.
- notes/ - working notes from the development process.
- tests/ - test cases and integration checks.

## How to run tests

Run the test suite from the repository root with:

```bash
cargo test
```

## Current Week 3 status

Week 3 work is in progress on the mempool, fee model, and durable storage.
So far:

- Implemented a nonce-aware transaction pool.
- Added duplicate transaction rejection.
- Added invalid signature rejection.
- Added future nonce queueing.
- Added stale nonce rejection.
- Added deterministic transaction ordering.
- Implemented an append-only on-disk block log with a checksummed record
  format (`crates/storage`).
- Implemented crash recovery: scans the log from the start, accepts valid
  records, and stops at the first corrupted or partial one — never skips
  over damaged bytes to resync on something that looks valid further in.
- Added a `chain` crate that ties block validation/execution to storage:
  a block is only treated as imported (canonical state and tip updated)
  once both validation *and* the durable storage append have succeeded.

This is a snapshot of in-progress work, not a finished system. There is no
networking, no block producer that actually pulls from the pool, no pool
size limits or eviction, and no guarantee that a transaction accepted into
the pool is still valid by the time it would be included in a block.
Recovery also does not yet verify that consecutive records actually chain
to each other (parent hash / height) — only that each record is valid on
its own. See `notes/w3d1-mempool.md`, `docs/fee-model.md`,
`notes/day-03-storage-recovery.md`, and `docs/storage.md` for the detailed
design and implementation notes.

## Limitations

This is a prototype. It is not intended to be secure, performant, or complete enough for production use.
It should be treated as an educational artifact and a place to reason about execution design.

