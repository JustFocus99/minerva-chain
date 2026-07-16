use block::block::Block;
use state::chain_state::ChainState;
use storage::BlockStore;

use crate::error::ImportError;

/// Ties block validation/execution (`state::ChainState::execute_block`) to
/// durable storage (`storage::BlockStore`) under the Week 3 ordering rule:
/// a block is imported only after validation succeeds *and* storage append
/// succeeds. If either fails, canonical state -- including the tip -- is
/// left exactly as it was. See docs/block-validation.md step 13 and
/// docs/storage.md.
pub struct Chain<S: BlockStore> {
    pub(crate) state: ChainState,
    pub(crate) store: S,
}

impl<S: BlockStore> Chain<S> {
    pub fn new(state: ChainState, store: S) -> Self {
        Self { state, store }
    }

    pub fn state(&self) -> &ChainState {
        &self.state
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    /// Validates and executes `block` against candidate state (state root
    /// and all, via `ChainState::execute_block`), persists it to storage,
    /// and only then promotes candidate state to canonical. Storage never
    /// sees a block that hasn't already cleared every validation gate, and
    /// canonical state -- including the tip -- never advances unless
    /// storage confirms the block is durable.
    pub fn import_block(&mut self, block: Block) -> Result<(), ImportError> {
        let candidate_state = ChainState::execute_block(&self.state, block.clone())?;

        self.store.append_block(&block)?;

        self.state = candidate_state;

        tracing::info!(
            height = block.header.height,
            block_hash = %primitives::to_hex(&block.header.block_hash),
            parent_hash = %primitives::to_hex(&block.header.parent_hash),
            state_root = %primitives::to_hex(&self.state.state_commitment()),
            "block_imported"
        );
        Ok(())
    }
}
