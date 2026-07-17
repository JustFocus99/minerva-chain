//! Day 5 Hour 4 -- `execution::replay_chain` cost as a function of chain
//! length. Each benchmarked chain is built and confirmed to execute
//! cleanly once beforehand as setup; only the `replay_chain` call itself
//! (rebuilding state from genesis through every block again) is timed. See
//! `docs/benchmarks.md` for the machine, command, results, and what these
//! numbers do and do not prove.

mod common;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use execution::replay_chain;

const ACCOUNT_COUNT: u8 = 16;
const TXS_PER_BLOCK: usize = 5;

fn bench_replay(c: &mut Criterion) {
    let mut group = c.benchmark_group("replay_chain");
    for &block_count in &[100usize, 1000] {
        let (genesis, blocks) = common::build_chain(ACCOUNT_COUNT, block_count, TXS_PER_BLOCK);
        group.bench_with_input(BenchmarkId::from_parameter(block_count), &block_count, |b, _| {
            b.iter(|| {
                let result = replay_chain(genesis.clone(), &blocks);
                std::hint::black_box(result.expect("bench chain must replay successfully"));
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_replay);
criterion_main!(benches);
