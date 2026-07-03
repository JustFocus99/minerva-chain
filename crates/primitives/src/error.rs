#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrimitiveError {
    InvalidLength { expected: usize, actual: usize },
    AmountOverflow,
    AmountUnderflow,
    ZeroAmount,
}
