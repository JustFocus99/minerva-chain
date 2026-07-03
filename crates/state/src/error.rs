use primitives::error::PrimitiveError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateError {
    Primitive(PrimitiveError),
}

impl From<PrimitiveError> for StateError {
    fn from(value: PrimitiveError) -> Self {
        Self::Primitive(value)
    }
}

