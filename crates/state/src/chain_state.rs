use crate::error::StateError;
use ::block::block::Block;
use block::merkle_root;
use primitives::{AccountId, Amount, BlockHash};
use std::collections::BTreeMap;
use transaction::transaction::SignedTransaction;

#[derive(Default, Clone)]
pub struct ChainState {
    accounts: BTreeMap<AccountId, crate::account::Account>,
}

impl ChainState {
    pub fn new() -> Self {
        Self {
            accounts: BTreeMap::new(),
        }
    }

    pub fn create_account(&mut self, account: crate::account::Account) {
        self.accounts.insert(account.id, account);
    }

    pub fn get_account(&self, account_id: &AccountId) -> Option<&crate::account::Account> {
        self.accounts.get(account_id)
    }

    pub fn get_account_mut(
        &mut self,
        account_id: &AccountId,
    ) -> Option<&mut crate::account::Account> {
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
    ) -> Result<(), StateError> {
        let tx = &signed_tx.transaction;

        if !signed_tx.verify() {
            return Err(StateError::InvalidSignature);
        }

        if !self.accounts.contains_key(&tx.from) {
            return Err(StateError::SenderMissing);
        }

        if !self.accounts.contains_key(&tx.to) {
            return Err(StateError::ReceiverMissing);
        }

        if tx.amount == 0 {
            return Err(StateError::ZeroAmount);
        }

        if tx.from == tx.to {
            return Err(StateError::SenderEqualsReceiver);
        }

        let Some(sender_balance) = self.accounts.get(&tx.from).map(|account| account.balance)
        else {
            return Err(StateError::SenderMissing);
        };
        let Some(sender_nonce) = self.accounts.get(&tx.from).map(|account| account.nonce) else {
            return Err(StateError::SenderMissing);
        };

        if sender_nonce != tx.nonce {
            return Err(StateError::InvalidNonce {
                expected: sender_nonce,
                actual: tx.nonce,
            });
        }

        if sender_balance < tx.amount {
            return Err(StateError::InsufficientBalance {
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

    pub fn execute_block(
        parent_state: &ChainState,
        block: Block,
    ) -> Result<ChainState, StateError> {
        let actual_transaction_root = merkle_root(
            &block
                .transactions
                .iter()
                .map(|tx| tx.transaction.id())
                .collect::<Vec<_>>(),
        );
        if block.header.transaction_root != actual_transaction_root {
            return Err(StateError::InvalidTransactionRoot);
        }

        let mut temp_state = parent_state.clone();

        for signed_tx in block.transactions {
            temp_state.apply_signed_transaction(signed_tx)?;
        }

        if block.header.state_commitment != temp_state.state_commitment() {
            return Err(StateError::InvalidStateCommitment);
        }

        Ok(temp_state)
    }
}
