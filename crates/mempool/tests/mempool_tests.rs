use mempool::pool::{PoolAdmission, TransactionPool};
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

#[test]
fn rejects_duplicate_transaction() {
    let mut pool = TransactionPool::new();
    let state = ChainState::new();
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
    let state = ChainState::new();
    let tx_a = signed_tx([1u8; 32], [2u8; 32], 10, 0);
    let tx_b = signed_tx([1u8; 32], [3u8; 32], 15, 1);

    let result_a = pool.submit_transaction(tx_a, &state);
    let result_b = pool.submit_transaction(tx_b, &state);

    assert_eq!(result_a, PoolAdmission::Accepted);
    assert_eq!(result_b, PoolAdmission::Accepted);
    assert_eq!(pool.len(), 2);
}
