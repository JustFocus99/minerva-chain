use transaction::transaction::{SignedTransaction, UnsignedTransaction};

#[test]
fn valid_signature_verifies() {
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
fn wrong_public_key_fails_verification() {
    let tx = UnsignedTransaction {
        from: [1u8; 32],
        to: [2u8; 32],
        amount: 42,
        nonce: 7,
    };

    let mut signed = SignedTransaction::sign(tx);
    signed.public_key = [9u8; 32];

    assert!(!signed.verify());
}

#[test]
fn modified_transaction_fails_verification() {
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

#[test]
fn debug_output_does_not_expose_private_key() {
    let tx = UnsignedTransaction {
        from: [1u8; 32],
        to: [2u8; 32],
        amount: 42,
        nonce: 7,
    };

    let signed = SignedTransaction::sign(tx);
    let debug = format!("{signed:?}");

    assert!(!debug.contains("private"));
    assert!(!debug.contains("secret"));
}
