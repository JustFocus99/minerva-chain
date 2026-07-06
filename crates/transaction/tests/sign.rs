use transaction::transaction::{SignedTransaction, UnsignedTransaction};

#[test]
fn signed_transaction_contains_signature_and_public_key() {
    let tx = UnsignedTransaction {
        from: [1u8; 32],
        to: [2u8; 32],
        amount: 42,
        nonce: 7,
    };

    let signed = SignedTransaction::sign(tx);

    assert_eq!(signed.public_key, [7u8; 32]);
    assert_eq!(
        signed.signature[..32],
        crypto::hash::hash_bytes(&signed.transaction.to_bytes())
    );
    assert_eq!(
        signed.signature[32..],
        crypto::hash::hash_bytes(&signed.transaction.to_bytes())
    );
}
