use primitives::amount::{amount_from_u64, checked_add_amount, checked_sub_amount};
use primitives::error::PrimitiveError;

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
