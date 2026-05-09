use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use o_sfu_benchmarks::packet_metrics::PacketMetricsContentionBenchmarkFixture;

fn bench_packet_metrics_contention(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("packet_metrics_contention");
    for worker_count in [1_usize, 2, 4, 8] {
        let fixture = PacketMetricsContentionBenchmarkFixture::new(worker_count);
        group.throughput(Throughput::Elements(fixture.operation_count_u64()));
        group.bench_with_input(
            BenchmarkId::new("shared_global_atomic", worker_count),
            &fixture,
            |bencher, fixture| {
                bencher.iter(|| fixture.shared_global_atomic_cycle());
            },
        );
        group.bench_with_input(
            BenchmarkId::new("per_worker_atomic", worker_count),
            &fixture,
            |bencher, fixture| {
                bencher.iter(|| fixture.per_worker_atomic_cycle());
            },
        );
        group.bench_with_input(
            BenchmarkId::new("thread_local_aggregate", worker_count),
            &fixture,
            |bencher, fixture| {
                bencher.iter(|| fixture.thread_local_aggregate_cycle());
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_packet_metrics_contention);
criterion_main!(benches);
