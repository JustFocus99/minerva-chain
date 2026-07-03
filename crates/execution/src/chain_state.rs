use crate::error::ExecutionError;
use primitives::{AccountId, Amount, BlockHash};
use state::account::Account;
use std::collections::BTreeMap;
use transaction::transaction::SignedTransaction;

#[derive(Default)]
pub struct ChainState {
    accounts: BTreeMap<AccountId, Account>,
}

impl ChainState {
    pub fn new() -> Self {
        Self {
            accounts: BTreeMap::new(),
        }
    }

    pub fn create_account(&mut self, account: Account) {
        self.accounts.insert(account.id, account);
    }

    pub fn get_account(&self, account_id: &AccountId) -> Option<&Account> {
        self.accounts.get(account_id)
    }

    pub fn get_account_mut(&mut self, account_id: &AccountId) -> Option<&mut Account> {
        self.accounts.get_mut(account_id)
    }

    pub fn total_supply(&self) -> Amount {
        self.accounts.values().map(|account| account.balance).sum()
    }

    pub fn state_commitment(&self) -> BlockHash {
        let mut state_bytes = Vec::new();
        for account in self.accounts.values() {
            state_bytes.extend_from_slice(&account.id);
            state_bytes.extend_from_slice(&account.balance.to_be_bytes());
            state_bytes.extend_from_slice(&account.nonce.to_be_bytes());
        }
        crypto::hash::hash_bytes(&state_bytes)
    }

    pub fn apply_signed_transaction(
        &mut self,
        signed_tx: SignedTransaction,
    ) -> Result<(), ExecutionError> {
        let tx = &signed_tx.transaction;

        if !signed_tx.verify() {
            return Err(ExecutionError::InvalidSignature);
        }

        if !self.accounts.contains_key(&tx.from) {
            return Err(ExecutionError::SenderMissing);
        }

        if !self.accounts.contains_key(&tx.to) {
            return Err(ExecutionError::ReceiverMissing);
        }

        if tx.amount == 0 {
            return Err(ExecutionError::ZeroAmount);
        }

        if tx.from == tx.to {
            return Err(ExecutionError::SenderEqualsReceiver);
        }

        let Some(sender_balance) = self.accounts.get(&tx.from).map(|account| account.balance)
        else {
            return Err(ExecutionError::SenderMissing);
        };
        let Some(sender_nonce) = self.accounts.get(&tx.from).map(|account| account.nonce) else {
            return Err(ExecutionError::SenderMissing);
        };

        if sender_nonce != tx.nonce {
            return Err(ExecutionError::InvalidNonce {
                expected: sender_nonce,
                actual: tx.nonce,
            });
        }

        if sender_balance < tx.amount {
            return Err(ExecutionError::InsufficientBalance {
                available: sender_balance,
                required: tx.amount,
            });
        }

        let sender_new_balance = sender_balance - tx.amount;
        let receiver_new_balance = self.accounts.get(&tx.to).unwrap().balance + tx.amount;

        let sender_account = self.accounts.get_mut(&tx.from).unwrap();
        sender_account.balance = sender_new_balance;
        sender_account.increment_nonce();

        let receiver_account = self.accounts.get_mut(&tx.to).unwrap();
        receiver_account.balance = receiver_new_balance;

        Ok(())
    }
}
