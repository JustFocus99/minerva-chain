use block::block::{Block, BlockHeader};
use primitives::{BlockHash, TransactionRoot};
use transaction::transaction::{SignedTransaction, UnsignedTransaction};

pub fn account(seed: u8) -> [u8; 32] {
    [seed; 32]
}

pub fn sample_transaction(seed: u8) -> SignedTransaction {
    let unsigned = UnsignedTransaction {
        from: account(seed),
        to: account(seed.wrapping_add(1)),
        amount: 10 + seed as u64,
        nonce: seed as u64,
    };
    SignedTransaction::sign(unsigned)
}

pub fn sample_block(height: u64, parent_hash: BlockHash, seed: u8) -> Block {
    let header = BlockHeader::new(
        height,
        parent_hash,
        TransactionRoot::new([seed; 32]),
        [seed.wrapping_add(2); 32],
        [seed.wrapping_add(3); 32],
        seed as u64,
    );
    Block {
        header,
        transactions: vec![
            sample_transaction(seed),
            sample_transaction(seed.wrapping_add(10)),
        ],
    }
}
