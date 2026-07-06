use block::merkle_root;
use primitives::TransactionId;

/// Helper to create a TransactionId from a seed value
fn tx_id(seed: u8) -> TransactionId {
    let mut bytes = [0u8; 32];
    bytes[0] = seed;
    TransactionId::new(bytes)
}

#[test]
fn empty_list_produces_stable_root() {
    let root1 = merkle_root(&[]);
    let root2 = merkle_root(&[]);
    assert_eq!(root1, root2, "Empty list should produce stable root");
}

#[test]
fn one_transaction_produces_stable_root() {
    let tx = tx_id(1);
    let root1 = merkle_root(&[tx]);
    let root2 = merkle_root(&[tx]);
    assert_eq!(
        root1, root2,
        "Single transaction should produce stable root"
    );
}

#[test]
fn same_transaction_list_produces_same_root() {
    let txs = [tx_id(1), tx_id(2), tx_id(3)];
    let root1 = merkle_root(&txs);
    let root2 = merkle_root(&txs);
    assert_eq!(
        root1, root2,
        "Same transaction list should produce same root"
    );
}

#[test]
fn changing_one_transaction_changes_root() {
    let txs1 = [tx_id(1), tx_id(2), tx_id(3)];
    let txs2 = [tx_id(1), tx_id(99), tx_id(3)]; // Changed second transaction

    let root1 = merkle_root(&txs1);
    let root2 = merkle_root(&txs2);

    assert_ne!(root1, root2, "Changing one transaction should change root");
}

#[test]
fn transaction_order_changes_root() {
    // CRITICAL TEST: Block execution depends on transaction order
    // The same transactions in different order must produce different roots
    let tx1 = tx_id(1);
    let tx2 = tx_id(2);
    let tx3 = tx_id(3);

    let root_ordered = merkle_root(&[tx1, tx2, tx3]);
    let root_reversed = merkle_root(&[tx3, tx2, tx1]);

    assert_ne!(
        root_ordered, root_reversed,
        "Transaction order MUST change root. [Tx1, Tx2, Tx3] must produce \
         different root from [Tx3, Tx2, Tx1]. Block execution depends on order!"
    );
}

#[test]
fn odd_number_of_transactions_works() {
    let txs = [tx_id(1), tx_id(2), tx_id(3), tx_id(4), tx_id(5)];
    let root = merkle_root(&txs);

    // Should not panic and produce a valid root
    assert_eq!(root.as_bytes().len(), 32, "Root should be 32 bytes");
}

#[test]
fn two_different_lists_do_not_produce_same_root() {
    let txs1 = [tx_id(1), tx_id(2), tx_id(3)];
    let txs2 = [tx_id(4), tx_id(5), tx_id(6)];

    let root1 = merkle_root(&txs1);
    let root2 = merkle_root(&txs2);

    assert_ne!(
        root1, root2,
        "Different transaction lists should produce different roots"
    );
}
