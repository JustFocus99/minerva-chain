use crypto::{
    hash::hash_bytes,
    signature::{sign_message, verify_signature},
};
use primitives::{AccountId, Amount, Nonce, PublicKeyBytes, SignatureBytes, TransactionId};

pub struct UnsignedTransaction {
    pub from: AccountId,
    pub to: AccountId,
    pub amount: Amount,
    pub nonce: Nonce,
}

impl UnsignedTransaction {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.from);
        bytes.extend_from_slice(&self.to);
        bytes.extend_from_slice(&self.amount.to_be_bytes());
        bytes.extend_from_slice(&self.nonce.to_be_bytes());
        bytes
    }

    pub fn id(&self) -> TransactionId {
        hash_bytes(&self.to_bytes())
    }

    pub fn is_valid(&self) -> bool {
        self.amount > 0 && self.from != self.to
    }
}

pub struct SignedTransaction {
    pub transaction: UnsignedTransaction,
    pub public_key: PublicKeyBytes,
    pub signature: SignatureBytes,
}

impl SignedTransaction {
    pub fn sign(transaction: UnsignedTransaction) -> Self {
        let bytes = transaction.to_bytes();
        let (public_key, signature) = sign_message(&bytes);
        Self {
            transaction,
            public_key,
            signature,
        }
    }

    pub fn verify(&self) -> bool {
        verify_signature(
            &self.transaction.to_bytes(),
            self.public_key,
            self.signature,
        )
    }
}
