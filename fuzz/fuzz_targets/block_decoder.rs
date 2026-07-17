//! Day 5 Hour 2 -- fuzzes `storage::record::decode_record`, the real
//! decoder `storage::AppendOnlyBlockStore::load_blocks` and
//! `storage::recovery::recover` both run over every byte on disk (see
//! `docs/storage.md`). This is the boundary between untrusted bytes (a
//! corrupted file, a byte-flipped disk, bytes from `minerva-node
//! import-block`) and a trusted in-memory `Block`.
//!
//! Fuzz rule (see `docs/fuzzing.md`):
//!   - arbitrary bytes may return `Err`
//!   - arbitrary bytes must never panic or produce undefined behavior
//!   - a decoded value must not be accepted as valid unless it fully
//!     passed every check `decode_record` performs (magic, version,
//!     length sanity, payload decode, checksum, declared-vs-computed block
//!     hash, commit marker -- see `record.rs`)

#![no_main]

use libfuzzer_sys::fuzz_target;
use storage::record::decode_record;

fuzz_target!(|data: &[u8]| {
    let Ok((block, record_len)) = decode_record(data, 0) else {
        return;
    };

    // `decode_record` must never report consuming more bytes than it was
    // given.
    assert!(record_len <= data.len());

    // `decode_record` only returns `Ok` after comparing the record's
    // declared hash against a hash it recomputed from the decoded header
    // fields (`StorageError::BlockHashMismatch`) -- so a successfully
    // decoded block's own header must always be internally consistent.
    assert!(block.header.verify_hash());
});
