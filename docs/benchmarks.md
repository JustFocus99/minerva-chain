# Benchmarks

## Purpose

Rough, local timings for the three operations most likely to matter for
"how much does this cost as the chain grows": executing a block
(`state::chain_state::ChainState::execute_block`), replaying a chain from
genesis (`execution::replay_chain`), and appending blocks to the durable
log (`storage::AppendOnlyBlockStore::append_block`). Built with
[Criterion](https://github.com/bheisler/criterion.rs) — statistical
sampling, warm-up, and outlier detection, but still a single machine, a
single run, no isolation from everything else that happened to be running
on it at the time.

**These benchmarks are local educational measurements, not production
throughput claims.**

## Machine

- CPU: Intel Core i7-1185G7 @ 3.00GHz (8 logical cores)
- RAM: 15 GiB
- OS: Ubuntu 24.04.4 LTS, kernel `5.15.153.1-microsoft-standard-WSL2`
  (**WSL2 on Windows, not bare-metal Linux** — the filesystem underneath
  `storage_append` in particular goes through the WSL2 virtualized I/O
  path, not a native Linux block device)
- Rust: `rustc 1.96.1`, `cargo 1.96.1` (stable, not the `nightly`
  toolchain the `fuzz/` crate uses)

## Command used

```bash
# from the repository root
cargo bench --bench block_execution
cargo bench --bench replay
cargo bench --bench storage_append
```

Each is a separate `[[bench]]` target in the root `Cargo.toml`
(`harness = false`, so Criterion supplies its own `main`, not the built-in
`#[bench]` harness). `benches/common/mod.rs` builds the genesis state,
blocks, and chains each target measures — that construction happens once
per benchmarked input, outside Criterion's timed `b.iter`/`b.iter_batched`
closure.

## Results

All three accounts/transfers setups use 16 funded participant accounts,
one-unit transfers, round-robining senders so nonces stay valid. Ranges
are Criterion's `[low estimate, point estimate, high estimate]` from its
default 100-sample run.

### `execute_block` — one block, N transactions

| transactions | time |
|---|---|
| 10 | 7.65 µs |
| 100 | 70.0 µs |
| 1,000 | 759.6 µs |

### `replay_chain` — genesis through N blocks (5 tx/block)

| blocks | total tx | time |
|---|---|---|
| 100 | 500 | 690.7 µs |
| 1,000 | 5,000 | 26.56 ms |

### `AppendOnlyBlockStore::append_block` — 100 blocks (5 tx/block)

| operation | time |
|---|---|
| append 100 blocks | 241.3 ms (≈2.41 ms/block) |

## What the numbers mean

- **`execute_block` scales close to linearly with transactions per
  block**, as expected: the dominant work per transaction is a handful of
  `BTreeMap` lookups/writes on a small (16-account) state, a placeholder
  signature check (see below), and a `state_commitment()` hash of every
  account at the end — none of which depend on transactions outside the
  one block being executed. 10→100→1,000 tx is roughly 9x→11x in cost for
  a 10x→10x growth in size, consistent with "linear plus a small constant
  per-block overhead" rather than anything worse.
- **`replay_chain` does *not* scale linearly with chain length**, and
  that's real, not benchmark noise: 10x more blocks (100→1,000, 500→5,000
  total tx) costs about 38x more time, not 10x. The likely cause is
  visible directly in the code, not just inferred from the numbers:
  `ChainState::execute_block` builds each block's candidate state via
  `StateSnapshot::from_canonical`, which does a full `ChainState::clone()`
  — including `included_transaction_ids`, the set of every transaction ID
  ever committed, which `chain_state.rs` already documents as growing
  unboundedly (see its doc comment and `snapshot.rs`'s "Trade-off"
  section, which calls full-state cloning "simple and safe for an
  educational prototype" but explicitly not what a production
  implementation would do). Cloning a set that grows by 5 every block, once
  per block, over N blocks is `O(N²)` work in the transaction count, not
  `O(N)` — which is consistent with what these two data points show.
- **`storage_append` is dominated by `fsync`, not by encoding.** Each
  `append_block` call does two `write_all` + `flush` + `sync_data`
  round-trips (body, then commit marker — see "The commit marker" in
  `docs/storage.md`) specifically so a crash mid-write leaves a
  recoverable partial record rather than a corrupt one. ~2.4 ms/block on
  this machine is a statement about this machine's disk and WSL2's I/O
  virtualization layer far more than it is a statement about the encoding
  logic in `storage::record`.

## What these numbers do not prove

- **Not a production throughput or capacity claim of any kind.** No
  concurrency, no real network or consensus layer, no realistic mixed
  workload, one machine, one run.
- **Not representative of real signature-verification cost.**
  `crypto::signature::sign_message`/`verify_signature` are a deterministic
  placeholder (a hash compared to a hash — see `crypto/src/signature.rs`),
  not real Ed25519 despite `ed25519-dalek` being a dependency. A build
  that actually verifies Ed25519 signatures per transaction would make
  `execute_block`'s numbers meaningfully higher; these numbers say nothing
  about that cost.
- **Not portable across machines, OSes, or disks.** `storage_append` in
  particular is a filesystem/`fsync` measurement taken through WSL2's
  virtualized disk path on one laptop — a different disk (real NVMe on
  bare-metal Linux, a network filesystem, a CI runner's ephemeral storage)
  would plausibly give a very different number, in either direction.
- **Not a regression baseline.** There's no CI job wired up to run these
  benchmarks or fail on drift (matching `docs/fuzzing.md`'s "Coverage-guided
  regression tracking in CI" non-goal for the same reason) — these are
  one-time, manually-run measurements, not a tracked baseline to compare
  future runs against.
- **Not proof the `replay_chain` scaling described above is a bug to fix
  this week.** It's a real, explainable consequence of a trade-off the
  code already documents as deliberate for "Week 3" (see
  `snapshot.rs`). Whether it's worth fixing is a scope question, not
  something these numbers answer by themselves.
