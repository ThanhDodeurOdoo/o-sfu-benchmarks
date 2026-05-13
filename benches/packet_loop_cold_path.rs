use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use o_sfu_benchmarks::packet_loop_cold_path::{
    IncomingStatsColdPathBenchmarkFixture, IngressColdPathBenchmarkFixture,
    RoutePlanningColdPathBenchmarkFixture,
};

fn bench_ingress_routing_cold_paths(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("packet_loop_ingress_cold_paths");
    for event_count in [1_024_usize, 16_384, 65_536] {
        let fixture = IngressColdPathBenchmarkFixture::new(event_count);
        group.throughput(Throughput::Elements(fixture.event_count_u64()));
        group.bench_with_input(
            BenchmarkId::new("inline_rare_branches", event_count),
            &fixture,
            |bencher, fixture| {
                bencher.iter(|| fixture.inline_rare_branch_cycle());
            },
        );
        group.bench_with_input(
            BenchmarkId::new("cold_rare_branches", event_count),
            &fixture,
            |bencher, fixture| {
                bencher.iter(|| fixture.cold_rare_branch_cycle());
            },
        );
    }
    group.finish();
}

fn bench_forward_planning_cold_paths(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("packet_loop_forward_planning_cold_paths");
    for destination_count in [32_usize, 128, 512] {
        let fixture = RoutePlanningColdPathBenchmarkFixture::new(256, destination_count);
        group.throughput(Throughput::Elements(fixture.route_attempt_count_u64()));
        group.bench_with_input(
            BenchmarkId::new("inline_reject_branches", destination_count),
            &fixture,
            |bencher, fixture| {
                bencher.iter(|| fixture.inline_reject_branch_cycle());
            },
        );
        group.bench_with_input(
            BenchmarkId::new("cold_reject_branches", destination_count),
            &fixture,
            |bencher, fixture| {
                bencher.iter(|| fixture.cold_reject_branch_cycle());
            },
        );
    }
    group.finish();
}

fn bench_incoming_stats_cold_paths(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("packet_loop_incoming_stats_cold_paths");
    for packet_count in [1_024_usize, 16_384, 65_536] {
        let fixture = IncomingStatsColdPathBenchmarkFixture::new(packet_count);
        group.throughput(Throughput::Elements(fixture.packet_count_u64()));
        group.bench_with_input(
            BenchmarkId::new("inline_observation_branches", packet_count),
            &fixture,
            |bencher, fixture| {
                bencher.iter(|| fixture.inline_observation_cycle());
            },
        );
        group.bench_with_input(
            BenchmarkId::new("cold_observation_branches", packet_count),
            &fixture,
            |bencher, fixture| {
                bencher.iter(|| fixture.cold_observation_cycle());
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_ingress_routing_cold_paths,
    bench_forward_planning_cold_paths,
    bench_incoming_stats_cold_paths
);
criterion_main!(benches);
