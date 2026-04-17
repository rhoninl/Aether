//! Replay-speed benchmark.
//!
//! Target: 10x–100x wall-clock on a 1000-tick empty scenario. We bench
//! the engine with criterion; run with `--test` for a smoke pass.

use std::time::Duration;

use aether_sim_harness::{run_scenario, Scenario};
use criterion::{criterion_group, criterion_main, Criterion};

fn bench_1000_tick_scenario(c: &mut Criterion) {
    let scenario = Scenario::new("bench_1000_ticks").push_ticks(1000);
    let sim_wall_equivalent = Duration::from_secs(1000 / 60); // 60Hz

    c.bench_function("replay_1000_empty_ticks", |b| {
        b.iter(|| {
            let report = run_scenario(&scenario);
            assert!(report.verdict.is_pass());
        });
    });

    // Sanity: one standalone run should be well under the 60Hz wall-clock equivalent.
    let start = std::time::Instant::now();
    let report = run_scenario(&scenario);
    let elapsed = start.elapsed();
    assert!(report.verdict.is_pass());
    let speedup = sim_wall_equivalent.as_secs_f64() / elapsed.as_secs_f64().max(1e-9);
    println!(
        "1000-tick replay wall-clock: {:?}, sim-wall equivalent: {:?}, speedup: {:.1}x",
        elapsed, sim_wall_equivalent, speedup
    );
    assert!(
        speedup >= 10.0,
        "expected >=10x speedup, got {speedup:.2}x (elapsed: {elapsed:?})"
    );
}

criterion_group!(benches, bench_1000_tick_scenario);
criterion_main!(benches);
