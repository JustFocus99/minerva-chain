use block::block::{Block, BlockHeader};
use block::merkle_root;
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
            1,
            parent_state.state_commitment(),
            transaction_root,
            expected_state.state_commitment(),
            [4u8; 32],
            0,
        ),
        transactions,
    }
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
            1,
            parent.state_commitment(),
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
            1,
            parent.state_commitment(),
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
    block.header.transaction_root = merkle_root(&[]);

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
    block.header.state_commitment = [99u8; 32];

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
    block.header.transaction_root = root_reversed;

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
