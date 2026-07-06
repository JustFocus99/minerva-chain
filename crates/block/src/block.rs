use primitives::{BlockHash, BlockHeight, StateCommitment, ValidatorId};
use transaction::transaction::SignedTransaction;

pub struct BlockHeader {
    pub height: BlockHeight,
    pub parent_hash: BlockHash,
    pub transaction_root: [u8; 32],
    pub state_commitment: StateCommitment,
    pub producer: ValidatorId,
    pub slot: u64,
}

pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<SignedTransaction>,
}
