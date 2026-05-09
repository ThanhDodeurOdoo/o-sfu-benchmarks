use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use o_sfu_benchmarks::bitrate_counter::BitrateCounterWriteBenchmarkFixture;

fn bench_bitrate_counter_write(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("bitrate_counter_write");
    for operation_count in [65_536_usize, 1_000_000] {
        let fixture = BitrateCounterWriteBenchmarkFixture::new(operation_count);
        group.throughput(Throughput::Elements(fixture.operation_count_u64()));
        group.bench_with_input(
            BenchmarkId::new("current_fetch_update", operation_count),
            &fixture,
            |bencher, fixture| {
                bencher.iter(|| fixture.current_fetch_update_write_cycle());
            },
        );
        group.bench_with_input(
            BenchmarkId::new("fetch_add_release", operation_count),
            &fixture,
            |bencher, fixture| {
                bencher.iter(|| fixture.fetch_add_write_cycle());
            },
        );
        group.bench_with_input(
            BenchmarkId::new("single_writer_load_store", operation_count),
            &fixture,
            |bencher, fixture| {
                bencher.iter(|| fixture.single_writer_load_store_write_cycle());
            },
        );
    }
    group.finish();
}

fn bench_bitrate_counter_record_same_window(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("bitrate_counter_record_same_window");
    for operation_count in [65_536_usize, 1_000_000] {
        let fixture = BitrateCounterWriteBenchmarkFixture::new(operation_count);
        group.throughput(Throughput::Elements(fixture.operation_count_u64()));
        group.bench_with_input(
            BenchmarkId::new("current_fetch_update", operation_count),
            &fixture,
            |bencher, fixture| {
                bencher.iter(|| fixture.current_fetch_update_record_cycle());
            },
        );
        group.bench_with_input(
            BenchmarkId::new("fetch_add_release", operation_count),
            &fixture,
            |bencher, fixture| {
                bencher.iter(|| fixture.fetch_add_record_cycle());
            },
        );
        group.bench_with_input(
            BenchmarkId::new("single_writer_load_store", operation_count),
            &fixture,
            |bencher, fixture| {
                bencher.iter(|| fixture.single_writer_load_store_record_cycle());
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_bitrate_counter_write,
    bench_bitrate_counter_record_same_window
);
criterion_main!(benches);
