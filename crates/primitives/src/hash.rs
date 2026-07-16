/// A 32-byte hash-like value used for blocks and commitments.
pub type BlockHash = [u8; 32];

/// Lower-hex-encodes a hash-like byte slice for logging and error messages
/// -- the one place this workspace formats a hash as a string, so every
/// crate that logs a `block_hash`/`parent_hash`/`state_root` field
/// produces the same representation.
pub fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A 32-byte commitment representing state after execution.
pub type StateCommitment = [u8; 32];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TransactionId([u8; 32]);

impl TransactionId {
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TransactionRoot([u8; 32]);

impl TransactionRoot {
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}
