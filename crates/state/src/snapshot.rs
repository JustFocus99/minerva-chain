//! A controlled way to produce candidate state during block import.
//!
//! See `docs/block-validation.md`: canonical state must never be mutated
//! during validation, only candidate state — and candidate state is only
//! ever promoted to canonical after every validation check has passed.
//! `StateSnapshot` is that boundary: it clones canonical state once, funnels
//! every mutation through `ChainState`'s own checked methods (so it can't
//! be poked into an invalid state), and only ever hands the result back to
//! the caller explicitly via [`StateSnapshot::into_state`].
//!
//! ## Trade-off
//!
//! Full-state cloning is simple and safe for an educational prototype,
//! but it becomes expensive and hides bad ownership/design choices.
//! A production implementation would use journaling, copy-on-write, database
//! transactions, or account-level write sets.

use crate::chain_state::ChainState;
use crate::error::StateError;
use primitives::BlockHash;
use transaction::transaction::SignedTransaction;

pub struct StateSnapshot {
    candidate: ChainState,
}

impl StateSnapshot {
    /// Clones canonical state into a working candidate. `canonical` is only
    /// ever borrowed here — this cannot mutate it.
    pub fn from_canonical(canonical: &ChainState) -> Self {
        Self {
            candidate: canonical.clone(),
        }
    }

    /// Applies a transaction to the candidate state only. On error, the
    /// candidate is left exactly as `apply_signed_transaction` leaves it
    /// (unmutated for that transaction — see `docs/fee-model.md`), and
    /// canonical state was never in the call path at all.
    pub fn apply_signed_transaction(
        &mut self,
        signed_tx: SignedTransaction,
    ) -> Result<(), StateError> {
        self.candidate.apply_signed_transaction(signed_tx)
    }

    pub fn state_commitment(&self) -> BlockHash {
        self.candidate.state_commitment()
    }

    /// Read-only view of the candidate. There is no `candidate_mut`: every
    /// mutation must go through a checked method like
    /// `apply_signed_transaction` so the snapshot can never be pushed into
    /// an invalid state by a caller reaching in directly.
    pub fn candidate(&self) -> &ChainState {
        &self.candidate
    }

    /// Consumes the snapshot and promotes the candidate to be the new
    /// canonical state. Call this only after every validation check in the
    /// import pipeline has passed. If validation fails instead, simply drop
    /// the snapshot — there is nothing to roll back, since canonical state
    /// was never touched.
    pub fn into_state(self) -> ChainState {
        self.candidate
    }
}
