use primitives::{AccountId, Amount};
use state::account::Account;
use state::chain_state::ChainState;

/// The account set and fee collector a chain starts from, before any block
/// has been replayed. `ChainState::execute_block`'s genesis convention
/// (see `docs/block-validation.md`) only constrains the first block's
/// *header* — height `0`, `parent_hash == GENESIS_PARENT_HASH`. It says
/// nothing about starting account balances, because those were never part
/// of the block log at all. `GenesisConfig` is the answer to "where does
/// genesis state come from?" in `docs/replay.md`: a fixed, explicit
/// starting point, not something inferred from the log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenesisConfig {
    pub accounts: Vec<(AccountId, Amount)>,
    pub fee_collector: AccountId,
}

impl GenesisConfig {
    pub fn new(accounts: Vec<(AccountId, Amount)>, fee_collector: AccountId) -> Self {
        Self {
            accounts,
            fee_collector,
        }
    }

    /// Builds the genesis `ChainState`: every configured account created
    /// with its starting balance, then the fee collector registered. The
    /// resulting state has no tip — the first block replayed against it
    /// must satisfy `execute_block`'s genesis convention.
    pub fn build_state(&self) -> ChainState {
        let mut state = ChainState::new();
        for (id, balance) in &self.accounts {
            state.create_account(Account::new(*id, *balance));
        }
        state.set_fee_collector(self.fee_collector);
        state
    }
}
