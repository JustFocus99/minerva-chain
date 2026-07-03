use primitives::{Amount, Nonce};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionError {
    SenderMissing,
    ReceiverMissing,
    InvalidSignature,
    InvalidNonce { expected: Nonce, actual: Nonce },
    InsufficientBalance { available: Amount, required: Amount },
    AmountOverflow,
    ZeroAmount,
    SenderEqualsReceiver,
}
