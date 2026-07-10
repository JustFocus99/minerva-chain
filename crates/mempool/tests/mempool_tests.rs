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
    let state = state_with_account([1u8; 32]);
    let tx_a = signed_tx([1u8; 32], [2u8; 32], 10, 0);
    let tx_b = signed_tx([1u8; 32], [3u8; 32], 15, 1);

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
