# Replay

## Purpose

This document defines deterministic replay: how a fresh (or restarted)
node rebuilds `ChainState` entirely from the on-disk block log, with no
in-memory state carried over from a previous run. It is a design document
for Day 4 — replay should be built to match this document, not the other
way around, the same convention `docs/storage.md` and
`docs/block-validation.md` already follow.

This is the piece `docs/storage.md`'s "Known gap" left open: recovery
(`storage::recovery::recover`) only validates each record *in isolation*
— magic, version, length, checksum, header self-consistency, commit
marker. It does not check that record N+1 actually chains onto record N
(`parent_hash` continuity, height sequencing), and nothing in the codebase
today replays a recovered log through `ChainState` to catch that. Replay
is that missing step.

## Non-goals

- **Fork choice.** This document assumes a single linear log, matching
  `docs/storage.md`'s non-goals ("the log assumes a single linear
  canonical chain"). Comparing or choosing between competing histories is
  separate, later Day 4 work with its own design doc.
- **Incremental / checkpointed replay.** Every replay run starts from
  genesis and processes the entire log. There is no snapshot or
  checkpoint format yet that would let a node resume from partway through
  its own history — see "Open questions" below.
- **Networking, or fetching blocks from a peer.** Replay only ever reads
  from the local block log.
- **Repairing a log that fails replay.** See "Replay is fatal, not
  best-effort" below — replay does not attempt to patch, skip, or
  otherwise recover from a block that fails to reproduce its declared
  state root. That is what `storage::recovery::recover` already did, at
  the structural level, before replay ever started.

## The core rule

```text
genesis state
+ ordered, storage-valid block log
+ deterministic transaction execution
= exact final state root

Replay must fail if any historical block does not reproduce its declared
state root. There is no partial trust: a log that replays 999 blocks
correctly and fails on the 1000th did not "mostly" rebuild the chain — it
failed to rebuild the chain, full stop.
```

This is the same discipline `docs/block-validation.md`'s core rule
applies to importing one new block, and `docs/storage.md`'s "Stopping,
not skipping" applies to reading the log — extended to cover an entire
run of history instead of one block or one record.

## Relationship to recovery

Replay runs *after* storage-level recovery, not instead of it, and the
two layers check different things:

```text
storage::recovery::recover  → structural validity of each record on its
                               own: magic, version, length, checksum,
                               header self-consistency, commit marker.
                               Stops at, and truncates away, the first
                               structurally bad or partial record.

replay                       → semantic validity of the resulting block
                               sequence: does it actually chain together,
                               and does executing it really produce the
                               state roots it claims to?
```

A record can pass every structural check recovery performs and still be
wrong at the replay layer — wrong parent hash, wrong height, a forged (but
internally self-consistent) transaction, a declared state root that
execution doesn't actually reproduce. Recovery cannot catch any of that; it
never decodes far enough to know what a "correct" next block even looks
like, because it has no `ChainState` to check against. Replay is where
that context exists.

## The replay algorithm

```text
1. Run storage-level recovery (`BlockStore::recover`) to truncate away any
   partial or structurally corrupted tail. Record its report — in
   particular `last_valid_height` / `last_valid_hash` — for the
   cross-check in step 5.
2. Load the now-structurally-clean log in order (`BlockStore::load_blocks`).
3. Start from the fixed genesis `ChainState` (see "Open questions" —
   where this comes from is not yet defined).
4. For each block in the loaded order, run it through the existing block
   import pipeline (`state::ChainState::execute_block`) against the state
   produced by the previous step. Do not re-implement any check replay
   needs — execute_block already performs every one of them:
     - block hash self-consistency (`BlockHeader::verify_hash`)
     - parent hash continuity and height sequencing (`validate_header`)
     - transaction Merkle root
     - duplicate transaction IDs, within the block and across history
       (`included_transaction_ids`)
     - every transaction's signature (`SignedTransaction::verify`)
     - nonce ordering (strict equality against the sender's current nonce)
     - fee accounting, charged atomically with the transfer
     - the block's declared state root against what execution actually
       produced
   The first block that fails any of these aborts replay entirely — see
   "Replay is fatal, not best-effort."
5. After every block has replayed successfully, the resulting
   `ChainState.tip()` must match the recovery report's
   `last_valid_height` / `last_valid_hash` from step 1. A mismatch means
   storage-level recovery accepted a record that didn't actually belong
   in this chain — precisely the chain-linkage gap `docs/storage.md`
   flags — and is itself a replay failure, not something to silently
   reconcile.
6. If every block replayed and the tip cross-check in step 5 passed, the
   resulting `ChainState` is the node's canonical state, and its tip is
   the chain's tip. The node may now accept new blocks.
```

## Replay is fatal, not best-effort

Recovery (`docs/storage.md`) truncates and moves on — a corrupted tail is
expected, survivable, and the node still starts. Replay does not have
that option. If a block in the *structurally clean* portion of the log
fails to execute, something worse than an interrupted write happened:
either the log was tampered with by something with write access, or a
previous run wrote a block that shouldn't have been accepted in the first
place, or there's a bug in `execute_block` or in whatever produced the
log. None of those are safe to paper over by skipping the block and
continuing — the node must refuse to start rather than come up with
state it cannot prove is correct. This mirrors `docs/block-validation.md`:
"a block either clears every gate and becomes canonical, or it clears
none of the effects of any gate" — applied here to an entire startup
sequence instead of one block.

## What replay verifies, mapped to existing checks

| Requirement (this doc) | Where it's already implemented |
|---|---|
| Parent hash continuity | `ChainState::validate_header`, called first inside `execute_block` |
| Block hash | `BlockHeader::verify_hash`, also inside `validate_header` |
| Transaction Merkle root | `execute_block`, compares recomputed root to `header.transaction_root` |
| Signatures | `SignedTransaction::verify`, called by `apply_signed_transaction` for every transaction |
| Nonce ordering | `apply_signed_transaction`, strict equality against the sender's current nonce |
| Fee accounting | `apply_signed_transaction`, checked arithmetic, atomic with the transfer (`docs/fee-model.md`) |
| State root after every block | `execute_block`, compares the post-execution `state_commitment()` to `header.state_commitment` |

Every row already exists as a check inside `execute_block`. Replay adds
no new validation logic of its own — its entire job is calling
`execute_block` once per block, in log order, starting from genesis, and
treating any failure as fatal rather than as something to route around.

## Open questions

- **Where does genesis state come from?** `execute_block`'s genesis
  convention (checked in `validate_header`) only constrains the *header*
  of the first block: `height == 0`, `parent_hash == GENESIS_PARENT_HASH`.
  It says nothing about the *account* state replay should start from —
  today, every test constructs its own ad hoc `ChainState` (accounts,
  balances, fee collector) by hand (see `setup_parent_state()` in
  `crates/state/tests/execute_block.rs`). There is no canonical,
  on-disk-or-otherwise genesis configuration a real node could load. This
  has to be resolved — a fixed genesis account set defined somewhere
  deterministic — before replay is actually implementable, not just
  designed.
- **What does the caller do after a fatal replay failure?** This document
  says replay must refuse to start. It does not yet say what the node
  process does next (exit with an error, refuse only new-block
  acceptance while still serving reads, etc.) — an operational decision,
  not a correctness one, deferred to whatever implements the node CLI.
- **Incremental replay / checkpointing.** Full replay-from-genesis is
  correct but doesn't scale indefinitely. A future state-snapshot format
  (checkpoint a `ChainState` + the height it corresponds to, replay only
  the log past that point) is out of scope for Day 4 but will eventually
  be necessary — noted here so it isn't forgotten, not designed here.

## Implementation status

Not yet implemented. `crates/chain`'s `Chain::import_block` already
contains the "run `execute_block`, then only commit on success" half of
this — `Chain::new` currently requires the caller to already have a
`ChainState` in hand, with no path from "just opened a `BlockStore`" to
"have a replayed `Chain`." That constructor (something like
`Chain::from_storage`, running the algorithm above) is the natural next
piece of work, once the genesis-state open question is resolved.
