#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionPoolError {
    DuplicateTransaction,
    InvalidSignature,
    SenderMissing,
    StaleNonce,
    DuplicateNonceForSender,
    FeeOverflow,
    InsufficientFeeBalance,
    FeeCollectorMissing,
    MalformedTransaction,
}
