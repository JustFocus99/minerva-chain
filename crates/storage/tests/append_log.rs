mod common;

use block::GENESIS_PARENT_HASH;
use storage::{AppendOnlyBlockStore, BlockStore};

#[test]
fn appends_single_block() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("blocks.log");

    let mut store = AppendOnlyBlockStore::open(&path).expect("open store");
    let block = common::sample_block(0, GENESIS_PARENT_HASH, 1);
    store.append_block(&block).expect("append block");

    let bytes = std::fs::read(&path).expect("read log file");
    assert!(!bytes.is_empty());

    let loaded = store.load_blocks().expect("load blocks");
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].header, block.header);
}

#[test]
fn appends_multiple_blocks() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("blocks.log");

    let mut store = AppendOnlyBlockStore::open(&path).expect("open store");

    let block0 = common::sample_block(0, GENESIS_PARENT_HASH, 1);
    store.append_block(&block0).expect("append block 0");

    let block1 = common::sample_block(1, block0.header.block_hash, 2);
    store.append_block(&block1).expect("append block 1");

    let block2 = common::sample_block(2, block1.header.block_hash, 3);
    store.append_block(&block2).expect("append block 2");

    let loaded = store.load_blocks().expect("load blocks");
    assert_eq!(loaded.len(), 3);
    assert_eq!(loaded[0].header, block0.header);
    assert_eq!(loaded[1].header, block1.header);
    assert_eq!(loaded[2].header, block2.header);
}

#[test]
fn loaded_blocks_match_written_blocks() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("blocks.log");

    let mut store = AppendOnlyBlockStore::open(&path).expect("open store");

    let block0 = common::sample_block(0, GENESIS_PARENT_HASH, 10);
    let block1 = common::sample_block(1, block0.header.block_hash, 20);

    store.append_block(&block0).expect("append block 0");
    store.append_block(&block1).expect("append block 1");

    let loaded = store.load_blocks().expect("load blocks");
    assert_eq!(loaded.len(), 2);

    for (loaded_block, original_block) in loaded.iter().zip([&block0, &block1]) {
        assert_eq!(loaded_block.header, original_block.header);
        assert_eq!(
            loaded_block.transactions.len(),
            original_block.transactions.len()
        );

        for (loaded_tx, original_tx) in loaded_block
            .transactions
            .iter()
            .zip(&original_block.transactions)
        {
            assert_eq!(loaded_tx.transaction.from, original_tx.transaction.from);
            assert_eq!(loaded_tx.transaction.to, original_tx.transaction.to);
            assert_eq!(loaded_tx.transaction.amount, original_tx.transaction.amount);
            assert_eq!(loaded_tx.transaction.nonce, original_tx.transaction.nonce);
            assert_eq!(loaded_tx.public_key, original_tx.public_key);
            assert_eq!(loaded_tx.signature, original_tx.signature);
        }
    }
}
