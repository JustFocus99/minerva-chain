use execution::ReplayError;
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

/// Why `Chain::from_storage` (node restart / cold start) failed. Recovery
/// truncating a corrupted or partial tail is *not* a failure here -- that's
/// the expected, survivable case documented in `docs/storage.md` and
/// `docs/replay.md`. This only fires if the storage layer itself errors
/// (e.g. an I/O failure), or if the structurally-clean blocks recovery
/// hands back fail to replay -- which per `docs/replay.md` is fatal, not
/// something to route around.
#[derive(Debug, Error)]
pub enum StartupError {
    #[error("failed to load block log: {0}")]
    Storage(#[from] StorageError),
    #[error("replay failed: {0}")]
    Replay(#[from] ReplayError),
}
