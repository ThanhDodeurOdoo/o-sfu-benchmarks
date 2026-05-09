use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use o_sfu_benchmarks::local_vp8_fanout::LocalVp8FanoutBenchmarkFixture;

fn bench_local_vp8_fanout(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("local_vp8_fanout");
    for destination_count in [1_usize, 8, 32, 64] {
        group.throughput(Throughput::Elements(
            u64::try_from(destination_count).unwrap_or(u64::MAX),
        ));
        group.bench_with_input(
            BenchmarkId::new("current_parse_per_destination", destination_count),
            &destination_count,
            |bencher, &destination_count| {
                let mut fixture = LocalVp8FanoutBenchmarkFixture::new(destination_count);
                bencher.iter(|| fixture.current_parse_per_destination_cycle());
            },
        );
        group.bench_with_input(
            BenchmarkId::new("cached_descriptor", destination_count),
            &destination_count,
            |bencher, &destination_count| {
                let mut fixture = LocalVp8FanoutBenchmarkFixture::new(destination_count);
                bencher.iter(|| fixture.cached_descriptor_cycle());
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_local_vp8_fanout);
criterion_main!(benches);
