use mempool::error::TransactionPoolError;
use mempool::pool::{PoolAdmission, TransactionPool};
use state::account::Account;
use state::chain_state::ChainState;
use transaction::transaction::{SignedTransaction, UnsignedTransaction};

fn signed_tx(from: [u8; 32], to: [u8; 32], amount: u64, nonce: u64) -> SignedTransaction {
    SignedTransaction::sign(UnsignedTransaction {
        from,
        to,
        amount,
        nonce,
    })
}

fn resign(tx: &SignedTransaction) -> SignedTransaction {
    signed_tx(
        tx.transaction.from,
        tx.transaction.to,
        tx.transaction.amount,
        tx.transaction.nonce,
    )
}

fn state_with_account(id: [u8; 32]) -> ChainState {
    let mut state = ChainState::new();
    state.create_account(Account::new(id, 100));
    state
}

#[test]
fn rejects_duplicate_transaction() {
    let mut pool = TransactionPool::new();
    let state = state_with_account([1u8; 32]);
    let tx_a = signed_tx([1u8; 32], [2u8; 32], 10, 0);

    let first_result = pool.submit_transaction(resign(&tx_a), &state);
    let second_result = pool.submit_transaction(resign(&tx_a), &state);

    assert_eq!(first_result, PoolAdmission::Accepted);
    assert_eq!(second_result, PoolAdmission::Duplicate);
    assert_eq!(pool.len(), 1);
}

#[test]
fn allows_different_transaction_ids() {
    let mut pool = TransactionPool::new();
    let mut state = ChainState::new();
    state.create_account(Account::new([1u8; 32], 100));
    state.create_account(Account::new([2u8; 32], 100));
    let tx_a = signed_tx([1u8; 32], [3u8; 32], 10, 0);
    let tx_b = signed_tx([2u8; 32], [3u8; 32], 15, 0);

    let result_a = pool.submit_transaction(tx_a, &state);
    let result_b = pool.submit_transaction(tx_b, &state);

    assert_eq!(result_a, PoolAdmission::Accepted);
    assert_eq!(result_b, PoolAdmission::Accepted);
    assert_eq!(pool.len(), 2);
}

#[test]
fn rejects_invalid_signature_before_pool_insert() {
    let mut pool = TransactionPool::new();
    let state = state_with_account([1u8; 32]);
    let mut tx = signed_tx([1u8; 32], [2u8; 32], 10, 0);
    tx.signature[0] ^= 0xff;
    let tx_id = tx.transaction.id();

    let result = pool.submit_transaction(tx, &state);

    assert_eq!(
        result,
        PoolAdmission::Rejected(TransactionPoolError::InvalidSignature)
    );
    assert_eq!(pool.len(), 0);
    assert!(!pool.contains_transaction_id(&tx_id));
}

#[test]
fn rejects_duplicate_nonce_for_sender() {
    let mut pool = TransactionPool::new();
    let state = state_with_account([1u8; 32]);
    let tx_a = signed_tx([1u8; 32], [2u8; 32], 10, 0);
    let tx_b = signed_tx([1u8; 32], [3u8; 32], 20, 0);

    let result_a = pool.submit_transaction(tx_a, &state);
    let result_b = pool.submit_transaction(tx_b, &state);

    assert_eq!(result_a, PoolAdmission::Accepted);
    assert_eq!(
        result_b,
        PoolAdmission::Rejected(TransactionPoolError::DuplicateNonceForSender)
    );
    assert_eq!(pool.len(), 1);
}

#[test]
fn accepts_expected_nonce() {
    let mut pool = TransactionPool::new();
    let state = state_with_account([1u8; 32]);
    let tx = signed_tx([1u8; 32], [2u8; 32], 10, 0);

    let result = pool.submit_transaction(tx, &state);

    assert_eq!(result, PoolAdmission::Accepted);
}

#[test]
fn queues_future_nonce() {
    let mut pool = TransactionPool::new();
    let state = state_with_account([1u8; 32]);
    let tx = signed_tx([1u8; 32], [2u8; 32], 10, 1);

    let result = pool.submit_transaction(tx, &state);

    assert_eq!(result, PoolAdmission::QueuedForFutureNonce);
    assert_eq!(pool.len(), 1);
    assert!(pool.ready_transactions(&state).is_empty());
}

#[test]
fn rejects_stale_nonce() {
    let mut pool = TransactionPool::new();
    let mut state = ChainState::new();
    state.create_account({
        let mut account = Account::new([1u8; 32], 100);
        account.increment_nonce();
        account.increment_nonce();
        account
    });
    let tx = signed_tx([1u8; 32], [2u8; 32], 10, 1);

    let size_before = pool.len();
    let result = pool.submit_transaction(tx, &state);
    let size_after = pool.len();

    assert_eq!(
        result,
        PoolAdmission::Rejected(TransactionPoolError::StaleNonce)
    );
    assert_eq!(size_before, size_after);
}

#[test]
fn does_not_execute_nonce_gap() {
    let mut pool = TransactionPool::new();
    let state = state_with_account([1u8; 32]);
    let tx = signed_tx([1u8; 32], [2u8; 32], 10, 2);

    pool.submit_transaction(tx, &state);

    assert!(pool.ready_transactions(&state).is_empty());
}

#[test]
fn orders_transactions_by_sender_and_nonce_deterministically() {
    let mut pool = TransactionPool::new();
    let mut state = ChainState::new();
    state.create_account(Account::new([2u8; 32], 100));
    state.create_account(Account::new([5u8; 32], 100));

    pool.submit_transaction(signed_tx([5u8; 32], [1u8; 32], 10, 1), &state);
    pool.submit_transaction(signed_tx([2u8; 32], [1u8; 32], 10, 2), &state);
    pool.submit_transaction(signed_tx([5u8; 32], [1u8; 32], 10, 0), &state);
    pool.submit_transaction(signed_tx([2u8; 32], [1u8; 32], 10, 0), &state);

    let ordered = pool.ordered_transactions();
    let keys: Vec<([u8; 32], u64)> = ordered
        .iter()
        .map(|tx| (tx.transaction.from, tx.transaction.nonce))
        .collect();

    assert_eq!(
        keys,
        vec![
            ([2u8; 32], 0),
            ([2u8; 32], 2),
            ([5u8; 32], 0),
            ([5u8; 32], 1),
        ]
    );
}

#[test]
fn invalid_signature_does_not_change_pool_size() {
    let mut pool = TransactionPool::new();
    let state = state_with_account([1u8; 32]);
    pool.submit_transaction(signed_tx([1u8; 32], [2u8; 32], 10, 0), &state);
    let size_before = pool.len();

    let mut bad_tx = signed_tx([1u8; 32], [3u8; 32], 5, 1);
    bad_tx.signature[0] ^= 0xff;
    pool.submit_transaction(bad_tx, &state);

    let size_after = pool.len();

    assert_eq!(size_before, size_after);
}
