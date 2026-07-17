//! Shared block/chain construction for the Hour 4 benchmarks. Deliberately
//! mirrors `tests/property_tests.rs`'s helpers (`genesis_config`,
//! `build_block`) rather than reusing them directly -- benches, tests, and
//! fuzz targets are separate Cargo targets with no shared lib to import
//! from, so this is the benches/-local copy of the same small pattern.
//! Not a target of any measurement itself: every function here runs during
//! a benchmark's *setup*, outside the timed `b.iter` closure.
//!
//! Each `benches/*.rs` file compiles this module as part of its own
//! separate bench binary and only uses a subset of it (`block_execution`
//! never calls `build_chain`, `replay`/`storage_append` never call
//! `build_execution_bench`) -- `dead_code` is expected per-binary, not a
//! sign anything here is actually unused.
#![allow(dead_code)]

use block::block::{Block, BlockHeader};
use block::{GENESIS_PARENT_HASH, merkle_root};
use execution::GenesisConfig;
use primitives::{AccountId, Amount};
use state::chain_state::ChainState;
use transaction::transaction::{SignedTransaction, UnsignedTransaction};

/// Comfortably larger than any amount these benches transfer (1 unit per
/// transaction, plus `BASE_FEE`), so no bench block ever fails on
/// insufficient balance regardless of `tx_count`.
const INITIAL_BALANCE: Amount = 1_000_000_000;
const FEE_COLLECTOR_BYTE: u8 = 250;

pub fn account_id(byte: u8) -> AccountId {
    [byte; 32]
}

fn fee_collector() -> AccountId {
    account_id(FEE_COLLECTOR_BYTE)
}

/// `account_count` funded participant accounts (bytes `1..=account_count`)
/// plus a registered fee collector.
pub fn genesis_config(account_count: u8) -> GenesisConfig {
    let accounts = (1..=account_count)
        .map(|byte| (account_id(byte), INITIAL_BALANCE))
        .chain(std::iter::once((fee_collector(), 0)))
        .collect();
    GenesisConfig::new(accounts, fee_collector())
}

fn signed_transfer(from: AccountId, to: AccountId, amount: Amount, nonce: u64) -> SignedTransaction {
    SignedTransaction::sign(UnsignedTransaction { from, to, amount, nonce })
}

/// `tx_count` one-unit transfers, round-robining over `account_count`
/// senders (each sending to the next account in the ring) so every
/// sender's nonce sequence stays valid without needing one account per
/// transaction. `nonces` carries each account's next nonce across calls,
/// so a caller building multiple blocks can keep passing the same
/// `nonces` slice to get a continuous, valid sequence.
fn round_robin_transfers(account_count: u8, tx_count: usize, nonces: &mut [u64]) -> Vec<SignedTransaction> {
    (0..tx_count)
        .map(|i| {
            let sender_idx = i % account_count as usize;
            let receiver_idx = (sender_idx + 1) % account_count as usize;
            let from = account_id(sender_idx as u8 + 1);
            let to = account_id(receiver_idx as u8 + 1);
            let nonce = nonces[sender_idx];
            nonces[sender_idx] += 1;
            signed_transfer(from, to, 1, nonce)
        })
        .collect()
}

/// Builds a block chaining onto `parent_header` (or genesis, if `None`),
/// computing its transaction root and state commitment from actually
/// applying `transactions` to `parent_state` -- same construction
/// `tests/property_tests.rs::build_block` uses. Requires `transactions` to
/// already be individually valid and nonce-ordered against `parent_state`.
fn build_block(parent_state: &ChainState, parent_header: Option<&BlockHeader>, transactions: Vec<SignedTransaction>) -> Block {
    let root = merkle_root(&transactions.iter().map(|tx| tx.transaction.id()).collect::<Vec<_>>());

    let mut expected_state = parent_state.clone();
    for tx in &transactions {
        expected_state
            .apply_signed_transaction(tx.clone())
            .expect("bench transactions are always individually valid and nonce-ordered");
    }

    let (height, parent_hash) = match parent_header {
        None => (0, GENESIS_PARENT_HASH),
        Some(header) => (header.height + 1, header.block_hash),
    };

    Block {
        header: BlockHeader::new(height, parent_hash, root, expected_state.state_commitment(), [9u8; 32], height),
        transactions,
    }
}

/// A genesis state and a single unexecuted block of `tx_count`
/// transactions chained directly onto it -- the input to the
/// `execute_block` benchmark (`benches/block_execution.rs`).
pub fn build_execution_bench(account_count: u8, tx_count: usize) -> (ChainState, Block) {
    let state = genesis_config(account_count).build_state();
    let mut nonces = vec![0u64; account_count as usize];
    let transactions = round_robin_transfers(account_count, tx_count, &mut nonces);
    let block = build_block(&state, None, transactions);
    (state, block)
}

/// A genesis config and `block_count` blocks (each `txs_per_block`
/// transactions) chained from genesis, every block already confirmed to
/// execute cleanly -- the input to the `replay_chain`
/// (`benches/replay.rs`) and `AppendOnlyBlockStore::append_block`
/// (`benches/storage_append.rs`) benchmarks.
pub fn build_chain(account_count: u8, block_count: usize, txs_per_block: usize) -> (GenesisConfig, Vec<Block>) {
    let genesis = genesis_config(account_count);
    let mut state = genesis.build_state();
    let mut header: Option<BlockHeader> = None;
    let mut nonces = vec![0u64; account_count as usize];
    let mut blocks = Vec::with_capacity(block_count);

    for _ in 0..block_count {
        let transactions = round_robin_transfers(account_count, txs_per_block, &mut nonces);
        let block = build_block(&state, header.as_ref(), transactions);
        state = ChainState::execute_block(&state, block.clone())
            .expect("bench chain blocks must execute cleanly during setup");
        header = Some(block.header);
        blocks.push(block);
    }

    (genesis, blocks)
}
