# `minerva-node`

A minimal CLI for driving a `minerva-chain` node by hand. It exists to
demonstrate the pieces built in earlier hours (storage, block validation,
replay, the mempool) end to end, from the command line — it is not a
server and does not talk to a network. Every command is its own
short-lived process: nothing is kept in memory between commands. State is
always rebuilt from the on-disk block log via `chain::Chain::from_storage`
(see `docs/replay.md`), the same way a real node restart would.

## Genesis

There is no genesis config file. Every command derives the same fixed
genesis independently: three named accounts (`alice`, `bob`, `carol`),
each starting with a balance of `1,000,000`, plus a `fee-collector`
account starting at `0`. An account's 32-byte id is just the SHA-256 hash
of its name — `--from alice` always resolves to the same account, on
every command, on every run, with nothing to keep in sync. Names other
than these four don't exist as accounts; transactions naming them will be
rejected by pool admission when a block is produced.

## Data layout

`--data-dir` (default `./data`) holds:

- `blocks.log` — the durable, append-only block log (`storage::AppendOnlyBlockStore`).
- `mempool.dat` — pending transactions queued by `submit-tx`, not yet in a block.

A directory is "initialized" once `blocks.log` exists in it; every command
other than `init` checks for that first.

## Commands

```text
minerva-node init [--data-dir DIR]
    Creates DIR (if needed) and its block log.

minerva-node submit-tx --from NAME --to NAME --amount N --nonce N [--data-dir DIR]
    Signs a transaction and queues it in the mempool. Does not touch the
    block log or check it against current chain state -- that happens at
    produce-block time, the same admission control `mempool::TransactionPool`
    already implements.

minerva-node produce-block [--data-dir DIR]
    Rebuilds chain state, pulls every pending transaction whose nonce is
    next-in-line for its sender (mempool::TransactionPool::ready_transactions),
    builds a block from them, and imports it (chain::Chain::import_block).
    Transactions that fail pool admission are dropped; transactions with a
    future nonce stay queued for a later block. Produces an empty block if
    nothing is ready -- there is no minimum transaction count.

minerva-node import-block PATH [--data-dir DIR]
    Decodes a single block record (the same format storage::record uses)
    from PATH and imports it. See "Simulating two nodes" below for how to
    produce one.

minerva-node replay [--data-dir DIR]
    Runs storage recovery + full replay from genesis (execution::replay_chain)
    and reports how many blocks replayed, the final state root, and the tip.

minerva-node tip [--data-dir DIR]
    Prints the current canonical tip (height + hash), or "none" pre-genesis.

minerva-node accounts [--data-dir DIR]
    Prints alice/bob/carol/fee-collector's balance and nonce.

minerva-node validate-storage [--data-dir DIR]
    Runs storage-level recovery only (no replay) and reports what it found:
    valid record count, whether the tail needed truncating, and why.
```

## Example session

```bash
cargo build -p node
BIN=./target/debug/minerva-node

$BIN init --data-dir ./data
# initialized node data directory at ./data

$BIN accounts --data-dir ./data
# alice          balance 1000000      nonce 0
# bob            balance 1000000      nonce 0
# carol          balance 1000000      nonce 0
# fee-collector  balance 0            nonce 0

$BIN submit-tx --data-dir ./data --from alice --to bob --amount 10 --nonce 0
$BIN submit-tx --data-dir ./data --from bob --to carol --amount 5 --nonce 0
# queued tx <hash>: alice -> bob amount 10 nonce 0
# queued tx <hash>: bob -> carol amount 5 nonce 0

$BIN produce-block --data-dir ./data
# produced block 0 (2 tx) -> <hash>

$BIN tip --data-dir ./data
# tip: height 0 -> <hash>

$BIN accounts --data-dir ./data
# alice          balance 999989       nonce 1
# bob            balance 1000004      nonce 1
# carol          balance 1000005      nonce 0
# fee-collector  balance 2            nonce 0

$BIN replay --data-dir ./data
# replayed 1 block(s) from genesis
#   final state root: <hash>
#   tip: height 0 -> <hash>

$BIN validate-storage --data-dir ./data
# valid records: 1
# original length: 554 bytes
# final length: 554 bytes
# no corruption found
# last valid record: height 0 -> <hash>
```

## Simulating two nodes with `import-block`

There's no networking, so "receiving a block from a peer" is simulated by
handing another node's block log to `import-block`. If a data directory has
produced exactly one block, its `blocks.log` *is* a single-block record
file — no separate export step needed:

```bash
$BIN init --data-dir ./nodeA
$BIN submit-tx --data-dir ./nodeA --from alice --to bob --amount 25 --nonce 0
$BIN produce-block --data-dir ./nodeA
cp ./nodeA/blocks.log ./block.bin

$BIN init --data-dir ./nodeB
$BIN import-block --data-dir ./nodeB ./block.bin
$BIN tip --data-dir ./nodeB
# tip: height 0 -> <same hash nodeA produced>
```

## Logs

Every command prints structured logs to stderr, at `info` level by
default. Set `RUST_LOG` to change what's shown (e.g.
`RUST_LOG=debug`, or `RUST_LOG=storage=trace` to see just the storage
crate at trace level). See `docs/logging.md` for the full event catalog
(`transaction_submitted`, `block_imported`, `replay_completed`, etc.) and
what fields each one carries:

```bash
$BIN produce-block --data-dir ./data
# <timestamp>  INFO state: block_validation_started height=1 block_hash=... parent_hash=...
# <timestamp>  INFO storage: storage_record_appended height=1 block_hash=... parent_hash=...
# <timestamp>  INFO chain: block_imported height=1 block_hash=... parent_hash=... state_root=...
# produced block 1 (1 tx) -> ...
```

## Non-goals

No networking, no key management (transactions use the deterministic
placeholder signature scheme from `crypto::signature`, not real keys), no
fork choice or competing branches (that's `block::fork_choice`, not wired
into this CLI), and no daemon/long-running process — every invocation
starts cold from whatever is already on disk.
