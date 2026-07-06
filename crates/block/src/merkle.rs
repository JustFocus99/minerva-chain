use crypto::hash::hash_bytes;
use primitives::{TransactionId, TransactionRoot};

pub fn merkle_root(transaction_ids: &[TransactionId]) -> TransactionRoot {
    // Implementation for calculating Merkle root
    if transaction_ids.is_empty() {
        return TransactionRoot::new(hash_bytes(b"minerva-empty-transaction-root")); // Return a default value for empty input
    }

    let mut level: Vec<[u8; 32]> = transaction_ids
        .iter()
        .map(|tx_id| *tx_id.as_bytes())
        .collect();

    while level.len() > 1 {
        let mut next_level = Vec::new();

        for i in (0..level.len()).step_by(2) {
            let left = level[i];
            let right = if i + 1 < level.len() {
                level[i + 1]
            } else {
                left // Duplicate the last element if odd number of elements
            };
            let combined = [left, right].concat();
            next_level.push(hash_bytes(&combined));
        }
        level = next_level;
    }

    TransactionRoot::new(level[0])
}
