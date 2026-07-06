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

impl BlockHeader {
    pub fn new(
        height: BlockHeight,
        parent_hash: BlockHash,
        transaction_root: [u8; 32],
        state_commitment: StateCommitment,
        producer: ValidatorId,
        slot: u64,
    ) -> Self {
        Self {
            height,
            parent_hash,
            transaction_root,
            state_commitment,
            producer,
            slot,
        }
    }

    pub fn compute_hash(&self) -> BlockHash {
        // Implementation for computing the block hash
        let mut data = Vec::new();
        data.extend_from_slice(&self.height.to_le_bytes());
        data.extend_from_slice(&self.parent_hash);
        data.extend_from_slice(&self.transaction_root);
        data.extend_from_slice(&self.state_commitment);
        data.extend_from_slice(&self.producer);
        data.extend_from_slice(&self.slot.to_le_bytes());
        crypto::hash::hash_bytes(&data)
    }
}
