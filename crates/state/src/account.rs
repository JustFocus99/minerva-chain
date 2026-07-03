use crate::error::StateError;
use primitives::{Nonce, amount, ids};

pub struct Account {
    pub id: ids::AccountId,
    pub balance: amount::Amount,
    pub nonce: Nonce,
}

impl Account {
    pub fn new(id: ids::AccountId, balance: amount::Amount) -> Self {
        Self {
            id,
            balance,
            nonce: 0,
        }
    }

    pub fn increment_nonce(&mut self) {
        self.nonce += 1;
    }

    pub fn deposit(&mut self, amount: amount::Amount) -> Result<(), StateError> {
        self.balance = amount::checked_add_amount(self.balance, amount)?;
        Ok(())
    }

    pub fn withdraw(&mut self, amount: amount::Amount) -> Result<(), StateError> {
        self.balance = amount::checked_sub_amount(self.balance, amount)?;
        Ok(())
    }

    pub fn get_balance(&self) -> amount::Amount {
        self.balance
    }

    pub fn get_nonce(&self) -> Nonce {
        self.nonce
    }

    pub fn get_id(&self) -> ids::AccountId {
        self.id
    }
}
