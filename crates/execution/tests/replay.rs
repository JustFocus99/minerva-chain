//! Hour 2 — replay engine. `replay_chain` rebuilds `ChainState` from a
//! `GenesisConfig` and an ordered block slice by running every block
//! through the same `ChainState::execute_block` pipeline a live node
//! uses. See docs/replay.md.

use block::block::{Block, BlockHeader};
use block::{GENESIS_PARENT_HASH, merkle_root};
use execution::{GenesisConfig, ReplayError, replay_chain};
use primitives::TransactionRoot;
use state::chain_state::ChainState;
use state::error::StateError;
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
    merkle_root(
        &transactions
            .iter()
            .map(|tx| tx.transaction.id())
            .collect::<Vec<_>>(),
    )
}

/// Builds a block chained onto `parent_header` (or satisfying the genesis
/// convention if `None`), computing the state root that would actually
/// result from executing `transactions` against `parent_state`.
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

fn with_parent_hash(header: &BlockHeader, parent_hash: [u8; 32]) -> BlockHeader {
    BlockHeader::new(
        header.height,
        parent_hash,
        header.transaction_root,
        header.state_commitment,
        header.producer,
        header.slot,
    )
}

fn with_state_commitment(header: &BlockHeader, state_commitment: [u8; 32]) -> BlockHeader {
    BlockHeader::new(
        header.height,
        header.parent_hash,
        header.transaction_root,
        state_commitment,
        header.producer,
        header.slot,
    )
}

/// A two-block valid chain: genesis -> block1 (Alice sends Bob 10) ->
/// block2 (Alice sends Bob 5).
fn two_block_chain() -> (GenesisConfig, Block, Block) {
    let genesis = genesis_config();
    let genesis_state = genesis.build_state();

    let block1 = build_block(&genesis_state, None, vec![signed_tx(ALICE, BOB, 10, 0)]);
    let state_after_1 = ChainState::execute_block(&genesis_state, block1.clone()).unwrap();

    let block2 = build_block(&state_after_1, Some(&block1.header), vec![signed_tx(ALICE, BOB, 5, 1)]);

    (genesis, block1, block2)
}

#[test]
fn replay_reconstructs_same_state() {
    let (genesis, block1, block2) = two_block_chain();

    let result = replay_chain(genesis, &[block1, block2]).expect("replay should succeed");

    assert_eq!(result.blocks_replayed, 2);
    assert_eq!(result.final_state.get_account(&ALICE).unwrap().balance, 83);
    assert_eq!(result.final_state.get_account(&ALICE).unwrap().nonce, 2);
    assert_eq!(result.final_state.get_account(&BOB).unwrap().balance, 65);
    assert_eq!(result.final_state.get_account(&FEE_COLLECTOR).unwrap().balance, 2);
}

#[test]
fn replay_reconstructs_same_state_root() {
    let (genesis, block1, block2) = two_block_chain();
    let expected_root = block2.header.state_commitment;
    let expected_tip_hash = block2.header.block_hash;

    let result = replay_chain(genesis, &[block1, block2]).expect("replay should succeed");

    assert_eq!(result.final_state_root, expected_root);
    assert_eq!(result.final_state.state_commitment(), expected_root);
    assert_eq!(result.tip_hash, Some(expected_tip_hash));
    assert_eq!(result.height, Some(1));
}

#[test]
fn replay_rejects_invalid_historical_state_root() {
    let (genesis, block1, block2) = two_block_chain();
    let mut tampered_block2 = block2;
    tampered_block2.header = with_state_commitment(&tampered_block2.header, [99u8; 32]);

    let err = replay_chain(genesis, &[block1, tampered_block2]).expect_err("expected replay to fail");

    assert_eq!(err.index, 1);
    assert_eq!(err.height, 1);
    assert!(matches!(err.source, StateError::InvalidStateCommitment));
}

#[test]
fn replay_rejects_broken_parent_chain() {
    let (genesis, block1, block2) = two_block_chain();
    let mut broken_block2 = block2;
    // Self-consistent header (block_hash still matches its own fields), but
    // it doesn't chain onto block1 -- a forged or out-of-order record would
    // look exactly like this.
    broken_block2.header = with_parent_hash(&broken_block2.header, [77u8; 32]);

    let err: ReplayError = replay_chain(genesis, &[block1, broken_block2]).expect_err("expected replay to fail");

    assert_eq!(err.index, 1);
    assert!(matches!(err.source, StateError::InvalidParentHash));
}

#[test]
fn replay_with_no_blocks_returns_genesis_state() {
    let genesis = genesis_config();
    let genesis_state = genesis.build_state();

    let result = replay_chain(genesis, &[]).expect("replay of empty log should succeed");

    assert_eq!(result.blocks_replayed, 0);
    assert_eq!(result.tip_hash, None);
    assert_eq!(result.height, None);
    assert_eq!(result.final_state_root, genesis_state.state_commitment());
}
