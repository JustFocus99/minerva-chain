use thiserror::Error;

/// Why `TransactionPool::submit_transaction` rejected a transaction. Used
/// both as the `PoolAdmission::Rejected` payload and as the `error` field
/// on the `transaction_rejected` log event -- see `docs/logging.md`.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TransactionPoolError {
    #[error("duplicate transaction")]
    DuplicateTransaction,
    #[error("invalid signature")]
    InvalidSignature,
    #[error("sender missing")]
    SenderMissing,
    #[error("stale nonce")]
    StaleNonce,
    #[error("duplicate nonce for sender")]
    DuplicateNonceForSender,
    #[error("fee overflow")]
    FeeOverflow,
    #[error("insufficient fee balance")]
    InsufficientFeeBalance,
    #[error("fee collector missing")]
    FeeCollectorMissing,
    #[error("malformed transaction")]
    MalformedTransaction,
}
