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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amount_addition_overflow_is_rejected() {
        let lhs = amount_from_u64(u64::MAX).unwrap();
        let rhs = amount_from_u64(1).unwrap();
        let err = checked_add_amount(lhs, rhs).unwrap_err();
        assert_eq!(err, PrimitiveError::AmountOverflow);
    }

    #[test]
    fn amount_subtraction_underflow_is_rejected() {
        let lhs = amount_from_u64(1).unwrap();
        let rhs = amount_from_u64(2).unwrap();
        let err = checked_sub_amount(lhs, rhs).unwrap_err();
        assert_eq!(err, PrimitiveError::AmountUnderflow);
    }

    #[test]
    fn zero_amount_is_rejected_where_required() {
        let err = amount_from_u64(0).unwrap_err();
        assert_eq!(err, PrimitiveError::ZeroAmount);
    }
}
