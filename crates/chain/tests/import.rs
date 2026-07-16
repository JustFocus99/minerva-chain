//! Hour 7 -- storage/block-import integration. Wires `state::ChainState`
//! (validation + execution) to `storage::AppendOnlyBlockStore` (durability)
//! through `chain::Chain::import_block`, and enforces the Week 3 ordering
//! rule: a block is imported only after validation succeeds *and* storage
//! append succeeds. If storage append fails, neither the canonical state
//! nor the tip may change -- see docs/storage.md and
//! docs/block-validation.md step 13.

use block::block::{Block, BlockHeader};
use block::{GENESIS_PARENT_HASH, merkle_root};
use chain::{Chain, ImportError};
use primitives::TransactionRoot;
use state::account::Account;
use state::chain_state::ChainState;
use state::error::StateError;
use storage::{AppendOnlyBlockStore, BlockStore, RecoveryReport, StorageError};
use transaction::transaction::{SignedTransaction, UnsignedTransaction};

fn account(id: [u8; 32], balance: u64) -> Account {
    Account::new(id, balance)
}

fn signed_tx(from: [u8; 32], to: [u8; 32], amount: u64, nonce: u64) -> SignedTransaction {
    SignedTransaction::sign(UnsignedTransaction {
        from,
        to,
        amount,
        nonce,
    })
}

const FEE_COLLECTOR: [u8; 32] = [7u8; 32];

fn setup_parent_state() -> ChainState {
    let mut state = ChainState::new();
    state.create_account(account([1u8; 32], 100));
    state.create_account(account([2u8; 32], 50));
    state.create_account(account(FEE_COLLECTOR, 0));
    state.set_fee_collector(FEE_COLLECTOR);
    state
}

fn transaction_root(transactions: &[SignedTransaction]) -> TransactionRoot {
    merkle_root(
        &transactions
            .iter()
            .map(|tx| tx.transaction.id())
            .collect::<Vec<_>>(),
    )
}

/// Builds a block that satisfies every validation gate against a fresh
/// `setup_parent_state()` (no history yet, so genesis convention applies).
fn build_valid_block(parent_state: &ChainState, transactions: Vec<SignedTransaction>) -> Block {
    let transaction_root = transaction_root(&transactions);

    let mut expected_state = parent_state.clone();
    for signed_tx in &transactions {
        expected_state
            .apply_signed_transaction(signed_tx.clone())
            .unwrap();
    }

    Block {
        header: BlockHeader::new(
            0,
            GENESIS_PARENT_HASH,
            transaction_root,
            expected_state.state_commitment(),
            [4u8; 32],
            0,
        ),
        transactions,
    }
}

/// A `BlockStore` that always fails to append, so tests can simulate a
/// storage-layer failure (disk full, io error, ...) without needing real
/// OS-level fault injection. Counts calls so tests can also assert storage
/// was (or wasn't) reached at all.
#[derive(Default)]
struct FailingBlockStore {
    append_calls: usize,
}

impl BlockStore for FailingBlockStore {
    fn append_block(&mut self, _block: &Block) -> Result<(), StorageError> {
        self.append_calls += 1;
        Err(StorageError::Io(std::io::Error::other(
            "simulated disk failure",
        )))
    }

    fn load_blocks(&self) -> Result<Vec<Block>, StorageError> {
        Ok(Vec::new())
    }

    fn recover(&mut self) -> Result<RecoveryReport, StorageError> {
        unimplemented!("not exercised by these tests")
    }
}

#[test]
fn imports_valid_block_persists_and_updates_tip() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("blocks.log");
    let store = AppendOnlyBlockStore::open(&path).expect("open store");

    let parent = setup_parent_state();
    let block = build_valid_block(&parent, vec![signed_tx([1u8; 32], [2u8; 32], 10, 0)]);
    let header = block.header;

    let mut chain = Chain::new(parent, store);
    chain.import_block(block).expect("import should succeed");

    assert_eq!(chain.state().tip(), Some(&header));
    assert_eq!(chain.state().get_account(&[1u8; 32]).unwrap().balance, 89);

    let loaded = chain.store().load_blocks().expect("load blocks");
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].header, header);
}

#[test]
fn invalid_block_does_not_reach_storage() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("blocks.log");
    let store = AppendOnlyBlockStore::open(&path).expect("open store");

    let parent = setup_parent_state();
    let mut bad_tx = signed_tx([1u8; 32], [2u8; 32], 10, 0);
    bad_tx.public_key = [9u8; 32];
    let block = Block {
        header: BlockHeader::new(
            0,
            GENESIS_PARENT_HASH,
            merkle_root(&[bad_tx.transaction.id()]),
            parent.state_commitment(),
            [4u8; 32],
            0,
        ),
        transactions: vec![bad_tx],
    };

    let mut chain = Chain::new(parent, store);
    let err = chain
        .import_block(block)
        .expect_err("expected validation failure");

    assert!(matches!(
        err,
        ImportError::Validation(StateError::InvalidSignature)
    ));
    assert!(chain.state().tip().is_none());

    // The block never cleared validation, so it must never have reached
    // the store -- not even a partial or later-truncated write.
    let loaded = chain.store().load_blocks().expect("load blocks");
    assert!(loaded.is_empty());
}

#[test]
fn storage_append_failure_does_not_commit_state() {
    let parent = setup_parent_state();
    let account_before = parent.get_account(&[1u8; 32]).cloned();
    let block = build_valid_block(&parent, vec![signed_tx([1u8; 32], [2u8; 32], 10, 0)]);

    let mut chain = Chain::new(parent, FailingBlockStore::default());
    let err = chain
        .import_block(block)
        .expect_err("expected storage append to fail");

    assert!(matches!(err, ImportError::Storage(_)));
    assert_eq!(chain.store().append_calls, 1);

    // The block validated successfully -- execute_block produced a
    // candidate state -- but storage rejected it, so canonical state must
    // still be exactly what it was before import was attempted.
    assert_eq!(
        chain.state().get_account(&[1u8; 32]).cloned(),
        account_before
    );
}

#[test]
fn storage_append_failure_does_not_update_tip() {
    let parent = setup_parent_state();
    let tip_before = parent.tip().copied();
    let block = build_valid_block(&parent, vec![signed_tx([1u8; 32], [2u8; 32], 10, 0)]);

    let mut chain = Chain::new(parent, FailingBlockStore::default());
    let err = chain
        .import_block(block)
        .expect_err("expected storage append to fail");

    assert!(matches!(err, ImportError::Storage(_)));
    assert_eq!(chain.state().tip().copied(), tip_before);
}
