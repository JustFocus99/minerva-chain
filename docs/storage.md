# Storage

## Purpose

This document defines the on-disk format for persisted blocks in
minerva-chain, and the recovery procedure that runs against it after an
interruption or corruption. It is a design document for Day 3 — the storage
engine should be built to match this format, not the other way around.

This closes the gap `docs/block-validation.md` left open at step 13
("persist block only after validation succeeds") and in its implementation
status ("there is no durable storage layer; everything is in-memory
`ChainState`"). Nothing here changes the import pipeline itself — a block
still only reaches storage after every validation step in
`docs/block-validation.md` has passed and it has been committed as
canonical.

## Non-goals

This is a simple append-only log, not a production database. It does not
provide:

- concurrent writers
- indexing beyond sequential scan (no random access by height or hash)
- compaction, compression, or space reclamation
- multi-file segmentation
- fork handling — the log assumes a single linear canonical chain, matching
  `docs/block-validation.md`'s non-goals

These may be revisited in a later milestone.

## The core rule

```text
A block is durable only if it is stored as a complete, verifiable record.

The log is append-only: existing bytes are never rewritten in place.
A record is either fully present and valid, or it is not there at all.
There is no state where a reader can observe a half-written record as data.
```

Corruption or a partial write must be *detected*, never silently absorbed.
A storage layer that quietly accepts a truncated or bit-flipped record is
worse than one that refuses to start — it hides damage instead of surfacing
it.

## On-disk format

The log is a flat file: a sequence of records, one per block, in commit
order. There is no separate index file for Day 3 — recovery always scans
from the beginning.

### Record layout

```text
[magic][version][length][height][hash][payload][crc32][commit marker]
```

| Field         | Size (bytes) | Description                                                        |
|---------------|-------------:|----------------------------------------------------------------------|
| magic         | 4            | Fixed byte sequence identifying this as a minerva-chain block record |
| version       | 1            | Format version. Day 3 defines version `1`.                          |
| length        | 4            | `u32`, little-endian. Byte length of `payload` only.                 |
| height        | 8            | `u64`, little-endian. Matches `BlockHeader::height`.                 |
| hash          | 32           | `BlockHeader::block_hash` — the hash the payload must reproduce.     |
| payload       | `length`     | Encoded `Block` (header + transactions). Encoding is out of scope for this doc — see "Open questions" below. |
| crc32         | 4            | CRC32 checksum over `height \|\| hash \|\| payload` (magic/version/length/commit marker excluded). |
| commit marker | 1            | `0x01` if this record was fully written and fsynced; absent (or the byte is unwritten/`0x00`) otherwise. |

Fixed-size prefix (magic + version + length + height + hash) is 49 bytes.
Fixed-size suffix (crc32 + commit marker) is 5 bytes. Total record size is
`49 + length + 5`.

`height` and `hash` are duplicated from the payload's own header fields
deliberately: a reader must be able to identify a record (for logging,
recovery diagnostics, and the "block hash matches payload" check below)
without first fully decoding the payload.

### Magic and version

```text
magic   = 4 bytes, fixed: 0x4D 0x49 0x4E 0x43   ("MINC")
version = 1 byte, currently 1
```

A reader that encounters a magic mismatch stops immediately — those bytes
are not a record at all, and scanning must not guess at a resync point (see
"Recovery" below). A reader that encounters an unsupported version rejects
the record even if every other field looks well-formed; Day 3 only
implements version `1`.

### The commit marker

The commit marker is the last byte written for a record, and it is the
field that turns "bytes are on disk" into "this record is committed."
Writers must:

1. Write magic, version, length, height, hash, payload, and crc32.
2. `fsync` (or equivalent) so those bytes are durable.
3. Write the commit marker byte.
4. `fsync` again.

If a process is interrupted between step 1 and step 3, the record on disk
has a valid-looking prefix but no commit marker — exactly the "partial
record" case recovery must detect and truncate away, not replay.

## Validity rules

A record is valid only if **all** of the following hold, checked in order:

```text
1. magic matches the expected 4-byte constant
2. version is supported (currently: version == 1)
3. length is sane (0 < length <= MAX_PAYLOAD_LEN, and enough bytes
   remain in the file to actually contain length + 5 more bytes)
4. payload decodes successfully into a Block
5. checksum matches: crc32(height || hash || payload) == the stored crc32
6. block hash matches payload: hashing the decoded block's header
   reproduces the record's hash field (BlockHeader::verify_hash(), see
   crates/block/src/block.rs)
7. commit marker is present and set to 0x01
```

Any single failure makes the whole record invalid — there is no partial
credit for "checksum passed but commit marker didn't" or similar. Steps are
checked in this order so that cheap structural checks (magic, version,
length) reject obviously-wrong data before paying for decode and hashing.

## Recovery

Recovery runs once, at startup, before any new block is appended:

```text
Scan from the beginning of the file.
For each record, in order:
    Accept it if it passes every validity rule above.
    Stop at the first record that is partial or fails any validity rule.
Truncate the file to the byte offset immediately after the last
    accepted record's commit marker.
Replay accepted blocks, in order, to rebuild in-memory ChainState
    (re-running the same import pipeline as docs/block-validation.md,
    since a stored block is not re-trusted just because it's on disk).
```

Truncation happens even if the file's on-disk length is unchanged from a
previous run — a truncate to the same offset is a no-op, but a truncate
that shortens the file is what actually discards a partial tail write.

### Stopping, not skipping

The rule that matters most: **corruption anywhere in the file stops the
scan at that point.** Recovery never skips a bad record and continues
reading records after it, even if a later record looks perfectly valid.

```text
Good reason: a bad record in the middle usually means the write process
was interrupted mid-record, and subsequent bytes may belong to that same
interrupted write, may be leftover data from a previous file that used to
occupy this disk region, or may be otherwise unrelated. Records after the
first bad one cannot be trusted to represent real chain history in the
right order, even if they individually pass validation.
```

A file with N valid records, one corrupt record, and what looks like more
valid records after it is treated as having exactly N valid records. The
tail — corrupt record onward — is truncated away entirely, not selectively
kept.

This is the storage-layer version of the same discipline
`docs/block-validation.md` applies to a single block: a block either
clears every gate or none of its effects apply. Here it's applied to the
log as a whole — the log either accepts a prefix of fully-valid records, or
it doesn't extend that prefix at all.

### Truncation is destructive and logged

Truncating the file discards bytes permanently. Recovery must log (at
minimum): the byte offset scanning stopped at, the height/hash of the last
accepted record (if any), and which validity rule the first rejected record
failed. This is diagnostic information for a human, not something recovery
acts on automatically beyond truncating — Day 3 does not attempt to repair
or re-request a corrupted record.

## Interrupted-write scenarios this format must survive

| Scenario | On-disk result | Recovery behavior |
|---|---|---|
| Crash before any bytes of a new record are written | Previous record's commit marker is the last byte on disk | Scan accepts everything up to and including it; no truncation needed |
| Crash after prefix (magic..payload) written, before crc32 | Trailing bytes present but incomplete | Length check or read-past-EOF fails; record rejected; file truncated to prior record's end |
| Crash after crc32 written, before commit marker | Record looks complete except the marker byte | Commit-marker check fails; record rejected; file truncated to prior record's end |
| Bit flip in payload (disk corruption, not a partial write) | Record is fully present but checksum or hash-match fails | Checksum/hash-match check fails; record rejected; file truncated to prior record's end — even though the record is "complete" |
| Bit flip in an already-accepted, already-replayed record from a previous run | Same as above | Same as above — validity is re-checked from scratch on every recovery run; nothing is trusted just because it was accepted last time |

The bit-flip cases are why validity is a content check (checksum, hash
match), not just a structural one (magic/version/length present). A record
can be structurally complete and still be corrupted data.

## Determinism

Replaying the same accepted prefix of the log must always rebuild the same
`ChainState`, for the same reason `docs/architecture.md`'s determinism
requirements and `docs/invariants.md`'s replay invariant already require:
recovery is just block import (`docs/block-validation.md`'s pipeline) run
against blocks whose bytes happen to come from disk instead of from a live
producer. It does not get its own, looser set of rules.

## Open questions / follow-up work

- **Payload encoding.** This doc defines the record framing around a
  payload but does not specify how `Block` (header + `Vec<SignedTransaction>`)
  is encoded into `payload` bytes. `docs/block-validation.md` step 1
  ("decode block") has the same open gap — there is no wire format for
  `Block` in this codebase yet. Resolving that is a prerequisite for
  implementing `payload` encode/decode here.
- **`MAX_PAYLOAD_LEN`.** A concrete sanity bound for `length` needs a
  number; left unspecified pending real block-size expectations.
- **CRC32 vs. a stronger hash.** CRC32 is a cheap check against accidental
  corruption (bit flips, truncation), not an adversarial one — a party who
  can rewrite arbitrary bytes on disk can also recompute a matching CRC32.
  `BlockHeader::block_hash` (step 6 of validity) is the field actually
  carrying cryptographic weight here; CRC32 exists purely as a fast
  first-pass integrity check before paying for decode + hash
  verification.
