use block::block::Block;
use primitives::{BlockHash, BlockHeight, StateCommitment};
use state::chain_state::ChainState;
use state::error::StateError;
use thiserror::Error;

use crate::genesis::GenesisConfig;

/// Why `replay_chain` refused to rebuild state from a block sequence. See
/// docs/replay.md, "Replay is fatal, not best-effort": any failure aborts
/// the whole replay, carrying enough context (`index`, `height`) to say
/// which block broke it.
#[derive(Debug, Error)]
#[error("block {index} (height {height}) failed to replay: {source}")]
pub struct ReplayError {
    pub index: usize,
    pub height: BlockHeight,
    #[source]
    pub source: StateError,
}

/// What replaying a block sequence from genesis produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayResult {
    pub final_state: ChainState,
    pub final_state_root: StateCommitment,
    pub tip_hash: Option<BlockHash>,
    pub height: Option<BlockHeight>,
    pub blocks_replayed: usize,
}

/// Rebuilds `ChainState` from `genesis` by running every block in `blocks`
/// (in order) through the same import pipeline a live node uses
/// (`ChainState::execute_block`) — replay does not re-implement or
/// loosen any check (parent hash continuity, block hash, transaction
/// Merkle root, signatures, nonce ordering, fee accounting, and the state
/// root are all already enforced there). The first block that fails
/// aborts the whole replay; there is no partial or best-effort result.
/// See docs/replay.md.
pub fn replay_chain(genesis: GenesisConfig, blocks: &[Block]) -> Result<ReplayResult, ReplayError> {
    let mut state = genesis.build_state();

    for (index, block) in blocks.iter().enumerate() {
        let height = block.header.height;
        state = ChainState::execute_block(&state, block.clone())
            .map_err(|source| ReplayError { index, height, source })?;
    }

    let tip = state.tip().copied();
    Ok(ReplayResult {
        final_state_root: state.state_commitment(),
        tip_hash: tip.map(|header| header.block_hash),
        height: tip.map(|header| header.height),
        blocks_replayed: blocks.len(),
        final_state: state,
    })
}
