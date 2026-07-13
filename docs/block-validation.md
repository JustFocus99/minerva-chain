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

As of Day 1 (mempool work), `state::chain_state::ChainState::execute_block`
implements part of this pipeline:

- Step 5 (transaction Merkle root) — implemented: recomputes the root from
  `block.transactions` and compares it to `block.header.transaction_root`.
- Step 8 (execute transactions against candidate state) — implemented: the
  function clones `parent_state` into a local `temp_state` and applies each
  transaction to that clone via `apply_signed_transaction`. `parent_state`
  itself is never touched — it's an immutable `&ChainState` reference, so
  the "canonical state untouched on failure" property holds by construction,
  not just by convention.
- Step 9 (atomic fee charging) — implemented inside
  `apply_signed_transaction` (see `docs/fee-model.md`).
- Steps 10–11 (candidate state root, compare declared root) — implemented:
  `temp_state.state_commitment()` is compared against
  `block.header.state_commitment`.
- Step 12 (commit only after all checks pass) — implemented at the
  function-return level: `temp_state` is only ever returned as `Ok(..)`
  after every prior step succeeds; the caller is responsible for actually
  treating the returned state as the new canonical state.

Not yet implemented — open work for later hours:

- Step 1 (decode). There is no wire format or decoder in this codebase yet;
  `Block` values are constructed directly in Rust. (Same gap as the
  mempool's "malformed transaction bytes" case — see
  `notes/w3d1-mempool.md`.)
- Step 2 (validate header) as a distinct, explicit check.
- Step 3 (parent hash validation). `ChainState` currently has no notion of
  "the hash of the block it was built from," so there is nothing to compare
  `block.header.parent_hash` against yet.
- Step 4 (height validation). `ChainState` does not track height at all.
- Step 6 (explicit duplicate transaction ID rejection within a block).
  Today, submitting the same transaction twice in one block will usually
  fail on the second application because the sender's nonce already
  advanced — but that's an incidental side effect of nonce checking, not a
  deliberate, explicit duplicate-ID check.
- Step 13 (persistence). There is no durable storage layer; everything is
  in-memory `ChainState`.

## Non-goals for Day 2

- Fork choice or handling competing chains — this pipeline assumes a single
  linear chain of blocks presented for import.
- Networking or block propagation.
- Persistence format design beyond "only write it once it's canonical."

These may be revisited in a later milestone.
