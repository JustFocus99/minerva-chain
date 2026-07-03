pub mod amount;
pub mod error;
pub mod hash;
pub mod ids;

pub use amount::Amount;
pub use hash::{BlockHash, StateCommitment};
pub use ids::{AccountId, TransactionId, ValidatorId};

/// A monotonically increasing height for blocks.
pub type Nonce = u64;

/// A monotonically increasing counter for account transactions.
pub type BlockHeight = u64;
