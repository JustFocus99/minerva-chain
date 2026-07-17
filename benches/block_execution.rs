//! Day 5 Hour 4 -- `ChainState::execute_block` cost as a function of
//! transaction count. Each benchmarked block is unexecuted and chains
//! directly onto genesis; only the `execute_block` call itself is timed,
//! block construction happens once beforehand as setup. See
//! `docs/benchmarks.md` for the machine, command, results, and what these
//! numbers do and do not prove.

mod common;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use state::chain_state::ChainState;

const ACCOUNT_COUNT: u8 = 16;

fn bench_execute_block(c: &mut Criterion) {
    let mut group = c.benchmark_group("execute_block");
    for &tx_count in &[10usize, 100, 1000] {
        let (parent_state, block) = common::build_execution_bench(ACCOUNT_COUNT, tx_count);
        group.bench_with_input(BenchmarkId::from_parameter(tx_count), &tx_count, |b, _| {
            b.iter(|| {
                let result = ChainState::execute_block(&parent_state, block.clone());
                std::hint::black_box(result.expect("bench block must execute successfully"));
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_execute_block);
criterion_main!(benches);
