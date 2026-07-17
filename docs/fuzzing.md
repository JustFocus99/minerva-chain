# Fuzzing

## Purpose

This document defines what fuzzing covers in minerva-chain, and the rule
every fuzz target must satisfy. It is a design document for Day 5 Hour 2
(decoders) and Hour 3 (execution and recovery, chained onto those same
decoders) — fuzz targets should be built to match this document, not the
other way around, the same convention every other `docs/*.md` in this
repository follows.

## Why decoders specifically

Everywhere else in this codebase, a `Block` or `SignedTransaction` value
is either constructed directly in-process (by a test, or by
`minerva-node`) or has already passed the checks
`docs/block-validation.md` and `docs/fee-model.md` define. Decoding is
different: it's the one place raw, arbitrary bytes — a corrupted disk
sector, a byte-flipped file, bytes handed to `minerva-node import-block`
— get turned into those trusted types in the first place. Nothing
upstream of a decoder has validated anything yet, by definition. That
makes `storage::record::decode_record` (the block decoder) and
`storage::record::decode_signed_transaction` (the transaction decoder,
factored out of it — see `record.rs`'s `TRANSACTION_RECORD_LEN`) the two
functions in this codebase where "arbitrary bytes" is not a hypothetical
input, it's the actual expected one.

## Chaining decoders into execution and recovery

Hour 2's two targets stop at "did the decoder panic." Hour 3 adds two more
that chain a decoder onto the next thing untrusted bytes actually reach in
this codebase, so that panic-freedom is proven end-to-end rather than only
at the decode boundary:

- `state_transition_input` decodes bytes with `decode_record` /
  `decode_signed_transaction` and, on success, runs the result through
  `state::chain_state::ChainState::execute_block` /
  `apply_signed_transaction` against a small fixed test state. A value
  that survives decoding is only *structurally* well-formed (see "Why
  decoders specifically" above) — it hasn't been checked for a valid
  signature, a real sender, a correct nonce, or anything else
  `docs/block-validation.md` and `docs/fee-model.md` require. This target
  proves execution rejects such a value with `Err` instead of panicking,
  not that execution's validation logic is correct (that's
  `state::chain_state`'s own test suite's job).
- `storage_recovery` treats the fuzzer's bytes as the entire contents of
  an on-disk log file and runs `storage::recovery::recover` over it — the
  same scan-and-truncate algorithm `AppendOnlyBlockStore::recover` uses
  after a real crash. Unlike the other three targets, this one does real
  filesystem I/O (via a reused `tempfile` scratch file, to keep iteration
  cost down) because `recover`'s signature takes a `&Path`/`&File`, not a
  byte slice.

Both remain within the same fuzz rule below; they just apply it one layer
further downstream.

## The fuzz rule

```text
Random bytes may return an error.
Random bytes must not panic.
Random bytes must not cause undefined behavior.
Random bytes must not be accepted as valid unless they fully pass validation.
```

Concretely, for all four targets (with the last two bullets specific to
`block_decoder` / `transaction_decoder` — see "Chaining decoders into
execution and recovery" above for what `state_transition_input` /
`storage_recovery` assert instead):

- `Err` is a completely normal, expected outcome for most inputs — a
  fuzz target that never hits its `Err` branch would only be testing the
  happy path, not fuzzing anything.
- A panic (an `unwrap`/`expect`/slice-index/arithmetic overflow that
  wasn't supposed to be reachable, or an uncontrolled allocation request
  built from attacker-controlled length bytes — see "What this already
  caught" below) is always a bug in the decoder, never a valid response
  to malformed input.
- Since this is 100% safe Rust (no `unsafe` in the decode path), "no
  undefined behavior" reduces to "no panic" plus whatever
  AddressSanitizer (on by default under `cargo fuzz run`) would catch —
  there's no raw pointer arithmetic or manual memory management here for
  UB to hide in.
- A decoded value must not be trusted beyond what decoding actually
  checked. Both fuzz targets assert this directly: `block_decoder` checks
  that a successfully decoded block's header hash is self-consistent
  (exactly what `decode_record`'s `BlockHashMismatch` check already
  guarantees, re-checked independently); `transaction_decoder` checks that
  re-encoding a successfully decoded transaction reproduces the original
  bytes exactly (a decoder that "succeeds" without a byte-exact
  round-trip silently dropped or misread something).

## What this already caught

Before any fuzzing ran, `decode_block`'s transaction loop read an
untrusted `u32` transaction count directly off the wire and called
`Vec::with_capacity(tx_count as usize)` before decoding a single
transaction. Four attacker-chosen bytes could request an allocation of
any size up to `u32::MAX * size_of::<SignedTransaction>()` — Rust's
allocator aborts the process on an allocation it can't satisfy, which is
not a catchable `Err`, unlike everything else this decoder does. The fix
(now in `record.rs`) is to grow the `Vec` from empty instead of
pre-allocating from an untrusted hint — the same protection every other
field in this format already had via `read_array`'s bounds check, just
missing here because `Vec::with_capacity` runs before any byte of the
claimed transactions is actually read. This is exactly the class of bug
fuzzing exists to find — a real crash from a few bytes of hostile input
— caught by reasoning about the code, since a random `libFuzzer` corpus
is unlikely to guess a `tx_count` anywhere near `u32::MAX` on a short
timescale. The two fuzz targets below still cover it, because nothing
stops a future change from reintroducing a similar bound.

## Running

```bash
# one-time setup
rustup toolchain install nightly
cargo install cargo-fuzz

# from the repository root
cargo +nightly fuzz run transaction_decoder -- -max_total_time=30
cargo +nightly fuzz run block_decoder -- -max_total_time=30
cargo +nightly fuzz run state_transition_input -- -max_total_time=30
cargo +nightly fuzz run storage_recovery -- -max_total_time=30
```

Week 3 doesn't need hours of continuous fuzzing — a short, timed run
(`-max_total_time`) confirms all four targets build, run, and survive a
number of generated inputs without crashing. `state_transition_input`
covers a few million iterations in that time, same order as the two
decoder-only targets; `storage_recovery` is much slower (real filesystem
I/O per iteration) and covers on the order of tens of thousands — still
enough to confirm it builds and survives, not enough for deep coverage.
Longer runs (overnight, in CI) are future work, not something this
repository's test suite depends on.

## `fuzz/` is its own workspace

`fuzz/Cargo.toml` has its own `[workspace]` table, deliberately detaching
it from the root workspace (`Cargo.toml`) — libFuzzer instrumentation
needs a nightly toolchain the rest of this project doesn't, and a fuzz
crate's dependency resolution shouldn't perturb the main workspace's. This
is `cargo fuzz`'s own convention, not something specific to this
repository. `fuzz/corpus/`, `fuzz/artifacts/`, `fuzz/target/`, and
`fuzz/coverage/` are gitignored (`fuzz/.gitignore`) — generated fuzzing
state, not source.

## Non-goals

- **Structural coverage of every type in this codebase.** Only values
  reachable from actual untrusted-byte entry points are fuzzed — the two
  decoders directly, plus `ChainState::execute_block` /
  `apply_signed_transaction` and `storage::recovery::recover` transitively,
  via those same decoders (see "Chaining decoders into execution and
  recovery" above). Other in-process-only APIs
  (`TransactionPool::submit_transaction`, …) that never receive decoder
  output aren't fuzzed — fuzzing raw bytes at that boundary would just be
  fuzzing whatever test harness constructed them, not the system.
- **Coverage-guided regression tracking in CI.** No fuzzing job is wired
  into `.github/` yet; these targets are run manually, per this document.
- **Semantic validation via fuzzing.** A fuzz target proves a decoder
  doesn't crash on hostile bytes, not that a *decoded* value is
  semantically valid (signature verifies, sender exists, nonce lines up).
  That's `tests/property_tests.rs` and each crate's own `tests/`, over
  in-process values, not raw bytes.
