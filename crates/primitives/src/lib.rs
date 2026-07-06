pub mod amount;
pub mod error;
pub mod hash;
pub mod ids;

pub use amount::Amount;
pub use hash::*;
pub use ids::{AccountId, ValidatorId};

/// A 32-byte public key representation.
pub type PublicKeyBytes = [u8; 32];

/// A 64-byte signature representation.
pub type SignatureBytes = [u8; 64];

/// A monotonically increasing height for blocks.
pub type Nonce = u64;

/// A monotonically increasing counter for account transactions.
pub type BlockHeight = u64;
