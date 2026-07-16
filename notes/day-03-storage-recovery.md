# Day 03 — Append-only block storage and crash recovery

## What storage needs to guarantee

Up through Week 3 Day 2, `ChainState` was purely in-memory — a block that
validated and executed successfully only ever lived as long as the process
did. Day 3's job is to make committed blocks durable, and to do it in a way
that survives the process dying mid-write, not just the happy path.

Core rule (see `docs/storage.md`):

```text
A block is durable only if it is stored as a complete, verifiable record.
The log is append-only: existing bytes are never rewritten in place.
A record is either fully present and valid, or it is not there at all.
```

The interesting problem isn't writing bytes to a file — it's what happens
when the process is killed halfway through a write, or a byte flips on
disk afterward. Both have to be *detected*, never silently absorbed. A
storage layer that quietly accepts a truncated or bit-flipped record as
data is worse than one that refuses to start.

## On-disk format, briefly

```text
[magic][version][length][height][hash][payload][crc32][commit marker]
```

`height` and `hash` are duplicated from the payload's own header fields so
a reader can identify a record without fully decoding it. The commit
marker is the last byte written for a record — see "The exact commit
point" below. Full field-by-field layout is in `docs/storage.md`; this
note is about what building it was actually like.

## Implementation retrospective

### What I implemented

- `crates/storage/src/record.rs` — `encode_block` / `decode_block` for the
  payload (header fields, then transaction count, then each transaction's
  from/to/amount/nonce/public key/signature, all little-endian), and
  `encode_record_without_marker` / `decode_record` for the full record
  framing. Validity is checked in a fixed order — magic, version, length
  sanity, payload decode, checksum, block hash match, commit marker — so
  cheap structural checks reject obviously-wrong data before paying for
  decode and hashing.
- `crates/storage/src/append_log.rs` — `AppendOnlyBlockStore`, an
  `OpenOptions::new().append(true)` file wrapper. `append_block` is a
  two-phase write: write + `flush` + `sync_data` the record body, *then*
  write + `flush` + `sync_data` the single commit-marker byte, as two
  separate fsync'd steps. `load_blocks` decodes every record from offset 0
  and errors on the first bad one — it does not run recovery itself.
- `crates/storage/src/recovery.rs` — `recover()` scans from byte 0 via
  `decode_record`, accepts records until the first failure, then
  `File::set_len`s the file back to the offset the scan stopped at. Returns
  a `RecoveryReport` (`valid_records`, `original_len`, `final_len`,
  `truncated_bytes`, `last_valid_height`/`last_valid_hash`,
  `rejected_reason`).
- `crates/chain` (new crate, Hour 7) — `Chain<S: BlockStore>` wraps a
  `ChainState` and a `BlockStore` together, and `import_block` enforces the
  ordering: validate + execute (`ChainState::execute_block`) → append to
  storage → *only then* replace canonical state. This is the piece
  `docs/block-validation.md` step 13 was left open pending.

### What corruption detection looks like

`decode_record` catches, per record: magic mismatch, unsupported version,
an insane or corrupted length field, truncation (not enough bytes for the
prefix/payload/suffix — covers both "file ends early" and "a crash cut a
write short"), a payload that doesn't decode cleanly, a CRC32 mismatch
over `height || hash || payload` (catches bit flips anywhere in that
range, including inside the encoded transactions), a declared record hash
that doesn't match the recomputed header hash, and a missing/unset commit
marker.

`recovery::recover` stops at the *first* record that fails any of the
above — it does not try to resync and pick up records after it, even if
they'd decode cleanly on their own. That's a deliberate property, not an
accident of the loop: a bad record in the middle usually means the write
that produced it was interrupted, and anything after it can't be trusted
to be real chain history in the right order. See "Stopping, not skipping"
in `docs/storage.md`.

### What tests were added

Hour 3 (`tests/append_log.rs`): `appends_single_block`,
`appends_multiple_blocks`, `loaded_blocks_match_written_blocks`.

Hour 4 — malformed bytes (`tests/decoder.rs`): `rejects_empty_file`,
`rejects_random_bytes`, `rejects_wrong_magic`, `rejects_wrong_version`,
`rejects_wrong_length`, `rejects_bad_checksum`, `rejects_truncated_payload`.

Hour 5 — interrupted-write recovery (`tests/recovery.rs`):
`recovers_after_interrupted_write`, `truncates_partial_record`,
`does_not_import_partial_block`.

Hour 6 — partially corrupted database, corruption in the middle
(`tests/recovery.rs`): `stops_at_first_corrupted_record`,
`does_not_skip_middle_corruption`, `reports_corruption_offset`. These
specifically construct block1 (valid) / block2 (bit-flipped, complete and
committed, not truncated) / block3 (perfectly well-formed) and assert
recovery accepts only block1 — proving the "stop, don't skip" rule holds
even when what comes after corruption looks fine.

Hour 7 — storage/block-import integration (`crates/chain/tests/import.rs`):
`imports_valid_block_persists_and_updates_tip`,
`invalid_block_does_not_reach_storage`,
`storage_append_failure_does_not_commit_state`,
`storage_append_failure_does_not_update_tip`. The last two use a
`FailingBlockStore` test double (always errors on `append_block`) rather
than real fault injection, since there's no clean way to make a real file
write fail on demand in a portable test.

### What changed along the way

- `RecoveryReport`'s fields were originally `accepted_records` /
  `last_accepted_height` / `last_accepted_hash`. Renamed to `valid_records`
  / `last_valid_height` / `last_valid_hash` — "accepted" reads ambiguously
  once there's also a rejected tail; "valid" says what's actually true of
  those records without implying anything about what came after.
- Shared test fixtures (`sample_block`, `sample_transaction`, `account`)
  moved out of `tests/append_log.rs` into `tests/common/mod.rs` once
  `tests/decoder.rs` and `tests/recovery.rs` needed the same block-building
  helpers — duplicating them per file would have meant three copies to
  keep in sync.
- `Block`, `SignedTransaction`, and `UnsignedTransaction` gained `#[derive(Clone)]`
  for Hour 7: `Chain::import_block` needs the block both to hand to
  `ChainState::execute_block` (which consumes it) and, separately, to hand
  to `BlockStore::append_block` (which only borrows it) — cloning once at
  the call site is simpler than reworking `execute_block`'s signature.

### What I learned building this

The two-phase write (fsync body, *then* fsync a separate one-byte marker)
is doing all the work here — without it, "did this record fully land on
disk" isn't answerable from the bytes alone. The recovery scan itself is
almost boring by comparison: decode records in a loop, stop on the first
error. The property worth being paranoid about isn't the happy path, it's
making sure "stop" actually means stop — that nothing downstream of the
first bad record ever gets a chance to look valid on its own. That's also
exactly the gap the Hour 7 integration surfaced: `recover()` validates
records in isolation, so it doesn't yet know if two consecutive *valid*
records actually chain to each other. See "Known gap" in
`docs/storage.md` and the last review question below.

## End-of-day review

**What kind of corruption can storage detect?**

Anything that breaks a single record's internal self-consistency:
structural corruption (bad magic, unsupported version, an insane length
field, too few bytes remaining — including a write cut short mid-record),
a CRC32 mismatch over `height || hash || payload` (catches bit flips
anywhere in that span, including inside the encoded transactions), a
record `hash` field that doesn't match the header hash recomputed from
the decoded payload, and a missing or unset commit marker. All of it goes
through `record::decode_record`, used identically by `load_blocks` (fails
loudly) and `recovery::recover` (stops the scan and truncates).

**What kind of corruption can it not detect yet?**

Two real gaps, not just theoretical ones. First: `recover()` and
`load_blocks()` validate each record *in isolation* — nothing currently
checks that record N+1's `parent_hash` equals record N's `block_hash`, or
that heights are sequential, while scanning the log. A structurally valid,
checksum-correct record in the wrong position would be accepted today.
That check only exists inside `ChainState::execute_block`'s header
validation, and nothing yet replays a whole recovered log through
`ChainState` at startup to invoke it. Second: CRC32 is an accidental-
corruption check, not an adversarial one — a party with write access to
the file can recompute a matching checksum and a self-consistent header
hash, so this format doesn't defend against deliberate tampering by
someone who controls the disk. Both are noted as open work in
`docs/storage.md`.

**What happens after an interrupted write?**

`append_block`'s two-phase write means a crash between the two `fsync`
calls leaves a record on disk with a complete-looking body but no (or a
zero) commit marker byte. On the next `recover()` call, the scan accepts
every record up to that one, then hits `MissingCommitMarker` (or
`Truncated`, if the crash happened even earlier, mid-body) and stops.
`recover()` then calls `File::set_len` to truncate the file back to the
offset immediately after the last fully-accepted record's marker,
permanently discarding the partial tail, and returns a `RecoveryReport`
describing what it found and cut. Nothing is repaired — Day 3 does not
attempt to reconstruct a partially-written block; whoever produced it
would need to re-produce and re-append it.

**What is the exact commit point for a block?**

At the storage layer: the instant the commit marker byte (`0x01`) is
written *and* `sync_data`'d — i.e., after both calls in
`file.write_all(&[COMMIT_MARKER]); file.sync_data();` return `Ok`. Every
byte written before that (magic through crc32, and its own separate fsync)
is necessary but not sufficient; without a synced marker, recovery treats
the record as not-committed and discards it. At the chain layer,
`Chain::import_block` only assigns `self.state = candidate_state` — the
step that actually advances the tip — strictly *after*
`self.store.append_block(&block)` has returned `Ok(())`. So a block
becomes durable the moment its marker is synced, and only becomes
canonical some time after that, never before and never concurrently with
it.
