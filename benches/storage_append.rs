//! Day 5 Hour 4 -- cost of appending 100 blocks to the durable, fsync'd
//! block log (`storage::AppendOnlyBlockStore::append_block`, see
//! `docs/storage.md`). The 100 blocks are built once as setup; each
//! benchmark iteration opens a fresh log file in a fresh temp directory
//! (via `iter_batched`, so setup/teardown stay out of the timed region)
//! and appends all 100 to it, two `sync_data` calls per block. See
//! `docs/benchmarks.md` for the machine, command, results, and what these
//! numbers do and do not prove.

mod common;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use storage::{AppendOnlyBlockStore, BlockStore};

const ACCOUNT_COUNT: u8 = 16;
const TXS_PER_BLOCK: usize = 5;
const BLOCK_COUNT: usize = 100;

fn bench_append_blocks(c: &mut Criterion) {
    let (_genesis, blocks) = common::build_chain(ACCOUNT_COUNT, BLOCK_COUNT, TXS_PER_BLOCK);

    c.bench_function("storage_append_100_blocks", |b| {
        b.iter_batched(
            || tempfile::tempdir().expect("tempdir"),
            |dir| {
                let mut store =
                    AppendOnlyBlockStore::open(dir.path().join("blocks.log")).expect("open store");
                for block in &blocks {
                    store.append_block(block).expect("append block");
                }
                dir
            },
            BatchSize::SmallInput,
        );
    });
}

criterion_group!(benches, bench_append_blocks);
criterion_main!(benches);
