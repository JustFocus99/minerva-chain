use transaction::transaction::UnsignedTransaction;

#[test]
fn same_unsigned_transaction_produces_same_bytes() {
    let tx = UnsignedTransaction {
        from: [1u8; 32],
        to: [2u8; 32],
        amount: 42,
        nonce: 7,
    };

    assert_eq!(tx.to_bytes(), tx.to_bytes());
}

#[test]
fn same_unsigned_transaction_produces_same_transaction_id() {
    let tx = UnsignedTransaction {
        from: [1u8; 32],
        to: [2u8; 32],
        amount: 42,
        nonce: 7,
    };

    assert_eq!(tx.id(), tx.id());
}

#[test]
fn changing_amount_changes_transaction_id() {
    let first = UnsignedTransaction {
        from: [1u8; 32],
        to: [2u8; 32],
        amount: 42,
        nonce: 7,
    };
    let second = UnsignedTransaction {
        from: [1u8; 32],
        to: [2u8; 32],
        amount: 43,
        nonce: 7,
    };

    assert_ne!(first.id(), second.id());
}

#[test]
fn changing_nonce_changes_transaction_id() {
    let first = UnsignedTransaction {
        from: [1u8; 32],
        to: [2u8; 32],
        amount: 42,
        nonce: 7,
    };
    let second = UnsignedTransaction {
        from: [1u8; 32],
        to: [2u8; 32],
        amount: 42,
        nonce: 8,
    };

    assert_ne!(first.id(), second.id());
}

#[test]
fn zero_amount_is_rejected() {
    let tx = UnsignedTransaction {
        from: [1u8; 32],
        to: [2u8; 32],
        amount: 0,
        nonce: 7,
    };

    assert!(!tx.is_valid());
}

#[test]
fn sender_equals_receiver_is_rejected() {
    let tx = UnsignedTransaction {
        from: [1u8; 32],
        to: [1u8; 32],
        amount: 10,
        nonce: 7,
    };

    assert!(!tx.is_valid());
}
