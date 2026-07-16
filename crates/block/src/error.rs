use primitives::BlockHash;
use thiserror::Error;

/// Why `ForkTree::insert_block` refused to change the tree. Neither variant
/// mutates the tree -- see `docs/fork-choice.md`'s `insert_block` steps 1
/// and 2.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ForkChoiceError {
    #[error("unknown parent: block declares parent {parent_hash:02x?}, which has not been inserted")]
    UnknownParent { parent_hash: BlockHash },
}
