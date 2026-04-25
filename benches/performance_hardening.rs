use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use o_sfu_benchmarks::performance_hardening::{
    KeyframeCoalescingBenchmarkFixture, LocalEgressFanoutBenchmarkFixture,
    RoutePlanningBenchmarkFixture, SourcePolicyWakeBenchmarkFixture,
};

fn bench_source_policy_wakes(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("source_policy_wakes");
    for mark_count in [64_usize, 256, 1024] {
        let mut fixture = SourcePolicyWakeBenchmarkFixture::new(mark_count);
        group.throughput(Throughput::Elements(
            u64::try_from(mark_count).unwrap_or(u64::MAX),
        ));
        group.bench_with_input(
            BenchmarkId::new("coalesced_worker_buffer", mark_count),
            &mark_count,
            |bencher, _mark_count| {
                bencher.iter(|| fixture.coalesced_worker_buffer_cycle());
            },
        );
    }
    group.finish();
}

fn bench_keyframe_coalescing(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("keyframe_coalescing");
    for request_count in [32_usize, 128, 512] {
        let mut fixture = KeyframeCoalescingBenchmarkFixture::new(request_count);
        group.throughput(Throughput::Elements(
            u64::try_from(request_count).unwrap_or(u64::MAX),
        ));
        group.bench_with_input(
            BenchmarkId::new("reusable_vec_sort_group", request_count),
            &request_count,
            |bencher, _request_count| {
                bencher.iter(|| fixture.reusable_vec_coalescing_cycle());
            },
        );
        group.bench_with_input(
            BenchmarkId::new("fresh_btree_map", request_count),
            &request_count,
            |bencher, _request_count| {
                bencher.iter(|| fixture.fresh_btree_coalescing_cycle());
            },
        );
    }
    group.finish();
}

fn bench_route_planning(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("route_planning");
    for destination_count in [8_usize, 32, 128] {
        let fixture = RoutePlanningBenchmarkFixture::new(64, destination_count);
        group.throughput(Throughput::Elements(
            u64::try_from(destination_count).unwrap_or(u64::MAX),
        ));
        group.bench_with_input(
            BenchmarkId::new("destination_gate_cycle", destination_count),
            &destination_count,
            |bencher, _destination_count| {
                bencher.iter(|| fixture.destination_gate_cycle());
            },
        );
    }
    group.finish();
}

fn bench_local_egress_fanout(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("local_egress_fanout");
    for destination_count in [8_usize, 32, 128] {
        let mut fixture = LocalEgressFanoutBenchmarkFixture::new(destination_count);
        group.throughput(Throughput::Elements(
            u64::try_from(destination_count).unwrap_or(u64::MAX),
        ));
        group.bench_with_input(
            BenchmarkId::new("shared_payload_planning", destination_count),
            &destination_count,
            |bencher, _destination_count| {
                bencher.iter(|| fixture.shared_payload_planning_cycle());
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_source_policy_wakes,
    bench_keyframe_coalescing,
    bench_route_planning,
    bench_local_egress_fanout
);
criterion_main!(benches);
