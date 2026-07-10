#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionPoolError {
    DuplicateTransaction,
    InvalidSignature,
    StaleNonce,
    FeeOverflow,
    MalformedTransaction,
}
