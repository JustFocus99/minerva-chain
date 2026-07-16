# Block Validation

## Purpose

This document defines the block import pipeline for minerva-chain: the
checks a block must pass before it is allowed to change canonical state. It
is a design document for Day 2 — block import should be built to match this
pipeline, not the other way around.

## The core rule

```text
A block does not become canonical merely because it is well-formed.

Canonical state must never be mutated during validation.
Only candidate state may be mutated before final commit.
```

Block import is guilty until proven innocent. Every step below is a chance
to reject the block. A block that fails any single step is fully discarded
— not partially applied, not applied-then-rolled-back. Canonical state is
only ever replaced in one atomic step, after every check has passed.

## Canonical vs. candidate state

Two different `ChainState` values are in play during import, and they must
never be confused:

```text
Canonical state  → the current, agreed-upon state of the chain.
                    Never touched until a block fully validates.

Candidate state   → a working copy, cloned from canonical state, that
                    absorbs the block's transactions during validation.
                    Thrown away entirely if anything fails.
```

This is the block-level version of the same rule `apply_signed_transaction`
already follows per-transaction (see `docs/fee-model.md`'s atomicity
section): mutate a scratch copy, validate it, and only promote it to be the
new source of truth if every check passed. Nothing in between is ever
observable — there is no state where canonical state reflects half a block.

## The import pipeline

Each numbered step is a hard gate. If a step fails, import stops
immediately, canonical state is untouched, and the block is rejected with a
specific error — later steps never run and never get a chance to mutate
anything.

1. **Decode block.** Turn the incoming bytes into a `Block` struct. If the
   bytes don't parse, the block is rejected before any of its fields are
   even inspected.
2. **Validate header.** Structural sanity of the header fields themselves
   (e.g. non-degenerate values) before anything derived from them is
   trusted.
3. **Validate parent hash.** `block.header.parent_hash` must match the hash
   of the block canonical state was actually built from. This is what
   prevents a block from attaching to the wrong point in history.
4. **Validate height.** `block.header.height` must be exactly one greater
   than the parent's height. This catches skipped or replayed heights.
5. **Validate transaction Merkle root.** Recompute the Merkle root over the
   block's transaction IDs and compare it to `block.header.transaction_root`.
   This binds the header to the exact transaction list and their order —
   the header can't claim one set of transactions while the block carries
   another.
6. **Reject duplicate transaction IDs.** No transaction ID may appear twice
   within the same block. A block that tries to spend or replay the same
   transaction twice is malformed, independent of whether execution would
   incidentally also catch it via a nonce mismatch.
7. **Verify all signatures.** Every transaction's signature must verify
   against its own contents before any of them are executed. Signature
   verification never depends on execution order.
8. **Execute transactions against candidate state.** Apply each transaction
   in block order to the candidate state clone: check sender/receiver
   existence, nonce sequencing, and balance sufficiency, and apply the
   transfer. This is where `state::ChainState::apply_signed_transaction`
   runs. The first transaction that fails aborts the whole block — nothing
   partially executes.
9. **Charge fees atomically.** Each transaction's fee debit and transfer
   happen together or not at all (see `docs/fee-model.md`). This is part of
   step 8, not a separate pass — fee accounting is not decoupled from
   execution.
10. **Compute candidate state root.** After every transaction in the block
    has executed successfully, hash the resulting candidate state.
11. **Compare declared state root.** The computed root must equal
    `block.header.state_commitment`. A block whose declared post-state
    doesn't match what execution actually produced is rejected — this
    catches a producer lying about, or miscalculating, the result of its
    own block.
12. **Commit candidate state only after all checks pass.** Only once every
    prior step has succeeded does candidate state replace canonical state.
    This is the single atomic promotion point — before it, canonical state
    is exactly what it was before import started.
13. **Persist block only after validation succeeds.** The block itself is
    only written to durable storage once it is canonical. A block that
    fails validation leaves no trace in persisted history.

## Failure handling

Any step failing means:

```text
Canonical state:  unchanged
Candidate state:  discarded
Block:            rejected, not persisted
```

There is no partial-success outcome. A block either clears every gate and
becomes canonical, or it clears none of the effects of any gate. This
mirrors the "no partial mutation" rule already established for individual
transactions and for the mempool's admission checks — block import is the
same discipline applied one level up.

## Implementation status

As of Day 2 Hour 4, `state::chain_state::ChainState::execute_block`
implements most of this pipeline:

- Step 2 (validate header) / Step 3 (parent hash) / Step 4 (height) —
  implemented in `ChainState::validate_header`, called first, before any
  other check. `BlockHeader` now carries a cached `block_hash` (computed
  once in `BlockHeader::new`) and `ChainState` tracks `tip: Option<BlockHeader>`
  — the header of the last committed block, `None` if this state has no
  history yet. When `tip` is `None`, the incoming header must satisfy the
  genesis convention (`height == 0`, `parent_hash == GENESIS_PARENT_HASH`,
  a fixed all-zero value). Otherwise it must chain onto the tip:
  `parent_hash == tip.block_hash` and `height == tip.height + 1`.
- "Block hash matches header bytes" — implemented as part of the same
  `validate_header` call, via `BlockHeader::verify_hash()`, which recomputes
  the hash over the header's other fields and compares it to the cached
  `block_hash`. This catches a header whose fields were mutated after it
  was hashed.
- Step 5 (transaction Merkle root) — implemented: recomputes the root from
  `block.transactions` and compares it to `block.header.transaction_root`.
- Step 6 (reject duplicate transaction IDs within a block, and reject a
  transaction already committed in an earlier block) — implemented as a
  read-only pre-pass over the block's transaction IDs, before candidate
  state is even created: a `BTreeSet` catches duplicates within the block
  (`StateError::DuplicateTransactionInBlock`), and a lookup against
  `ChainState::included_transaction_ids` (every transaction ID ever
  committed, carried forward across blocks) catches cross-block replay
  (`StateError::ReplayedTransaction`).
- Step 7 (verify signatures) / stale nonce / nonce gaps / insufficient
  balance / fee overflow / integer overflow — all already covered by
  `apply_signed_transaction`, which every transaction in the block runs
  through unchanged; a block is just a sequence of those same per-transaction
  checks. See `tests/executor.rs` for that coverage and `docs/fee-model.md`
  for the fee-specific rules.
- Step 8 (execute transactions against candidate state) — implemented via
  `StateSnapshot` (see `snapshot.rs`): `parent_state` is only ever borrowed,
  never mutated, so "canonical state untouched on failure" holds by
  construction, not just by convention.
- Step 9 (atomic fee charging) — implemented inside
  `apply_signed_transaction` (see `docs/fee-model.md`).
- Steps 10–11 (candidate state root, compare declared root) — implemented:
  the snapshot's state commitment is compared against
  `block.header.state_commitment`.
- Step 12 (commit only after all checks pass) — implemented at the
  function-return level: the new state (with its tip and included
  transaction IDs updated) is only ever returned as `Ok(..)` after every
  prior step succeeds; the caller is responsible for actually treating the
  returned state as the new canonical state.
- Step 13 (persistence). Implemented by the `chain` crate: `Chain::import_block`
  calls `ChainState::execute_block` first, appends the block to a
  `storage::BlockStore` only if that succeeds, and only then replaces
  canonical state (`self.state = candidate_state`). If storage append
  fails, canonical state — including the tip — is left exactly as it was;
  the candidate state produced by `execute_block` is simply dropped. See
  `docs/storage.md` and `crates/chain/tests/import.rs`.
  `included_transaction_ids` growing without bound across the whole chain's
  history is still the same full-state trade-off documented in
  `snapshot.rs` — acceptable for Week 3, not for production.

Not yet implemented — open work for later hours:

- Step 1 (decode). There is no wire format or decoder in this codebase yet;
  `Block` values are constructed directly in Rust. (Same gap as the
  mempool's "malformed transaction bytes" case — see
  `notes/w3d1-mempool.md`.)
- "Previous state root" as an explicit header field was considered and
  deliberately left out: `execute_block` always builds candidate state from
  the real `parent_state` directly, never from an externally declared
  "previous state root" value, so there's no untrusted transmission path
  for a redundant field to protect against yet. Worth revisiting if blocks
  ever arrive from an untrusted source instead of being constructed
  in-process.
- Timestamp was left off `BlockHeader` entirely — no deterministic rule for
  it has been defined, and the project's determinism requirements
  (`docs/architecture.md`) explicitly rule out wall-clock time affecting
  execution.

## Non-goals for Day 2

- Fork choice or handling competing chains — this pipeline assumes a single
  linear chain of blocks presented for import.
- Networking or block propagation.
- Persistence format design beyond "only write it once it's canonical."

These may be revisited in a later milestone.
