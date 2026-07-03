use transaction::transaction::UnsignedTransaction;

#[test]
fn unsigned_transaction_serializes_deterministically() {
    let tx = UnsignedTransaction {
        from: [1u8; 32],
        to: [2u8; 32],
        amount: 42,
        nonce: 7,
    };

    let first = tx.to_bytes();
    let second = tx.to_bytes();

    assert_eq!(first, second);
    assert_eq!(
        first,
        [1u8; 32]
            .iter()
            .copied()
            .chain([2u8; 32].iter().copied())
            .chain(42u64.to_be_bytes().iter().copied())
            .chain(7u64.to_be_bytes().iter().copied())
            .collect::<Vec<_>>()
    );
}

#[test]
fn unsigned_transaction_rejects_zero_amount() {
    let tx = UnsignedTransaction {
        from: [1u8; 32],
        to: [2u8; 32],
        amount: 0,
        nonce: 7,
    };

    assert!(!tx.is_valid());
}

#[test]
fn unsigned_transaction_rejects_sender_equals_receiver() {
    let tx = UnsignedTransaction {
        from: [1u8; 32],
        to: [1u8; 32],
        amount: 10,
        nonce: 7,
    };

    assert!(!tx.is_valid());
}
