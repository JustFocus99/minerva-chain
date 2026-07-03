pub struct UnsignedTransaction {
    pub from: AccountId,
    pub to: AccountId,
    pub amount: Amount,
    pub nonce: Nonce,
}

pub struct SignedTransaction {
    pub transaction: UnsignedTransaction,
    pub public_key: PublicKeyBytes,
    pub signature: SignatureBytes,
}