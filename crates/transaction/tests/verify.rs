use transaction::transaction::{SignedTransaction, UnsignedTransaction};

#[test]
fn signed_transaction_verifies_when_signature_is_valid() {
    let tx = UnsignedTransaction {
        from: [1u8; 32],
        to: [2u8; 32],
        amount: 42,
        nonce: 7,
    };

    let signed = SignedTransaction::sign(tx);
    assert!(signed.verify());
}

#[test]
fn signed_transaction_rejects_invalid_signature() {
    let tx = UnsignedTransaction {
        from: [1u8; 32],
        to: [2u8; 32],
        amount: 42,
        nonce: 7,
    };

    let mut signed = SignedTransaction::sign(tx);
    signed.transaction.amount = 99;

    assert!(!signed.verify());
}
