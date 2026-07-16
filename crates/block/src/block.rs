use primitives::{BlockHash, BlockHeight, StateCommitment, TransactionRoot, ValidatorId};
use transaction::transaction::SignedTransaction;

/// The `parent_hash` a genesis header (the first block of a chain) must
/// declare. There is no real parent block to point to, so this fixed,
/// all-zero convention stands in for "no parent." See
/// `docs/block-validation.md`.
pub const GENESIS_PARENT_HASH: BlockHash = [0u8; 32];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockHeader {
    pub height: BlockHeight,
    pub parent_hash: BlockHash,
    pub transaction_root: TransactionRoot,
    pub state_commitment: StateCommitment,
    pub producer: ValidatorId,
    pub slot: u64,
    /// The hash of this header's other fields, computed once at
    /// construction time by [`BlockHeader::compute_hash`] and cached here.
    /// Validation re-derives the hash and compares it against this field to
    /// catch a header whose fields were tampered with after hashing (see
    /// `docs/block-validation.md`'s "block hash matches header bytes" step).
    /// It is deliberately excluded from its own preimage in
    /// `compute_hash` — hashing it would be circular.
    pub block_hash: BlockHash,
}

#[derive(Debug)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<SignedTransaction>,
}

impl BlockHeader {
    pub fn new(
        height: BlockHeight,
        parent_hash: BlockHash,
        transaction_root: TransactionRoot,
        state_commitment: StateCommitment,
        producer: ValidatorId,
        slot: u64,
    ) -> Self {
        let mut header = Self {
            height,
            parent_hash,
            transaction_root,
            state_commitment,
            producer,
            slot,
            block_hash: [0u8; 32],
        };
        header.block_hash = header.compute_hash();
        header
    }

    pub fn compute_hash(&self) -> BlockHash {
        // Implementation for computing the block hash
        let mut data = Vec::new();
        data.extend_from_slice(&self.height.to_le_bytes());
        data.extend_from_slice(&self.parent_hash);
        data.extend_from_slice(self.transaction_root.as_bytes());
        data.extend_from_slice(&self.state_commitment);
        data.extend_from_slice(&self.producer);
        data.extend_from_slice(&self.slot.to_le_bytes());
        crypto::hash::hash_bytes(&data)
    }

    /// True if `block_hash` still matches a fresh recomputation over the
    /// other fields, i.e. nothing has mutated the header since it was
    /// constructed.
    pub fn verify_hash(&self) -> bool {
        self.block_hash == self.compute_hash()
    }
}
