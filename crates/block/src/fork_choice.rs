//! Local fork choice: the deterministic rule a node uses to pick a
//! canonical tip when it knows about more than one valid chain of blocks.
//! Not consensus -- see `docs/fork-choice.md`.

use std::collections::BTreeMap;

use primitives::{BlockHash, BlockHeight};

use crate::block::BlockHeader;
use crate::error::ForkChoiceError;

/// A branch's tip, reduced to exactly the two fields `ForkChoice` compares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChainBranch {
    pub tip_hash: BlockHash,
    pub height: BlockHeight,
}

/// The comparison rule itself, factored out of `ForkTree`'s storage so it
/// can be reasoned about on its own. See docs/fork-choice.md's "core rule":
/// greatest height wins; ties go to the lexicographically smallest tip
/// hash. `BlockHash` is `[u8; 32]`, whose `Ord` is already byte-wise
/// lexicographic, so `<` on `tip_hash` is exactly that rule.
pub struct ForkChoice;

impl ForkChoice {
    /// True if `candidate` should replace `current` as the canonical tip.
    pub fn is_better(candidate: &ChainBranch, current: &ChainBranch) -> bool {
        match candidate.height.cmp(&current.height) {
            std::cmp::Ordering::Greater => true,
            std::cmp::Ordering::Less => false,
            std::cmp::Ordering::Equal => candidate.tip_hash < current.tip_hash,
        }
    }
}

/// Whether `ForkTree::insert_block` actually added a new block, or found it
/// already known. Either way the tree is left in a valid state -- an
/// `AlreadyKnown` result means nothing was mutated, not that anything went
/// wrong. See docs/fork-choice.md's "do not import the same block twice".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertOutcome {
    Inserted,
    AlreadyKnown,
}

/// Every block header this node currently knows about, organized by hash
/// and parent hash, plus whichever tip fork choice currently selects as
/// canonical. Does not store transactions or execute anything -- see
/// docs/fork-choice.md's "What fork choice is not".
pub struct ForkTree {
    blocks_by_hash: BTreeMap<BlockHash, BlockHeader>,
    parent_by_hash: BTreeMap<BlockHash, BlockHash>,
    height_by_hash: BTreeMap<BlockHash, BlockHeight>,
    children_by_parent: BTreeMap<BlockHash, Vec<BlockHash>>,
    canonical_tip_hash: BlockHash,
}

impl ForkTree {
    /// Roots the tree at `genesis`. Genesis is trivially known from
    /// construction -- every other block must chain, directly or
    /// transitively, back to it via `parent_hash` before `insert_block`
    /// will accept it.
    pub fn new(genesis: BlockHeader) -> Self {
        let hash = genesis.block_hash;
        let mut blocks_by_hash = BTreeMap::new();
        let mut height_by_hash = BTreeMap::new();
        blocks_by_hash.insert(hash, genesis);
        height_by_hash.insert(hash, genesis.height);

        Self {
            blocks_by_hash,
            parent_by_hash: BTreeMap::new(),
            height_by_hash,
            children_by_parent: BTreeMap::new(),
            canonical_tip_hash: hash,
        }
    }

    /// Records `header`, rejecting it if its declared parent isn't already
    /// known, and re-selects the canonical tip. See docs/fork-choice.md's
    /// `insert_block` steps.
    pub fn insert_block(&mut self, header: BlockHeader) -> Result<InsertOutcome, ForkChoiceError> {
        let hash = header.block_hash;

        if self.blocks_by_hash.contains_key(&hash) {
            return Ok(InsertOutcome::AlreadyKnown);
        }

        if !self.blocks_by_hash.contains_key(&header.parent_hash) {
            return Err(ForkChoiceError::UnknownParent {
                parent_hash: header.parent_hash,
            });
        }

        self.parent_by_hash.insert(hash, header.parent_hash);
        self.height_by_hash.insert(hash, header.height);
        self.children_by_parent
            .entry(header.parent_hash)
            .or_default()
            .push(hash);
        self.blocks_by_hash.insert(hash, header);

        self.canonical_tip_hash = self.choose_tip().tip_hash;
        Ok(InsertOutcome::Inserted)
    }

    /// The currently-selected canonical tip, maintained incrementally on
    /// every `insert_block`.
    pub fn canonical_tip(&self) -> ChainBranch {
        ChainBranch {
            tip_hash: self.canonical_tip_hash,
            height: self.height_by_hash[&self.canonical_tip_hash],
        }
    }

    /// Recomputes the canonical tip from scratch by scanning every block
    /// with no children and applying `ForkChoice::is_better`, independent
    /// of `canonical_tip_hash`'s incrementally-maintained value. A correctly
    /// maintained tree always has `canonical_tip() == choose_tip()`. See
    /// docs/fork-choice.md's `choose_tip`.
    pub fn choose_tip(&self) -> ChainBranch {
        let mut best: Option<ChainBranch> = None;

        for (&hash, &height) in &self.height_by_hash {
            let is_leaf = self
                .children_by_parent
                .get(&hash)
                .is_none_or(|children| children.is_empty());
            if !is_leaf {
                continue;
            }

            let candidate = ChainBranch {
                tip_hash: hash,
                height,
            };
            best = Some(match best {
                None => candidate,
                Some(current) if ForkChoice::is_better(&candidate, &current) => candidate,
                Some(current) => current,
            });
        }

        best.expect("ForkTree always has at least the genesis block")
    }

    pub fn contains_block(&self, hash: &BlockHash) -> bool {
        self.blocks_by_hash.contains_key(hash)
    }

    pub fn parent_of(&self, hash: &BlockHash) -> Option<BlockHash> {
        self.parent_by_hash.get(hash).copied()
    }

    pub fn height_of(&self, hash: &BlockHash) -> Option<BlockHeight> {
        self.height_by_hash.get(hash).copied()
    }
}
