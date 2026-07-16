use execution::{GenesisConfig, replay_chain};
use storage::BlockStore;

use crate::error::StartupError;
use crate::import::Chain;

impl<S: BlockStore> Chain<S> {
    /// Rebuilds a `Chain` entirely from durable storage — this is what a
    /// node runs on restart, with no in-memory state carried over. Runs
    /// storage-level recovery first (truncates any corrupted or partial
    /// tail, per `docs/storage.md`), loads the now-structurally-clean log,
    /// and replays every block from `genesis` (`execution::replay_chain`,
    /// per `docs/replay.md`). The resulting `Chain`'s state and tip are
    /// exactly what that replay produced — nothing here re-derives or
    /// double-checks it, since `recover()` and `load_blocks()` scan the
    /// same bytes with the same decode logic and therefore always agree on
    /// which records are structurally present.
    pub fn from_storage(genesis: GenesisConfig, mut store: S) -> Result<Self, StartupError> {
        store.recover()?;
        let blocks = store.load_blocks()?;
        let replay_result = replay_chain(genesis, &blocks)?;

        Ok(Self {
            state: replay_result.final_state,
            store,
        })
    }
}
