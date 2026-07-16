use state::error::StateError;
use storage::StorageError;
use thiserror::Error;

/// Why `Chain::import_block` rejected a block. Distinguishes a block that
/// failed validation from one that validated but couldn't be made durable
/// -- both leave canonical state untouched (see `docs/storage.md`'s Week 3
/// ordering rule), but callers may want to react differently (e.g. retry
/// storage, but never retry a block that failed validation).
#[derive(Debug, Error)]
pub enum ImportError {
    #[error("block failed validation: {0}")]
    Validation(#[from] StateError),
    #[error("failed to persist block to storage: {0}")]
    Storage(#[from] StorageError),
}
