use crate::account::Account;
use crate::error::StateError;
use crate::snapshot::StateSnapshot;
use ::block::block::{Block, BlockHeader, GENESIS_PARENT_HASH};
use block::merkle_root;
use primitives::{
    AccountId, Amount, BASE_FEE, BlockHash, TransactionId,
    amount::{checked_add_amount, checked_sub_amount},
};
use std::collections::{BTreeMap, BTreeSet};
use transaction::transaction::SignedTransaction;

#[derive(Debug, Default, Clone)]
pub struct ChainState {
    accounts: BTreeMap<AccountId, Account>,
    fee_collector: Option<AccountId>,
    /// The header of the last block that was committed via `execute_block`.
    /// `None` means this state has no history yet — the next block executed
    /// against it must satisfy the genesis convention. See
    /// `docs/block-validation.md`.
    tip: Option<BlockHeader>,
    /// Every transaction ID ever committed via `execute_block`, across all
    /// blocks, kept so a transaction can't be replayed into a later block.
    /// Unbounded growth is one of the trade-offs of the full-state-cloning
    /// approach documented in `snapshot.rs` — acceptable for Week 3.
    included_transaction_ids: BTreeSet<TransactionId>,
}

impl ChainState {
    pub fn new() -> Self {
        Self {
            accounts: BTreeMap::new(),
            fee_collector: None,
            tip: None,
            included_transaction_ids: BTreeSet::new(),
        }
    }

    pub fn tip(&self) -> Option<&BlockHeader> {
        self.tip.as_ref()
    }

    pub fn contains_transaction_id(&self, tx_id: &TransactionId) -> bool {
        self.included_transaction_ids.contains(tx_id)
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

    /// Designates the special account that collects transaction fees.
    /// See docs/fee-model.md.
    pub fn set_fee_collector(&mut self, account_id: AccountId) {
        self.fee_collector = Some(account_id);
    }

    pub fn fee_collector(&self) -> Option<AccountId> {
        self.fee_collector
    }

    pub fn fee_collector_account(&self) -> Option<&Account> {
        self.fee_collector.and_then(|id| self.accounts.get(&id))
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

        let Some(sender_account) = self.get_account(&tx.from) else {
            return Err(StateError::SenderMissing);
        };
        let sender_balance = sender_account.balance;
        let sender_nonce = sender_account.nonce;

        let Some(receiver_account) = self.get_account(&tx.to) else {
            return Err(StateError::ReceiverMissing);
        };
        let receiver_balance = receiver_account.balance;

        if sender_nonce != tx.nonce {
            return Err(StateError::InvalidNonce {
                expected: sender_nonce,
                actual: tx.nonce,
            });
        }

        let total_debit = checked_add_amount(tx.amount, BASE_FEE)?;

        if sender_balance < total_debit {
            return Err(StateError::InsufficientBalance {
                available: sender_balance,
                required: total_debit,
            });
        }

        // verify fee collector exists
        let fee_collector_id = self.fee_collector.ok_or(StateError::FeeCollectorMissing)?;
        let fee_collector_balance = self
            .get_account(&fee_collector_id)
            .ok_or(StateError::FeeCollectorMissing)?
            .balance;

        let sender_new_balance = checked_sub_amount(sender_balance, total_debit)?;
        let receiver_new_balance = checked_add_amount(receiver_balance, tx.amount)?;
        let fee_collector_new_balance = checked_add_amount(fee_collector_balance, BASE_FEE)
            .map_err(|_| StateError::FeeOverflow)?;

        let sender_account = self.get_account_mut(&tx.from).unwrap();
        sender_account.balance = sender_new_balance;
        sender_account.increment_nonce();

        let receiver_account = self.get_account_mut(&tx.to).unwrap();
        receiver_account.balance = receiver_new_balance;

        let fee_collector_account = self.get_account_mut(&fee_collector_id).unwrap();
        fee_collector_account.balance = fee_collector_new_balance;

        Ok(())
    }

    /// Validates header fields against the current tip: block hash
    /// self-consistency, parent linkage (genesis convention if there is no
    /// tip yet, otherwise the tip's own hash), and height sequencing. See
    /// `docs/block-validation.md`.
    fn validate_header(header: &BlockHeader, tip: Option<&BlockHeader>) -> Result<(), StateError> {
        if !header.verify_hash() {
            return Err(StateError::InvalidBlockHash);
        }

        match tip {
            None => {
                if header.parent_hash != GENESIS_PARENT_HASH {
                    return Err(StateError::InvalidParentHash);
                }
                if header.height != 0 {
                    return Err(StateError::InvalidBlockHeight {
                        expected: 0,
                        actual: header.height,
                    });
                }
            }
            Some(parent) => {
                if header.parent_hash != parent.block_hash {
                    return Err(StateError::InvalidParentHash);
                }
                let expected_height = parent.height + 1;
                if header.height != expected_height {
                    return Err(StateError::InvalidBlockHeight {
                        expected: expected_height,
                        actual: header.height,
                    });
                }
            }
        }

        Ok(())
    }

    pub fn execute_block(
        parent_state: &ChainState,
        block: Block,
    ) -> Result<ChainState, StateError> {
        Self::validate_header(&block.header, parent_state.tip.as_ref())?;

        let tx_ids: Vec<TransactionId> = block
            .transactions
            .iter()
            .map(|tx| tx.transaction.id())
            .collect();

        let actual_transaction_root = merkle_root(&tx_ids);
        if block.header.transaction_root != actual_transaction_root {
            return Err(StateError::InvalidTransactionRoot);
        }

        let mut seen_in_block = BTreeSet::new();
        for tx_id in &tx_ids {
            if !seen_in_block.insert(*tx_id) {
                return Err(StateError::DuplicateTransactionInBlock);
            }
            if parent_state.included_transaction_ids.contains(tx_id) {
                return Err(StateError::ReplayedTransaction);
            }
        }

        let mut snapshot = StateSnapshot::from_canonical(parent_state);

        for signed_tx in block.transactions {
            snapshot.apply_signed_transaction(signed_tx)?;
        }

        if block.header.state_commitment != snapshot.state_commitment() {
            return Err(StateError::InvalidStateCommitment);
        }

        let mut new_state = snapshot.into_state();
        new_state.included_transaction_ids.extend(tx_ids);
        new_state.tip = Some(block.header);

        Ok(new_state)
    }
}
