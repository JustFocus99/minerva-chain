#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionPoolError {
    DuplicateTransaction,
    InvalidSignature,
    SenderMissing,
    StaleNonce,
    FeeOverflow,
    MalformedTransaction,
}
