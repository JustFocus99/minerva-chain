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
    let err = store.load_blocks().expect_err("partial tail must not decode as a block");
    assert!(matches!(err, storage::StorageError::Truncated { .. }));

    store.recover().expect("recover");

    let loaded = store.load_blocks().expect("load blocks after recovery");
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].header, block1.header);
}
