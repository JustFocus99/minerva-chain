# Fork Choice

## Purpose

This document defines local fork choice for minerva-chain: the rule a
single node uses to pick one canonical tip when it knows about more than
one valid chain of blocks. It is a design document for Day 4 Hour 4 —
fork choice should be built to match this document, not the other way
around, the same convention `docs/storage.md`, `docs/block-validation.md`,
and `docs/replay.md` already follow.

`docs/block-validation.md` and `docs/replay.md` both explicitly assume a
single linear chain and list fork choice as a non-goal. This is the
document that fills that gap — but only the local, structural half of it.
See "Non-goals" below.

## The core rule

```text
Among all currently known branch tips, choose the one with the greatest
height.

If two or more tips share the greatest height, choose the one whose
block hash is lexicographically smallest.
```

That's the entire rule. It is deliberately not:

- proof-of-work (greatest cumulative difficulty)
- proof-of-stake (greatest attestation weight)
- anything that weighs blocks by who produced them, when they arrived, or
  how much stake or work backs them

It is a **local, deterministic tie-break** over a set of block hashes and
heights already known to this node. Two nodes that have imported the same
set of blocks, regardless of the order they arrived in, must compute the
same canonical tip. That property — determinism independent of arrival
order — is the only thing this rule is required to guarantee, and it's
also the only thing tested (`handles_competing_forks`,
`tie_breaks_by_block_hash`).

## What fork choice is not

Fork choice, as defined here, does not re-run block validation. A block
handed to `ForkTree::insert_block` is assumed to have already cleared
`ChainState::execute_block` (see `docs/block-validation.md`) or replay
(see `docs/replay.md`) — fork choice only organizes headers that are
already known to be individually valid, by hash and parent hash. It does
not check signatures, transaction roots, state roots, or height
continuity; it trusts the height a header declares. Feeding it a header
that lied about its own height, or that never passed validation, is a
caller bug, not something fork choice is designed to catch.

This mirrors the separation `docs/replay.md` draws between storage
recovery (structural) and replay (semantic): `ForkTree` is the structural
layer — "which headers exist, and how do they connect" — not the
semantic layer that decided each one was individually valid.

## Branches, tips, and the canonical chain

```text
Genesis
   |
Block 1
   |
Block 2
  /     \
Block 3A Block 3B
```

- A **branch** is a path from genesis to some block with no children (a
  **tip**, sometimes called the **head**).
- The **canonical chain** is the path from genesis to whichever tip fork
  choice currently selects.
- A block that is not on the canonical chain (`Block 3B` above, if `3A`
  wins) is not discarded — it stays known, in case a later block extends
  it past the current canonical tip's height.

## `ForkTree`

`ForkTree` is the structure that stores every known block header by hash
and answers "what is the canonical tip right now?" It does not store
transactions or execute anything; it only needs each header's own hash,
parent hash, and height.

```text
blocks_by_hash:     hash -> BlockHeader
parent_by_hash:     hash -> parent hash
height_by_hash:      hash -> height
children_by_parent:  hash -> [hashes of blocks that declare it as parent]
canonical_tip:       hash of the currently-selected tip
```

A `ForkTree` is always rooted at a genesis header, supplied to
`ForkTree::new`. Genesis is trivially "known" from construction — every
other block must chain, directly or transitively, back to it via
`parent_hash` before it can be inserted.

### `insert_block`

```text
1. If this block's hash is already in blocks_by_hash, it is AlreadyKnown.
   Nothing is mutated -- not the tree, not the canonical tip.
2. Otherwise, if this block's parent_hash is not in blocks_by_hash, it is
   rejected: UnknownParent. Nothing is mutated.
3. Otherwise, the block is recorded (blocks_by_hash, parent_by_hash,
   height_by_hash, and it is added as a child of its parent). The
   canonical tip is then recomputed over every current tip (every block
   with no children) using the core rule.
```

Two competing blocks that share a parent (`Block 3A` and `Block 3B` above)
both pass step 2 — a known parent does not mean an *only* child — and
both end up stored. Only one becomes `canonical_tip`; the other remains a
known, non-canonical tip until something extends it further or the branch
that beat it grows even further ahead.

Recomputing the canonical tip from every known tip on every insert is
`O(number of known tips)`, not `O(1)`. That trade-off — simplicity over
incremental-update performance — is the same one `docs/storage.md` and
`docs/replay.md` already make elsewhere in this codebase for Week 3; it is
not the right choice for a node tracking a large, long-lived tip set.

### `choose_tip`

Recomputes the canonical tip from scratch by scanning every block with no
children and applying the core rule, independent of `canonical_tip`'s
incrementally-maintained value. It exists so tests (and callers who don't
trust incremental bookkeeping) can ask "what tip does this rule produce
for exactly this set of blocks, regardless of insertion order?" — which is
precisely the determinism property this document requires. A correctly
maintained `ForkTree` always has `canonical_tip() == choose_tip()`.

## `ChainBranch`

A branch's tip, reduced to exactly the two fields the core rule compares:

```text
ChainBranch { tip_hash, height }
```

## `ForkChoice`

The comparison rule itself, factored out of `ForkTree`'s storage so it can
be tested and reasoned about on its own: given two `ChainBranch` values,
which one wins?

```text
is_better(candidate, current):
    if candidate.height > current.height: candidate wins
    if candidate.height < current.height: current wins
    if candidate.height == current.height:
        candidate wins iff candidate.tip_hash < current.tip_hash
```

`BlockHash` is `[u8; 32]`, whose `Ord` implementation is already
byte-wise lexicographic — "lexicographically smallest hash" is exactly
what `candidate.tip_hash < current.tip_hash` means, with no separate
encoding step.

## Required behavior

```text
reject a block if its declared parent is unknown
accept a competing block if its declared parent is known, even if that
  parent already has another child
choose the tip with the greatest height
tie-break tips at equal height by smallest block hash
do not import the same block twice -- a repeat insert is a no-op, not a
  duplicate branch and not a canonical-tip change
```

## Non-goals

- **Re-validating blocks.** See "What fork choice is not" above --
  `ForkTree` trusts that anything handed to `insert_block` already passed
  `ChainState::execute_block` or replay.
- **Reorg execution.** Choosing a different canonical tip than before is
  a pure bookkeeping change in `ForkTree`. Actually rolling `ChainState`
  back and replaying the new canonical branch's blocks -- what a real
  reorg would require -- is not implemented here and not exercised by
  these tests.
- **Networking or block propagation.** `ForkTree` only organizes blocks
  already handed to it in-process, the same boundary
  `docs/replay.md` draws for replay.
- **Proof-of-work, stake weighting, validator votes, attestations,
  slashing, or any consensus protocol.** This is a deterministic local
  tie-break, not consensus between untrusting nodes. See "The core rule"
  above.
- **Pruning non-canonical branches.** A tip that loses stays in
  `ForkTree` indefinitely; there is no eviction of stale or
  far-behind branches yet.
