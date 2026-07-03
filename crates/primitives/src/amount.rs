use crate::error::PrimitiveError;

/// A numeric value for transfers and balances.
pub type Amount = u64;

/// Create a non-zero amount.
pub fn amount_from_u64(value: u64) -> Result<Amount, PrimitiveError> {
    if value == 0 {
        return Err(PrimitiveError::ZeroAmount);
    }

    Ok(value)
}

/// Add two amounts, rejecting overflow.
pub fn checked_add_amount(lhs: Amount, rhs: Amount) -> Result<Amount, PrimitiveError> {
    lhs.checked_add(rhs).ok_or(PrimitiveError::AmountOverflow)
}

/// Subtract two amounts, rejecting underflow.
pub fn checked_sub_amount(lhs: Amount, rhs: Amount) -> Result<Amount, PrimitiveError> {
    lhs.checked_sub(rhs).ok_or(PrimitiveError::AmountUnderflow)
}
