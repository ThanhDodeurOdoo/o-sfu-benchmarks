use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use o_sfu_benchmarks::source_policy_refresh::SourcePolicyRefreshBenchmarkFixture;

fn bench_source_policy_refresh(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("source_policy_refresh");
    for user_count in [16_usize, 64, 100] {
        for sources_per_user in [1_usize, 3] {
            let fixture = SourcePolicyRefreshBenchmarkFixture::new(user_count, sources_per_user);
            let route_count = fixture.route_count_u64();
            group.throughput(Throughput::Elements(route_count));
            group.bench_with_input(
                BenchmarkId::new(
                    format!("current_rebuild_{}src", sources_per_user),
                    user_count,
                ),
                &fixture,
                |bencher, fixture| {
                    bencher.iter(|| fixture.current_rebuild_cycle());
                },
            );
            group.bench_with_input(
                BenchmarkId::new(
                    format!("cached_ladders_{}src", sources_per_user),
                    user_count,
                ),
                &fixture,
                |bencher, fixture| {
                    bencher.iter(|| fixture.cached_ladder_cycle());
                },
            );
        }
    }
    group.finish();
}

criterion_group!(benches, bench_source_policy_refresh);
criterion_main!(benches);
