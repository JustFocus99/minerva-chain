//! Hour 4 -- local fork choice. See docs/fork-choice.md: greatest height
//! wins, ties go to the lexicographically smallest tip hash, and the whole
//! thing must be deterministic regardless of insertion order.

use block::block::BlockHeader;
use block::{ChainBranch, ForkChoiceError, ForkTree, GENESIS_PARENT_HASH, InsertOutcome};
use primitives::TransactionRoot;

fn genesis() -> BlockHeader {
    BlockHeader::new(0, GENESIS_PARENT_HASH, TransactionRoot::new([0u8; 32]), [0u8; 32], [0u8; 32], 0)
}

/// Builds a header chaining onto `parent`. `seed` only exists to vary the
/// header's hash (via `producer`/`slot`/`state_commitment`) so competing
/// blocks at the same height and parent don't collide.
fn child(parent: &BlockHeader, seed: u8) -> BlockHeader {
    BlockHeader::new(
        parent.height + 1,
        parent.block_hash,
        TransactionRoot::new([seed; 32]),
        [seed; 32],
        [seed; 32],
        seed as u64,
    )
}

#[test]
fn chooses_longest_chain() {
    let genesis = genesis();
    let mut tree = ForkTree::new(genesis);

    let b1 = child(&genesis, 1);
    let b2 = child(&b1, 2);
    let b3a = child(&b2, 3);
    let b4a = child(&b3a, 4);
    let b3b = child(&b2, 5);

    for header in [b1, b2, b3a, b4a, b3b] {
        tree.insert_block(header).expect("insert should succeed");
    }

    assert_eq!(
        tree.canonical_tip(),
        ChainBranch {
            tip_hash: b4a.block_hash,
            height: 4,
        }
    );
    assert_eq!(tree.choose_tip(), tree.canonical_tip());
}

#[test]
fn tie_breaks_by_block_hash() {
    let genesis = genesis();
    let b1 = child(&genesis, 1);
    let b2 = child(&b1, 2);
    let b3a = child(&b2, 3);
    let b3b = child(&b2, 4);
    assert_eq!(b3a.height, b3b.height);
    assert_ne!(b3a.block_hash, b3b.block_hash);

    let expected_tip = std::cmp::min(b3a.block_hash, b3b.block_hash);

    // Same set of blocks, two different insertion orders -- two nodes that
    // saw these blocks in a different order must still agree on the tip.
    let mut tree_a = ForkTree::new(genesis);
    for header in [b1, b2, b3a, b3b] {
        tree_a.insert_block(header).expect("insert should succeed");
    }

    let mut tree_b = ForkTree::new(genesis);
    for header in [b1, b2, b3b, b3a] {
        tree_b.insert_block(header).expect("insert should succeed");
    }

    assert_eq!(tree_a.canonical_tip().tip_hash, expected_tip);
    assert_eq!(tree_b.canonical_tip().tip_hash, expected_tip);
    assert_eq!(tree_a.canonical_tip(), tree_b.canonical_tip());
}

#[test]
fn rejects_unknown_parent() {
    let genesis = genesis();
    let mut tree = ForkTree::new(genesis);

    let orphan = BlockHeader::new(1, [0xAAu8; 32], TransactionRoot::new([1u8; 32]), [1u8; 32], [1u8; 32], 1);

    let err = tree.insert_block(orphan).expect_err("unknown parent must be rejected");
    assert_eq!(
        err,
        ForkChoiceError::UnknownParent {
            parent_hash: [0xAAu8; 32]
        }
    );

    assert!(!tree.contains_block(&orphan.block_hash));
    assert_eq!(tree.canonical_tip().tip_hash, genesis.block_hash);
}

#[test]
fn handles_competing_forks() {
    let genesis = genesis();
    let mut tree = ForkTree::new(genesis);

    let b1 = child(&genesis, 1);
    tree.insert_block(b1).expect("insert b1");

    let b2a = child(&b1, 2);
    let b2b = child(&b1, 3);
    tree.insert_block(b2a).expect("insert b2a");
    tree.insert_block(b2b).expect("insert b2b");

    // Both branches are known...
    assert!(tree.contains_block(&b2a.block_hash));
    assert!(tree.contains_block(&b2b.block_hash));
    assert_eq!(tree.parent_of(&b2a.block_hash), Some(b1.block_hash));
    assert_eq!(tree.parent_of(&b2b.block_hash), Some(b1.block_hash));

    // ...but only one is canonical, chosen by the same deterministic rule.
    let expected_tip = std::cmp::min(b2a.block_hash, b2b.block_hash);
    assert_eq!(tree.canonical_tip().tip_hash, expected_tip);
}

#[test]
fn does_not_reimport_same_block_twice() {
    let genesis = genesis();
    let mut tree = ForkTree::new(genesis);

    let b1 = child(&genesis, 1);
    assert_eq!(tree.insert_block(b1).unwrap(), InsertOutcome::Inserted);

    let tip_before = tree.canonical_tip();
    assert_eq!(tree.insert_block(b1).unwrap(), InsertOutcome::AlreadyKnown);

    assert_eq!(tree.canonical_tip(), tip_before);
    assert_eq!(tree.height_of(&b1.block_hash), Some(1));
}
