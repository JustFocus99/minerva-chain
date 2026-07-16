use primitives::BlockHash;
use thiserror::Error;

/// Errors produced by the storage layer. See `docs/storage.md` for the
/// on-disk format and the validity rule each read-side variant corresponds
/// to.
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("block payload is too large to store: {length} bytes exceeds max {max}")]
    PayloadTooLarge { length: usize, max: u32 },

    #[error("record magic mismatch at offset {offset}")]
    MagicMismatch { offset: usize },

    #[error("unsupported record version {version} at offset {offset}")]
    UnsupportedVersion { offset: usize, version: u8 },

    #[error("record length {length} at offset {offset} is not sane (max {max})")]
    InvalidLength {
        offset: usize,
        length: u32,
        max: u32,
    },

    #[error(
        "record at offset {offset} is truncated: expected {expected} more bytes, found {found}"
    )]
    Truncated {
        offset: usize,
        expected: usize,
        found: usize,
    },

    #[error("checksum mismatch for record at offset {offset}")]
    ChecksumMismatch { offset: usize },

    #[error(
        "block hash mismatch for record at offset {offset}: header declares {declared:02x?}, payload hashes to {computed:02x?}"
    )]
    BlockHashMismatch {
        offset: usize,
        declared: BlockHash,
        computed: BlockHash,
    },

    #[error("record at offset {offset} is missing its commit marker")]
    MissingCommitMarker { offset: usize },

    #[error("failed to decode block payload at offset {offset}: {reason}")]
    Decode { offset: usize, reason: String },
}
