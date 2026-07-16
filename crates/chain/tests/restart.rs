//! Hour 3 — deterministic replay from storage. `Chain::from_storage` is
//! what a node runs on restart: no in-memory state carries over, so
//! everything here simulates a restart by dropping one `Chain` (closing
//! its file handle) and building a fresh one from the same path. See
//! docs/replay.md and docs/storage.md.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use block::block::{Block, BlockHeader};
use block::GENESIS_PARENT_HASH;
use chain::Chain;
use execution::GenesisConfig;
use primitives::TransactionRoot;
use state::chain_state::ChainState;
use storage::{AppendOnlyBlockStore, BlockStore, record};
use transaction::transaction::{SignedTransaction, UnsignedTransaction};

const ALICE: [u8; 32] = [1u8; 32];
const BOB: [u8; 32] = [2u8; 32];
const FEE_COLLECTOR: [u8; 32] = [7u8; 32];

fn genesis_config() -> GenesisConfig {
    GenesisConfig::new(vec![(ALICE, 100), (BOB, 50), (FEE_COLLECTOR, 0)], FEE_COLLECTOR)
}

fn signed_tx(from: [u8; 32], to: [u8; 32], amount: u64, nonce: u64) -> SignedTransaction {
    SignedTransaction::sign(UnsignedTransaction {
        from,
        to,
        amount,
        nonce,
    })
}

fn transaction_root(transactions: &[SignedTransaction]) -> TransactionRoot {
    block::merkle_root(
        &transactions
            .iter()
            .map(|tx| tx.transaction.id())
            .collect::<Vec<_>>(),
    )
}

fn build_block(
    parent_state: &ChainState,
    parent_header: Option<&BlockHeader>,
    transactions: Vec<SignedTransaction>,
) -> Block {
    let root = transaction_root(&transactions);

    let mut expected_state = parent_state.clone();
    for tx in &transactions {
        expected_state.apply_signed_transaction(tx.clone()).unwrap();
    }

    let (height, parent_hash) = match parent_header {
        None => (0, GENESIS_PARENT_HASH),
        Some(header) => (header.height + 1, header.block_hash),
    };

    Block {
        header: BlockHeader::new(height, parent_hash, root, expected_state.state_commitment(), [4u8; 32], 0),
        transactions,
    }
}

/// Appends raw bytes directly to the log file, bypassing
/// `AppendOnlyBlockStore::append_block` -- how these tests simulate a
/// write that got interrupted, or bytes that got corrupted after the
/// fact, independent of a live `Chain`.
fn append_raw(path: &Path, bytes: &[u8]) {
    let mut file = OpenOptions::new()
        .append(true)
        .open(path)
        .expect("open log file for raw append");
    file.write_all(bytes).expect("raw write");
    file.sync_data().expect("sync raw write");
}

/// Simulates a node's first run: opens a fresh store at `path` and imports
/// two valid, chained blocks through `Chain::import_block`. Returns the
/// two blocks so tests can assert against them after a simulated restart.
fn first_run(path: &Path) -> (Block, Block) {
    let store = AppendOnlyBlockStore::open(path).expect("open store");
    let mut chain = Chain::new(genesis_config().build_state(), store);

    let block1 = build_block(chain.state(), None, vec![signed_tx(ALICE, BOB, 10, 0)]);
    chain.import_block(block1.clone()).expect("import block 1");

    let block2 = build_block(chain.state(), Some(&block1.header), vec![signed_tx(ALICE, BOB, 5, 1)]);
    chain.import_block(block2.clone()).expect("import block 2");

    (block1, block2)
}

#[test]
fn node_restart_recovers_tip() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("blocks.log");
    let (_block1, block2) = first_run(&path);

    let store = AppendOnlyBlockStore::open(&path).expect("reopen store");
    let restarted = Chain::from_storage(genesis_config(), store).expect("restart should succeed");

    assert_eq!(restarted.state().tip(), Some(&block2.header));
}

#[test]
fn node_restart_recovers_accounts() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("blocks.log");
    first_run(&path);

    let store = AppendOnlyBlockStore::open(&path).expect("reopen store");
    let restarted = Chain::from_storage(genesis_config(), store).expect("restart should succeed");

    // Alice: 100 - (10 + fee) - (5 + fee) = 83, two transactions sent.
    assert_eq!(restarted.state().get_account(&ALICE).unwrap().balance, 83);
    assert_eq!(restarted.state().get_account(&ALICE).unwrap().nonce, 2);
    assert_eq!(restarted.state().get_account(&BOB).unwrap().balance, 65);
    assert_eq!(restarted.state().get_account(&FEE_COLLECTOR).unwrap().balance, 2);
}

#[test]
fn node_restart_rejects_corrupted_log() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("blocks.log");
    let (_block1, block2) = first_run(&path);

    // Corrupt bytes appear after block 2: a complete, fully-committed
    // record with a bit flipped in its payload -- not a partial write.
    // The record's own content doesn't matter (it's about to be corrupted
    // and discarded); it just needs to encode cleanly.
    let genesis_state = genesis_config().build_state();
    let block3 = build_block(&genesis_state, Some(&block2.header), vec![signed_tx(BOB, ALICE, 1, 0)]);
    let mut record3 = record::encode_record_without_marker(&block3).expect("encode block 3");
    record3.push(record::COMMIT_MARKER);
    let payload_start = 49; // FIXED_PREFIX_LEN: magic(4)+version(1)+length(4)+height(8)+hash(32)
    record3[payload_start] ^= 0xFF;
    append_raw(&path, &record3);

    let store = AppendOnlyBlockStore::open(&path).expect("reopen store");
    let restarted = Chain::from_storage(genesis_config(), store)
        .expect("restart should succeed by discarding the corrupted tail, not by erroring");

    // The corrupted record (and anything after it) is rejected -- the node
    // comes up with exactly the state the valid prefix produced.
    assert_eq!(restarted.state().tip(), Some(&block2.header));
    assert_eq!(restarted.store().load_blocks().expect("load blocks").len(), 2);
}

#[test]
fn node_restart_after_interrupted_write_recovers_last_good_tip() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("blocks.log");
    let (_block1, block2) = first_run(&path);

    // A write for block 3 that was cut off partway through -- no crc32,
    // no commit marker, exactly what a crash mid-`append_block` leaves
    // behind.
    let genesis_state = genesis_config().build_state();
    let block3 = build_block(&genesis_state, Some(&block2.header), vec![signed_tx(BOB, ALICE, 1, 0)]);
    let mut record3 = record::encode_record_without_marker(&block3).expect("encode block 3");
    record3.push(record::COMMIT_MARKER);
    let half = record3.len() / 2;
    append_raw(&path, &record3[..half]);

    let store = AppendOnlyBlockStore::open(&path).expect("reopen store");
    let restarted = Chain::from_storage(genesis_config(), store).expect("restart should succeed");

    assert_eq!(restarted.state().tip(), Some(&block2.header));
    assert_eq!(restarted.state().get_account(&ALICE).unwrap().balance, 83);
    assert_eq!(restarted.store().load_blocks().expect("load blocks").len(), 2);
}
