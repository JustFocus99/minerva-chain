use crate::error::TransactionPoolError;
use primitives::TransactionId;
use state::chain_state::ChainState;
use std::collections::BTreeSet;
use transaction::transaction::SignedTransaction;

// which transactions are ready for a block
// which transaction should come first
// how to sort by sender and nonce

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PoolAdmission {
    Accepted,                       // The transaction entered the pool and is ready or eligible
    QueuedForFutureNonce, // The transaction is not bad, but its nonce is too far ahead. Store it, but do not include it in the next block yet.
    Duplicate,            // The same transaction ID already exists in the pool.
    Rejected(TransactionPoolError), // The transaction is invalid and must not be stored.
}

#[derive(Debug, Default)]
pub struct TransactionPool {
    pub seen_transaction_ids: BTreeSet<TransactionId>,
    pub transactions: Vec<SignedTransaction>,
}

impl TransactionPool {
    pub fn new() -> Self {
        Self {
            seen_transaction_ids: BTreeSet::new(),
            transactions: vec![],
        }
    }

    pub fn submit_transaction(
        &mut self,
        tx: SignedTransaction,
        _current_state: &ChainState,
    ) -> PoolAdmission {
        let tx_id = tx.transaction.id();
        if self.contains_transaction_id(&tx_id) {
            return PoolAdmission::Duplicate;
        }

        self.transactions.push(tx);
        self.seen_transaction_ids.insert(tx_id);

        PoolAdmission::Accepted
    }

    pub fn contains_transaction_id(&self, tx_id: &TransactionId) -> bool {
        self.seen_transaction_ids.contains(tx_id)
    }

    pub fn len(&self) -> usize {
        self.transactions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.transactions.is_empty()
    }
}
