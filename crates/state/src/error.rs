use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum StateError {
    #[error("invalid signature")]
    InvalidSignature,
    #[error("sender missing")]
    SenderMissing,
    #[error("receiver missing")]
    ReceiverMissing,
    #[error("zero amount")]
    ZeroAmount,
    #[error("sender equals receiver")]
    SenderEqualsReceiver,
    #[error("invalid nonce: expected {expected}, got {actual}")]
    InvalidNonce { expected: u64, actual: u64 },
    #[error("insufficient balance: available {available}, required {required}")]
    InsufficientBalance { available: u64, required: u64 },
}
