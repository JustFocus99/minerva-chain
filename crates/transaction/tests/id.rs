use transaction::transaction::UnsignedTransaction;

#[test]
fn transaction_id_is_derived_from_serialized_bytes() {
    let tx = UnsignedTransaction {
        from: [1u8; 32],
        to: [2u8; 32],
        amount: 42,
        nonce: 7,
    };

    let expected = crypto::hash::hash_bytes(&tx.to_bytes());
    assert_eq!(tx.id(), expected);
}
