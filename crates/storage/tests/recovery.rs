//! Hour 5 — interrupted-write recovery. Simulates a crash mid-write by
//! raw-appending a partial record behind the store's back, then checks
//! that recovery accepts the clean prefix, truncates the partial tail, and
//! never lets the partial record leak into `load_blocks`.

mod common;

use std::fs::OpenOptions;
use std::io::Write;

use block::GENESIS_PARENT_HASH;
use storage::record;
use storage::{AppendOnlyBlockStore, BlockStore};

/// Appends raw bytes directly to the log file, bypassing
/// `AppendOnlyBlockStore::append_block` entirely — this is how the tests
/// simulate a write that got interrupted partway through.
fn append_raw(path: &std::path::Path, bytes: &[u8]) {
    let mut file = OpenOptions::new()
        .append(true)
        .open(path)
        .expect("open log file for raw append");
    file.write_all(bytes).expect("raw write");
    file.sync_data().expect("sync raw write");
}

#[test]
fn recovers_after_interrupted_write() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("blocks.log");

    let mut store = AppendOnlyBlockStore::open(&path).expect("open store");

    let block1 = common::sample_block(0, GENESIS_PARENT_HASH, 1);
    store.append_block(&block1).expect("append block 1");

    let block2 = common::sample_block(1, block1.header.block_hash, 2);
    store.append_block(&block2).expect("append block 2");

    let block3 = common::sample_block(2, block2.header.block_hash, 3);
    let mut record3 = record::encode_record_without_marker(&block3).expect("encode block 3");
    record3.push(record::COMMIT_MARKER);
    let half = record3.len() / 2;
    append_raw(&path, &record3[..half]);

    let report = store.recover().expect("recover");

    assert_eq!(report.valid_records, 2);
    assert_eq!(report.last_valid_height, Some(block2.header.height));
    assert_eq!(report.last_valid_hash, Some(block2.header.block_hash));
    assert!(report.truncated());
    assert_eq!(report.truncated_bytes, half as u64);

    let loaded = store.load_blocks().expect("load blocks after recovery");
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].header, block1.header);
    assert_eq!(loaded[1].header, block2.header);
}

#[test]
fn truncates_partial_record() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("blocks.log");

    let mut store = AppendOnlyBlockStore::open(&path).expect("open store");

    let block1 = common::sample_block(0, GENESIS_PARENT_HASH, 1);
    store.append_block(&block1).expect("append block 1");
    let clean_len = std::fs::metadata(&path).expect("stat log file").len();

    let block2 = common::sample_block(1, block1.header.block_hash, 2);
    let mut partial = record::encode_record_without_marker(&block2).expect("encode block 2");
    partial.truncate(partial.len() - 5); // cut mid-record, before crc32/marker even exist
    append_raw(&path, &partial);

    let corrupted_len = std::fs::metadata(&path).expect("stat log file").len();
    assert!(corrupted_len > clean_len);

    store.recover().expect("recover");

    let final_len = std::fs::metadata(&path).expect("stat log file").len();
    assert_eq!(final_len, clean_len);
}

#[test]
fn does_not_import_partial_block() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("blocks.log");

    let mut store = AppendOnlyBlockStore::open(&path).expect("open store");

    let block1 = common::sample_block(0, GENESIS_PARENT_HASH, 1);
    store.append_block(&block1).expect("append block 1");

    let block2 = common::sample_block(1, block1.header.block_hash, 2);
    let mut partial = record::encode_record_without_marker(&block2).expect("encode block 2");
    partial.truncate(partial.len() - 5);
    append_raw(&path, &partial);

    // Before recovery runs, the partial tail must not be silently accepted
    // as data — reading it surfaces an error rather than a mangled block.
    let err = store
        .load_blocks()
        .expect_err("partial tail must not decode as a block");
    assert!(matches!(err, storage::StorageError::Truncated { .. }));

    store.recover().expect("recover");

    let loaded = store.load_blocks().expect("load blocks after recovery");
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].header, block1.header);
}

// --- Hour 6: partially corrupted database detection ---
//
// These simulate disk corruption in the middle of the file (a bit flip on
// an otherwise complete, fully-committed record), not a partial write. The
// record after it is left perfectly well-formed on purpose: recovery must
// still stop at the corrupted one and must not resync past it to pick the
// good-looking record back up. See "Stopping, not skipping" in
// docs/storage.md.

/// Encodes `block` as a complete record (payload + crc32 + commit marker)
/// and flips a byte inside the payload, so the record is structurally
/// whole but its checksum (and therefore the rest of validation) fails.
fn corrupt_record(block: &block::block::Block) -> Vec<u8> {
    let mut record = record::encode_record_without_marker(block).expect("encode block");
    record.push(record::COMMIT_MARKER);
    let payload_start = 49; // FIXED_PREFIX_LEN: magic(4)+version(1)+length(4)+height(8)+hash(32)
    record[payload_start] ^= 0xFF;
    record
}

#[test]
fn stops_at_first_corrupted_record() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("blocks.log");

    let mut store = AppendOnlyBlockStore::open(&path).expect("open store");

    let block1 = common::sample_block(0, GENESIS_PARENT_HASH, 1);
    store.append_block(&block1).expect("append block 1");

    let block2 = common::sample_block(1, block1.header.block_hash, 2);
    append_raw(&path, &corrupt_record(&block2));

    let report = store.recover().expect("recover");

    assert_eq!(report.valid_records, 1);
    assert_eq!(report.last_valid_height, Some(block1.header.height));
    assert_eq!(report.last_valid_hash, Some(block1.header.block_hash));
    assert!(report.rejected_reason.is_some());
    assert!(report.truncated());
}

#[test]
fn does_not_skip_middle_corruption() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("blocks.log");

    let mut store = AppendOnlyBlockStore::open(&path).expect("open store");

    let block1 = common::sample_block(0, GENESIS_PARENT_HASH, 1);
    store.append_block(&block1).expect("append block 1");

    let block2 = common::sample_block(1, block1.header.block_hash, 2);
    append_raw(&path, &corrupt_record(&block2));

    // A perfectly valid-looking record right after the corrupted one. If
    // recovery skipped block 2 and resynced on the next record, it would
    // find and accept this.
    let block3 = common::sample_block(2, block2.header.block_hash, 3);
    let mut record3 = record::encode_record_without_marker(&block3).expect("encode block 3");
    record3.push(record::COMMIT_MARKER);
    append_raw(&path, &record3);

    let report = store.recover().expect("recover");

    assert_eq!(report.valid_records, 1);
    assert_eq!(report.last_valid_height, Some(block1.header.height));
    assert_eq!(report.last_valid_hash, Some(block1.header.block_hash));

    let loaded = store.load_blocks().expect("load blocks after recovery");
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].header, block1.header);
}

#[test]
fn reports_corruption_offset() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("blocks.log");

    let mut store = AppendOnlyBlockStore::open(&path).expect("open store");

    let block1 = common::sample_block(0, GENESIS_PARENT_HASH, 1);
    store.append_block(&block1).expect("append block 1");
    let clean_len = std::fs::metadata(&path).expect("stat log file").len();

    let block2 = common::sample_block(1, block1.header.block_hash, 2);
    let corrupted_record2 = corrupt_record(&block2);
    append_raw(&path, &corrupted_record2);

    let block3 = common::sample_block(2, block2.header.block_hash, 3);
    let mut record3 = record::encode_record_without_marker(&block3).expect("encode block 3");
    record3.push(record::COMMIT_MARKER);
    append_raw(&path, &record3);

    let report = store.recover().expect("recover");

    // The offset scanning stopped at is exactly where the corrupted record
    // begins -- not partway into it, and not somewhere inside block 3's
    // well-formed bytes.
    assert_eq!(report.final_len, clean_len);
    assert_eq!(
        report.original_len,
        clean_len + corrupted_record2.len() as u64 + record3.len() as u64
    );
    // The truncated span covers both the corrupted record and the
    // trailing well-formed one -- block 3 was never trusted on its own.
    assert_eq!(
        report.truncated_bytes,
        corrupted_record2.len() as u64 + record3.len() as u64
    );
}
