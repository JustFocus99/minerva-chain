use block::block::BlockHeader;
use primitives::{BlockHash, BlockHeight, StateCommitment, TransactionRoot, ValidatorId};

fn make_header(height: BlockHeight) -> BlockHeader {
    let parent_hash: BlockHash = [1u8; 32];
    let transaction_root: TransactionRoot = TransactionRoot::new([2u8; 32]);
    let state_commitment: StateCommitment = [3u8; 32];
    let producer: ValidatorId = [4u8; 32];

    BlockHeader::new(
        height,
        parent_hash,
        transaction_root,
        state_commitment,
        producer,
        7,
    )
}

#[test]
fn same_header_gives_same_block_hash() {
    let header1 = make_header(10);
    let header2 = make_header(10);

    assert_eq!(header1.compute_hash(), header2.compute_hash());
}

#[test]
fn changing_height_changes_block_hash() {
    let header1 = make_header(10);
    let header2 = make_header(11);

    assert_ne!(header1.compute_hash(), header2.compute_hash());
}

#[test]
fn changing_parent_hash_changes_block_hash() {
    let mut header1 = make_header(10);
    let mut header2 = make_header(10);

    header1.parent_hash = [5u8; 32];
    header2.parent_hash = [6u8; 32];

    assert_ne!(header1.compute_hash(), header2.compute_hash());
}

#[test]
fn changing_transaction_root_changes_block_hash() {
    let mut header1 = make_header(10);
    let mut header2 = make_header(10);

    header1.transaction_root = TransactionRoot::new([7u8; 32]);
    header2.transaction_root = TransactionRoot::new([8u8; 32]);

    assert_ne!(header1.compute_hash(), header2.compute_hash());
}

#[test]
fn changing_state_commitment_changes_block_hash() {
    let mut header1 = make_header(10);
    let mut header2 = make_header(10);

    header1.state_commitment = [9u8; 32];
    header2.state_commitment = [10u8; 32];

    assert_ne!(header1.compute_hash(), header2.compute_hash());
}
