use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use o_sfu_benchmarks::routing_miss_fingerprint::{
    REALISTIC_PACKET_LENGTHS, RoutingMissFingerprintBenchmarkFixture,
};

const PACKET_COUNT: usize = 4096;
const MISS_GATE_MIX_CYCLES: usize = 128;

fn bench_routing_miss_packet_fingerprint(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("routing_miss_packet_fingerprint");
    for packet_len in REALISTIC_PACKET_LENGTHS {
        let fixture = RoutingMissFingerprintBenchmarkFixture::new(PACKET_COUNT, packet_len);
        fixture.assert_production_matches_scalar();
        group.throughput(Throughput::Bytes(
            u64::try_from(fixture.byte_len()).unwrap_or(u64::MAX),
        ));
        group.bench_with_input(
            BenchmarkId::new("scalar_reference", packet_len),
            &packet_len,
            |bencher, _packet_len| {
                bencher.iter(|| fixture.scalar_cycle());
            },
        );
        group.bench_with_input(
            BenchmarkId::new("o_sfu_core_production", packet_len),
            &packet_len,
            |bencher, _packet_len| {
                bencher.iter(|| fixture.production_cycle());
            },
        );
    }

    let fixture = RoutingMissFingerprintBenchmarkFixture::miss_gate_mix(MISS_GATE_MIX_CYCLES);
    fixture.assert_production_matches_scalar();
    group.throughput(Throughput::Bytes(
        u64::try_from(fixture.byte_len()).unwrap_or(u64::MAX),
    ));
    group.bench_function(
        BenchmarkId::new("scalar_reference", "weighted_miss_gate"),
        |bencher| {
            bencher.iter(|| fixture.scalar_cycle());
        },
    );
    group.bench_function(
        BenchmarkId::new("o_sfu_core_production", "weighted_miss_gate"),
        |bencher| {
            bencher.iter(|| fixture.production_cycle());
        },
    );
    group.finish();
}

criterion_group!(benches, bench_routing_miss_packet_fingerprint);
criterion_main!(benches);
