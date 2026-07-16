pub mod block;
pub mod error;
pub mod fork_choice;
pub mod merkle;

pub use block::GENESIS_PARENT_HASH;
pub use error::ForkChoiceError;
pub use fork_choice::{ChainBranch, ForkChoice, ForkTree, InsertOutcome};
pub use merkle::merkle_root;
