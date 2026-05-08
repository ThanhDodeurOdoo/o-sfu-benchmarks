use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use o_sfu_benchmarks::consumer_packet_gate::{
    ConsumerPacketGateBatchBenchmarkFixture, ConsumerPacketGateRouteMaintenanceBenchmarkFixture,
    benchmark_source_media_id, build_current_linear_route, build_media_id_indexed_route,
};

fn bench_consumer_packet_gate_batch(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("consumer_packet_gate_batch_full_source");
    for destination_count in [8_usize, 32, 128, 512] {
        group.throughput(Throughput::Elements(
            u64::try_from(destination_count).unwrap_or(u64::MAX),
        ));
        group.bench_with_input(
            BenchmarkId::new(
                format!("current_linear_scan/source_{}", benchmark_source_media_id()),
                destination_count,
            ),
            &destination_count,
            |bencher, &destination_count| {
                let mut fixture = ConsumerPacketGateBatchBenchmarkFixture::new(destination_count);
                bencher.iter(|| fixture.current_linear_batch_cycle());
            },
        );
        group.bench_with_input(
            BenchmarkId::new(
                format!(
                    "media_id_btree_index/source_{}",
                    benchmark_source_media_id()
                ),
                destination_count,
            ),
            &destination_count,
            |bencher, &destination_count| {
                let mut fixture = ConsumerPacketGateBatchBenchmarkFixture::new(destination_count);
                bencher.iter(|| fixture.media_id_indexed_batch_cycle());
            },
        );
        group.bench_with_input(
            BenchmarkId::new(
                format!(
                    "identity_btree_index/source_{}",
                    benchmark_source_media_id()
                ),
                destination_count,
            ),
            &destination_count,
            |bencher, &destination_count| {
                let mut fixture = ConsumerPacketGateBatchBenchmarkFixture::new(destination_count);
                bencher.iter(|| fixture.identity_indexed_batch_cycle());
            },
        );
    }
    group.finish();
}

fn bench_consumer_packet_gate_route_creation(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("consumer_packet_gate_route_creation");
    for destination_count in [8_usize, 32, 128, 512] {
        group.throughput(Throughput::Elements(
            u64::try_from(destination_count).unwrap_or(u64::MAX),
        ));
        group.bench_with_input(
            BenchmarkId::new("current_vec_push", destination_count),
            &destination_count,
            |bencher, &destination_count| {
                bencher.iter(|| build_current_linear_route(destination_count));
            },
        );
        group.bench_with_input(
            BenchmarkId::new("media_id_indexed_vec_push", destination_count),
            &destination_count,
            |bencher, &destination_count| {
                bencher.iter(|| build_media_id_indexed_route(destination_count));
            },
        );
    }
    group.finish();
}

fn bench_consumer_packet_gate_route_maintenance(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("consumer_packet_gate_route_maintenance");
    for destination_count in [8_usize, 32, 128, 512] {
        group.throughput(Throughput::Elements(
            u64::try_from(destination_count).unwrap_or(u64::MAX),
        ));
        group.bench_with_input(
            BenchmarkId::new("current_linear_remove_add", destination_count),
            &destination_count,
            |bencher, &destination_count| {
                let mut fixture =
                    ConsumerPacketGateRouteMaintenanceBenchmarkFixture::new(destination_count);
                bencher.iter(|| fixture.current_linear_remove_add_cycle());
            },
        );
        group.bench_with_input(
            BenchmarkId::new("media_id_indexed_swap_remove_add", destination_count),
            &destination_count,
            |bencher, &destination_count| {
                let mut fixture =
                    ConsumerPacketGateRouteMaintenanceBenchmarkFixture::new(destination_count);
                bencher.iter(|| fixture.media_id_indexed_swap_remove_add_cycle());
            },
        );
        group.bench_with_input(
            BenchmarkId::new("media_id_indexed_stable_remove_add", destination_count),
            &destination_count,
            |bencher, &destination_count| {
                let mut fixture =
                    ConsumerPacketGateRouteMaintenanceBenchmarkFixture::new(destination_count);
                bencher.iter(|| fixture.media_id_indexed_stable_remove_add_cycle());
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_consumer_packet_gate_batch,
    bench_consumer_packet_gate_route_creation,
    bench_consumer_packet_gate_route_maintenance
);
criterion_main!(benches);
