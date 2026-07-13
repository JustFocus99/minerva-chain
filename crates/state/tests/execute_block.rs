use block::block::{Block, BlockHeader};
use block::{GENESIS_PARENT_HASH, merkle_root};
use primitives::TransactionRoot;
use state::account::Account;
use state::chain_state::ChainState;
use state::error::StateError;
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

fn resign(tx: &SignedTransaction) -> SignedTransaction {
    SignedTransaction::sign(UnsignedTransaction {
        from: tx.transaction.from,
        to: tx.transaction.to,
        amount: tx.transaction.amount,
        nonce: tx.transaction.nonce,
    })
}

const FEE_COLLECTOR: [u8; 32] = [7u8; 32];

fn setup_parent_state() -> ChainState {
    let mut state = ChainState::new();
    state.create_account(account([1u8; 32], 100));
    state.create_account(account([2u8; 32], 50));
    state.create_account(account([3u8; 32], 0));
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

/// Builds a block against a parent state that has no history yet (a fresh
/// `setup_parent_state()`), so it must satisfy the genesis convention:
/// height 0, parent_hash == GENESIS_PARENT_HASH. See
/// `docs/block-validation.md`.
fn build_valid_block(parent_state: &ChainState, transactions: Vec<SignedTransaction>) -> Block {
    let transaction_root = transaction_root(&transactions);

    let mut expected_state = parent_state.clone();
    for signed_tx in &transactions {
        expected_state
            .apply_signed_transaction(resign(signed_tx))
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

/// Rebuilds a header with one field swapped, recomputing `block_hash` so the
/// header stays internally self-consistent. Lets a test corrupt exactly the
/// field it's targeting without also (accidentally) tripping the separate
/// `InvalidBlockHash` check first.
fn with_transaction_root(header: &BlockHeader, transaction_root: TransactionRoot) -> BlockHeader {
    BlockHeader::new(
        header.height,
        header.parent_hash,
        transaction_root,
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

/// The rollback pattern: attempt to import a block expected to fail, assert
/// it actually failed with the expected error, and assert `parent` is
/// bit-for-bit unchanged. There's no separate `chain.state()` /
/// `chain.import_block()` split in this codebase -- `ChainState::execute_block`
/// is a pure function that borrows `parent` immutably and returns a new
/// state, so `parent` itself is the thing to compare before/after, playing
/// the role `chain.state()` would in a stateful wrapper.
fn assert_import_rejected(
    parent: &ChainState,
    block: Block,
    matches_expected: impl Fn(&StateError) -> bool,
) {
    let before = parent.clone();
    match ChainState::execute_block(parent, block) {
        Ok(_) => panic!("expected block import to fail"),
        Err(err) => assert!(matches_expected(&err), "unexpected error: {err:?}"),
    }
    assert_eq!(*parent, before);
}

#[test]
fn valid_block_executes_all_transactions() {
    let parent = setup_parent_state();
    let transactions = vec![
        signed_tx([1u8; 32], [2u8; 32], 10, 0),
        signed_tx([1u8; 32], [3u8; 32], 15, 1),
    ];
    let block = build_valid_block(&parent, transactions);

    let result = ChainState::execute_block(&parent, block).unwrap();

    assert_eq!(result.get_account(&[1u8; 32]).unwrap().balance, 73);
    assert_eq!(result.get_account(&[1u8; 32]).unwrap().nonce, 2);
    assert_eq!(result.get_account(&[2u8; 32]).unwrap().balance, 60);
    assert_eq!(result.get_account(&[3u8; 32]).unwrap().balance, 15);
    assert_eq!(result.total_supply(), parent.total_supply());
}

#[test]
fn valid_block_changes_state_commitment() {
    let parent = setup_parent_state();
    let parent_commitment = parent.state_commitment();
    let block = build_valid_block(&parent, vec![signed_tx([1u8; 32], [2u8; 32], 10, 0)]);

    let result = ChainState::execute_block(&parent, block).unwrap();

    assert_ne!(result.state_commitment(), parent_commitment);
}

#[test]
fn block_transaction_root_must_match_transactions() {
    let parent = setup_parent_state();
    let transactions = vec![
        signed_tx([1u8; 32], [2u8; 32], 10, 0),
        signed_tx([1u8; 32], [3u8; 32], 15, 1),
    ];
    let block = build_valid_block(&parent, transactions);

    assert_eq!(
        block.header.transaction_root,
        transaction_root(&block.transactions)
    );

    ChainState::execute_block(&parent, block).unwrap();
}

#[test]
fn block_state_commitment_must_match_result_state() {
    let parent = setup_parent_state();
    let tx = signed_tx([1u8; 32], [2u8; 32], 10, 0);
    let block = build_valid_block(&parent, vec![resign(&tx)]);

    let mut expected_state = parent.clone();
    expected_state.apply_signed_transaction(tx).unwrap();

    assert_eq!(
        block.header.state_commitment,
        expected_state.state_commitment()
    );

    let result = ChainState::execute_block(&parent, block).unwrap();
    assert_eq!(result.state_commitment(), expected_state.state_commitment());
}

#[test]
fn block_with_invalid_signature_fails() {
    let parent = setup_parent_state();
    let mut bad_tx = signed_tx([1u8; 32], [2u8; 32], 10, 0);
    bad_tx.public_key = [9u8; 32];
    let root = merkle_root(&[bad_tx.transaction.id()]);

    let block = Block {
        header: BlockHeader::new(
            0,
            GENESIS_PARENT_HASH,
            root,
            parent.state_commitment(),
            [4u8; 32],
            0,
        ),
        transactions: vec![bad_tx],
    };

    let err = match ChainState::execute_block(&parent, block) {
        Err(err) => err,
        Ok(_) => panic!("expected invalid signature"),
    };
    assert!(matches!(err, StateError::InvalidSignature));
}

#[test]
fn invalid_block_does_not_mutate_parent_state() {
    let parent = setup_parent_state();
    let before = (
        parent.get_account(&[1u8; 32]).unwrap().balance,
        parent.get_account(&[2u8; 32]).unwrap().balance,
        parent.get_account(&[1u8; 32]).unwrap().nonce,
        parent.state_commitment(),
    );

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

    match ChainState::execute_block(&parent, block) {
        Err(_) => {}
        Ok(_) => panic!("expected block execution to fail"),
    }

    assert_eq!(
        (
            parent.get_account(&[1u8; 32]).unwrap().balance,
            parent.get_account(&[2u8; 32]).unwrap().balance,
            parent.get_account(&[1u8; 32]).unwrap().nonce,
            parent.state_commitment(),
        ),
        before
    );
}

#[test]
fn block_with_bad_transaction_root_fails() {
    let parent = setup_parent_state();
    let mut block = build_valid_block(&parent, vec![signed_tx([1u8; 32], [2u8; 32], 10, 0)]);
    block.header = with_transaction_root(&block.header, merkle_root(&[]));

    let err = match ChainState::execute_block(&parent, block) {
        Err(err) => err,
        Ok(_) => panic!("expected invalid transaction root"),
    };
    assert!(matches!(err, StateError::InvalidTransactionRoot));
}

#[test]
fn block_with_bad_state_commitment_fails() {
    let parent = setup_parent_state();
    let mut block = build_valid_block(&parent, vec![signed_tx([1u8; 32], [2u8; 32], 10, 0)]);
    block.header = with_state_commitment(&block.header, [99u8; 32]);

    let err = match ChainState::execute_block(&parent, block) {
        Err(err) => err,
        Ok(_) => panic!("expected invalid state commitment"),
    };
    assert!(matches!(err, StateError::InvalidStateCommitment));
}

#[test]
fn transaction_ordering_affects_root() {
    let tx1 = signed_tx([1u8; 32], [2u8; 32], 10, 0);
    let tx2 = signed_tx([1u8; 32], [3u8; 32], 5, 1);

    let root_forward = transaction_root(&[tx1, tx2]);
    let root_reversed = transaction_root(&[
        signed_tx([1u8; 32], [3u8; 32], 5, 1),
        signed_tx([1u8; 32], [2u8; 32], 10, 0),
    ]);

    assert_ne!(root_forward, root_reversed);

    let parent = setup_parent_state();
    let mut block = build_valid_block(
        &parent,
        vec![
            signed_tx([1u8; 32], [2u8; 32], 10, 0),
            signed_tx([1u8; 32], [3u8; 32], 5, 1),
        ],
    );
    block.header = with_transaction_root(&block.header, root_reversed);

    let err = match ChainState::execute_block(&parent, block) {
        Err(err) => err,
        Ok(_) => panic!("expected invalid transaction root"),
    };
    assert!(matches!(err, StateError::InvalidTransactionRoot));
}

#[test]
fn replaying_same_block_from_same_state_gives_same_state_commitment() {
    let parent = setup_parent_state();
    let transactions = [
        signed_tx([1u8; 32], [2u8; 32], 10, 0),
        signed_tx([1u8; 32], [3u8; 32], 15, 1),
    ];

    let first = ChainState::execute_block(
        &parent,
        build_valid_block(&parent, transactions.iter().map(resign).collect()),
    )
    .unwrap();
    let second = ChainState::execute_block(
        &parent,
        build_valid_block(&parent, transactions.iter().map(resign).collect()),
    )
    .unwrap();

    assert_eq!(first.state_commitment(), second.state_commitment());
    assert_eq!(
        first.get_account(&[1u8; 32]).unwrap(),
        second.get_account(&[1u8; 32]).unwrap()
    );
    assert_eq!(
        first.get_account(&[2u8; 32]).unwrap(),
        second.get_account(&[2u8; 32]).unwrap()
    );
    assert_eq!(
        first.get_account(&[3u8; 32]).unwrap(),
        second.get_account(&[3u8; 32]).unwrap()
    );
}

// --- Day 2, Hour 3: block header validation ---

#[test]
fn rejects_invalid_parent_hash() {
    let parent = setup_parent_state();
    let tx = signed_tx([1u8; 32], [2u8; 32], 10, 0);
    let root = merkle_root(&[tx.transaction.id()]);
    // parent has no history yet (tip is None), so genesis convention
    // requires parent_hash == GENESIS_PARENT_HASH -- this uses something else.
    let block = Block {
        header: BlockHeader::new(0, [7u8; 32], root, parent.state_commitment(), [4u8; 32], 0),
        transactions: vec![tx],
    };

    let err = ChainState::execute_block(&parent, block).unwrap_err();
    assert!(matches!(err, StateError::InvalidParentHash));
}

#[test]
fn rejects_invalid_block_height() {
    let parent = setup_parent_state();
    let tx = signed_tx([1u8; 32], [2u8; 32], 10, 0);
    let root = merkle_root(&[tx.transaction.id()]);
    // Genesis convention requires height == 0 when there is no tip yet.
    let block = Block {
        header: BlockHeader::new(
            5,
            GENESIS_PARENT_HASH,
            root,
            parent.state_commitment(),
            [4u8; 32],
            0,
        ),
        transactions: vec![tx],
    };

    let err = ChainState::execute_block(&parent, block).unwrap_err();
    assert!(matches!(err, StateError::InvalidBlockHeight { .. }));
}

#[test]
fn rejects_invalid_transaction_root() {
    let parent = setup_parent_state();
    let mut block = build_valid_block(&parent, vec![signed_tx([1u8; 32], [2u8; 32], 10, 0)]);
    block.header = with_transaction_root(&block.header, merkle_root(&[]));

    let err = ChainState::execute_block(&parent, block).unwrap_err();
    assert!(matches!(err, StateError::InvalidTransactionRoot));
}

#[test]
fn rejects_invalid_block_hash() {
    let parent = setup_parent_state();
    let tx = signed_tx([1u8; 32], [2u8; 32], 10, 0);
    let root = merkle_root(&[tx.transaction.id()]);
    let mut header = BlockHeader::new(
        0,
        GENESIS_PARENT_HASH,
        root,
        parent.state_commitment(),
        [4u8; 32],
        0,
    );
    // Corrupt the cached hash directly -- it no longer matches a fresh
    // recomputation over the header's other fields.
    header.block_hash = [42u8; 32];

    let block = Block {
        header,
        transactions: vec![tx],
    };

    let err = ChainState::execute_block(&parent, block).unwrap_err();
    assert!(matches!(err, StateError::InvalidBlockHash));
}

// --- Day 2, Hour 4: transaction validation inside block import ---
//
// Block import never trusts the mempool: even if a transaction was already
// checked before it reached the pool, the block executor re-validates
// everything itself. Per-transaction checks (invalid signature, stale
// nonce, insufficient balance, fee overflow, integer overflow) are already
// covered end to end by tests/executor.rs, since execute_block runs every
// transaction through the same apply_signed_transaction. The tests below
// cover what's specific to importing a *block* of transactions: duplicates
// within one block, and replay of a transaction already committed in an
// earlier block.

#[test]
fn rejects_duplicate_transactions_inside_block() {
    let parent = setup_parent_state();
    let tx = signed_tx([1u8; 32], [2u8; 32], 10, 0);
    // Same transaction (same from/to/amount/nonce -> same transaction ID)
    // submitted twice in the same block.
    let transactions = vec![resign(&tx), resign(&tx)];
    let root = transaction_root(&transactions);
    let block = Block {
        header: BlockHeader::new(
            0,
            GENESIS_PARENT_HASH,
            root,
            parent.state_commitment(),
            [4u8; 32],
            0,
        ),
        transactions,
    };

    let err = ChainState::execute_block(&parent, block).unwrap_err();

    assert!(matches!(err, StateError::DuplicateTransactionInBlock));
    assert_eq!(parent.get_account(&[1u8; 32]).unwrap().balance, 100);
}

#[test]
fn rejects_block_with_invalid_signature() {
    let parent = setup_parent_state();
    let mut bad_tx = signed_tx([1u8; 32], [2u8; 32], 10, 0);
    bad_tx.public_key = [9u8; 32];
    let root = merkle_root(&[bad_tx.transaction.id()]);
    let block = Block {
        header: BlockHeader::new(
            0,
            GENESIS_PARENT_HASH,
            root,
            parent.state_commitment(),
            [4u8; 32],
            0,
        ),
        transactions: vec![bad_tx],
    };

    let err = ChainState::execute_block(&parent, block).unwrap_err();

    assert!(matches!(err, StateError::InvalidSignature));
    assert_eq!(parent.get_account(&[1u8; 32]).unwrap().balance, 100);
}

#[test]
fn rejects_block_with_nonce_gap() {
    let parent = setup_parent_state();
    // Alice's current nonce is 0; this transaction skips straight to 2.
    let tx = signed_tx([1u8; 32], [2u8; 32], 10, 2);
    let root = merkle_root(&[tx.transaction.id()]);
    let block = Block {
        header: BlockHeader::new(
            0,
            GENESIS_PARENT_HASH,
            root,
            parent.state_commitment(),
            [4u8; 32],
            0,
        ),
        transactions: vec![tx],
    };

    let err = ChainState::execute_block(&parent, block).unwrap_err();

    assert!(matches!(err, StateError::InvalidNonce { .. }));
    assert_eq!(parent.get_account(&[1u8; 32]).unwrap().nonce, 0);
}

#[test]
fn rejects_block_with_replayed_transaction() {
    let parent = setup_parent_state();
    let tx = signed_tx([1u8; 32], [2u8; 32], 10, 0);
    let tx_id = tx.transaction.id();

    let block_1 = build_valid_block(&parent, vec![resign(&tx)]);
    let state_after_block_1 = ChainState::execute_block(&parent, block_1).unwrap();
    assert!(state_after_block_1.contains_transaction_id(&tx_id));

    // A second block, correctly chained onto the first, tries to replay the
    // exact same transaction that block 1 already committed.
    let tip = *state_after_block_1.tip().unwrap();
    let replay_tx = resign(&tx);
    let root = merkle_root(&[replay_tx.transaction.id()]);
    let block_2 = Block {
        header: BlockHeader::new(
            tip.height + 1,
            tip.block_hash,
            root,
            state_after_block_1.state_commitment(),
            [4u8; 32],
            0,
        ),
        transactions: vec![replay_tx],
    };

    let err = ChainState::execute_block(&state_after_block_1, block_2).unwrap_err();
    assert!(matches!(err, StateError::ReplayedTransaction));
}

// --- Day 2, Hour 5: fee execution atomicity (block level) ---
//
// Per-transaction atomicity (checked arithmetic computed before any
// mutation, fee and transfer moving together or not at all) is exercised
// directly in tests/executor.rs. This is the block-level case: fee overflow
// inside a block must reject the whole block, same as any other
// per-transaction failure.

#[test]
fn fee_overflow_rejects_block() {
    let parent = setup_parent_state();
    let tx = signed_tx([1u8; 32], [2u8; 32], u64::MAX, 0);
    let root = merkle_root(&[tx.transaction.id()]);
    let block = Block {
        header: BlockHeader::new(
            0,
            GENESIS_PARENT_HASH,
            root,
            parent.state_commitment(),
            [4u8; 32],
            0,
        ),
        transactions: vec![tx],
    };

    assert_import_rejected(&parent, block, |e| matches!(e, StateError::Amount(_)));
}

// --- Day 2, Hour 6: state root verification ---

#[test]
fn rejects_invalid_state_root() {
    let parent = setup_parent_state();
    let mut block = build_valid_block(&parent, vec![signed_tx([1u8; 32], [2u8; 32], 10, 0)]);
    block.header = with_state_commitment(&block.header, [99u8; 32]);

    assert_import_rejected(&parent, block, |e| {
        matches!(e, StateError::InvalidStateCommitment)
    });
}

#[test]
fn invalid_state_root_does_not_change_tip() {
    let parent = setup_parent_state();
    let mut block = build_valid_block(&parent, vec![signed_tx([1u8; 32], [2u8; 32], 10, 0)]);
    block.header = with_state_commitment(&block.header, [99u8; 32]);
    let tip_before = parent.tip().copied();

    assert_import_rejected(&parent, block, |e| {
        matches!(e, StateError::InvalidStateCommitment)
    });

    assert_eq!(parent.tip().copied(), tip_before);
}

#[test]
fn invalid_state_root_does_not_change_accounts() {
    let parent = setup_parent_state();
    let mut block = build_valid_block(&parent, vec![signed_tx([1u8; 32], [2u8; 32], 10, 0)]);
    block.header = with_state_commitment(&block.header, [99u8; 32]);
    let account_before = parent.get_account(&[1u8; 32]).cloned();

    assert_import_rejected(&parent, block, |e| {
        matches!(e, StateError::InvalidStateCommitment)
    });

    assert_eq!(parent.get_account(&[1u8; 32]).cloned(), account_before);
}

// --- Day 2, Hour 7: adversarial block import tests ---
//
// Every case below follows the same rollback pattern via
// assert_import_rejected: attempt an invalid import, confirm it failed with
// the expected error, confirm `parent` (the "chain state") is completely
// unchanged. Several of these overlap in substance with the Hour 3/4/5/6
// tests above (same underlying checks, different names/framing) -- kept
// anyway since these exact names were requested.

#[test]
fn invalid_parent_hash() {
    let parent = setup_parent_state();
    let tx = signed_tx([1u8; 32], [2u8; 32], 10, 0);
    let root = merkle_root(&[tx.transaction.id()]);
    let block = Block {
        header: BlockHeader::new(0, [7u8; 32], root, parent.state_commitment(), [4u8; 32], 0),
        transactions: vec![tx],
    };

    assert_import_rejected(&parent, block, |e| {
        matches!(e, StateError::InvalidParentHash)
    });
}

#[test]
fn invalid_state_root() {
    let parent = setup_parent_state();
    let mut block = build_valid_block(&parent, vec![signed_tx([1u8; 32], [2u8; 32], 10, 0)]);
    block.header = with_state_commitment(&block.header, [99u8; 32]);

    assert_import_rejected(&parent, block, |e| {
        matches!(e, StateError::InvalidStateCommitment)
    });
}

#[test]
fn invalid_transaction_root() {
    let parent = setup_parent_state();
    let mut block = build_valid_block(&parent, vec![signed_tx([1u8; 32], [2u8; 32], 10, 0)]);
    block.header = with_transaction_root(&block.header, merkle_root(&[]));

    assert_import_rejected(&parent, block, |e| {
        matches!(e, StateError::InvalidTransactionRoot)
    });
}

#[test]
fn integer_overflow() {
    let parent = setup_parent_state();
    let tx = signed_tx([1u8; 32], [2u8; 32], u64::MAX, 0);
    let root = merkle_root(&[tx.transaction.id()]);
    let block = Block {
        header: BlockHeader::new(
            0,
            GENESIS_PARENT_HASH,
            root,
            parent.state_commitment(),
            [4u8; 32],
            0,
        ),
        transactions: vec![tx],
    };

    assert_import_rejected(&parent, block, |e| matches!(e, StateError::Amount(_)));
}

#[test]
fn duplicate_transactions() {
    let parent = setup_parent_state();
    let tx = signed_tx([1u8; 32], [2u8; 32], 10, 0);
    let transactions = vec![resign(&tx), resign(&tx)];
    let root = transaction_root(&transactions);
    let block = Block {
        header: BlockHeader::new(
            0,
            GENESIS_PARENT_HASH,
            root,
            parent.state_commitment(),
            [4u8; 32],
            0,
        ),
        transactions,
    };

    assert_import_rejected(&parent, block, |e| {
        matches!(e, StateError::DuplicateTransactionInBlock)
    });
}

#[test]
fn repeated_block_import() {
    let parent = setup_parent_state();
    let tx = signed_tx([1u8; 32], [2u8; 32], 10, 0);

    let first_import = build_valid_block(&parent, vec![resign(&tx)]);
    let state_after_first_import = ChainState::execute_block(&parent, first_import).unwrap();

    // Re-import the same block a second time, on top of the state it
    // already produced. Its parent_hash still points at GENESIS, but the
    // chain has already moved past that.
    let repeated = build_valid_block(&parent, vec![resign(&tx)]);
    assert_import_rejected(&state_after_first_import, repeated, |e| {
        matches!(e, StateError::InvalidParentHash)
    });
}

#[test]
fn replay_attack() {
    let parent = setup_parent_state();
    let tx = signed_tx([1u8; 32], [2u8; 32], 10, 0);
    let tx_id = tx.transaction.id();

    let block_1 = build_valid_block(&parent, vec![resign(&tx)]);
    let state_after_block_1 = ChainState::execute_block(&parent, block_1).unwrap();
    assert!(state_after_block_1.contains_transaction_id(&tx_id));

    let tip = *state_after_block_1.tip().unwrap();
    let replay_tx = resign(&tx);
    let root = merkle_root(&[replay_tx.transaction.id()]);
    let block_2 = Block {
        header: BlockHeader::new(
            tip.height + 1,
            tip.block_hash,
            root,
            state_after_block_1.state_commitment(),
            [4u8; 32],
            0,
        ),
        transactions: vec![replay_tx],
    };

    assert_import_rejected(&state_after_block_1, block_2, |e| {
        matches!(e, StateError::ReplayedTransaction)
    });
}

#[test]
fn nonce_gap() {
    let parent = setup_parent_state();
    let tx = signed_tx([1u8; 32], [2u8; 32], 10, 2);
    let root = merkle_root(&[tx.transaction.id()]);
    let block = Block {
        header: BlockHeader::new(
            0,
            GENESIS_PARENT_HASH,
            root,
            parent.state_commitment(),
            [4u8; 32],
            0,
        ),
        transactions: vec![tx],
    };

    assert_import_rejected(&parent, block, |e| {
        matches!(e, StateError::InvalidNonce { .. })
    });
}

#[test]
fn invalid_signature() {
    let parent = setup_parent_state();
    let mut bad_tx = signed_tx([1u8; 32], [2u8; 32], 10, 0);
    bad_tx.public_key = [9u8; 32];
    let root = merkle_root(&[bad_tx.transaction.id()]);
    let block = Block {
        header: BlockHeader::new(
            0,
            GENESIS_PARENT_HASH,
            root,
            parent.state_commitment(),
            [4u8; 32],
            0,
        ),
        transactions: vec![bad_tx],
    };

    assert_import_rejected(&parent, block, |e| {
        matches!(e, StateError::InvalidSignature)
    });
}
