use crate::error::TransactionPoolError;
use primitives::amount::checked_add_amount;
use primitives::{AccountId, BASE_FEE, Nonce, TransactionId};
use state::chain_state::ChainState;
use std::collections::{BTreeMap, BTreeSet};
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
    pub transactions: BTreeMap<AccountId, BTreeMap<Nonce, SignedTransaction>>,
}

impl TransactionPool {
    pub fn new() -> Self {
        Self {
            seen_transaction_ids: BTreeSet::new(),
            transactions: BTreeMap::new(),
        }
    }

    pub fn submit_transaction(
        &mut self,
        tx: SignedTransaction,
        current_state: &ChainState,
    ) -> PoolAdmission {
        let tx_id = tx.transaction.id();
        if self.contains_transaction_id(&tx_id) {
            return PoolAdmission::Duplicate;
        }

        if !tx.transaction.is_valid() {
            return PoolAdmission::Rejected(TransactionPoolError::MalformedTransaction);
        }

        if !tx.verify() {
            return PoolAdmission::Rejected(TransactionPoolError::InvalidSignature);
        }

        let (sender_nonce, sender_balance) = match current_state.get_account(&tx.transaction.from) {
            Some(sender) => (sender.nonce, sender.balance),
            None => return PoolAdmission::Rejected(TransactionPoolError::SenderMissing),
        };

        if tx.transaction.nonce < sender_nonce {
            return PoolAdmission::Rejected(TransactionPoolError::StaleNonce);
        }

        if self
            .transactions
            .get(&tx.transaction.from)
            .is_some_and(|txs_by_nonce| txs_by_nonce.contains_key(&tx.transaction.nonce))
        {
            return PoolAdmission::Rejected(TransactionPoolError::DuplicateNonceForSender);
        }

        // Mirrors state::ChainState::apply_signed_transaction: total_debit is the
        // amount plus the fixed base fee (see docs/fee-model.md). Rejecting here
        // means an unpayable transaction never gets to wait in the pool.
        let Ok(total_debit) = checked_add_amount(tx.transaction.amount, BASE_FEE) else {
            return PoolAdmission::Rejected(TransactionPoolError::FeeOverflow);
        };

        if sender_balance < total_debit {
            return PoolAdmission::Rejected(TransactionPoolError::InsufficientFeeBalance);
        }

        let tx_nonce = tx.transaction.nonce;
        self.transactions
            .entry(tx.transaction.from)
            .or_default()
            .insert(tx_nonce, tx);
        self.seen_transaction_ids.insert(tx_id);

        if tx_nonce > sender_nonce {
            PoolAdmission::QueuedForFutureNonce
        } else {
            PoolAdmission::Accepted
        }
    }

    pub fn contains_transaction_id(&self, tx_id: &TransactionId) -> bool {
        self.seen_transaction_ids.contains(tx_id)
    }

    pub fn len(&self) -> usize {
        self.transactions.values().map(BTreeMap::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn ordered_transactions(&self) -> Vec<&SignedTransaction> {
        self.transactions
            .values()
            .flat_map(BTreeMap::values)
            .collect()
    }

    pub fn ready_transactions(&self, current_state: &ChainState) -> Vec<&SignedTransaction> {
        let mut ready_txs = Vec::new();

        for (account_id, txs_by_nonce) in &self.transactions {
            if let Some(account) = current_state.get_account(account_id) {
                let expected_nonce = account.nonce;
                if let Some(tx) = txs_by_nonce.get(&expected_nonce) {
                    ready_txs.push(tx);
                }
            }
        }

        ready_txs
    }

    pub fn pending_transactions(&self, current_state: &ChainState) -> Vec<&SignedTransaction> {
        let mut pending_txs = Vec::new();

        for (account_id, txs_by_nonce) in &self.transactions {
            if let Some(account) = current_state.get_account(account_id) {
                let expected_nonce = account.nonce;
                for (&nonce, tx) in txs_by_nonce {
                    if nonce > expected_nonce {
                        pending_txs.push(tx);
                    }
                }
            }
        }

        pending_txs
    }
}
