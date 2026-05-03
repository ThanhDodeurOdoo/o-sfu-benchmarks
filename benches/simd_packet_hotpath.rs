use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use o_sfu_benchmarks::simd_packet_hotpath::{
    H264PayloadScannerBenchmarkFixture, PacketFingerprintBenchmarkFixture,
    Vp8PayloadScannerBenchmarkFixture,
};

fn bench_packet_fingerprint(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("packet_fingerprint_scalar_vs_simd");
    for packet_len in [64_usize, 256, 1200] {
        let fixture = PacketFingerprintBenchmarkFixture::new(1024, packet_len);
        group.throughput(Throughput::Bytes(
            u64::try_from(1024_usize.saturating_mul(packet_len)).unwrap_or(u64::MAX),
        ));
        group.bench_with_input(
            BenchmarkId::new("current_scalar", packet_len),
            &packet_len,
            |bencher, _packet_len| {
                bencher.iter(|| fixture.scalar_cycle());
            },
        );
        group.bench_with_input(
            BenchmarkId::new("benchmark_simd", packet_len),
            &packet_len,
            |bencher, _packet_len| {
                bencher.iter(|| fixture.simd_cycle());
            },
        );
    }
    group.finish();
}

fn bench_h264_payload_scanner(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("h264_payload_scanner_scalar_vs_simd");
    for stap_a_nal_count in [4_usize, 16, 64] {
        let fixture = H264PayloadScannerBenchmarkFixture::new(512, stap_a_nal_count);
        group.throughput(Throughput::Elements(
            u64::try_from(512_usize).unwrap_or(u64::MAX),
        ));
        group.bench_with_input(
            BenchmarkId::new("current_scalar", stap_a_nal_count),
            &stap_a_nal_count,
            |bencher, _stap_a_nal_count| {
                bencher.iter(|| fixture.scalar_cycle());
            },
        );
        group.bench_with_input(
            BenchmarkId::new("benchmark_simd_stap_a_headers", stap_a_nal_count),
            &stap_a_nal_count,
            |bencher, _stap_a_nal_count| {
                bencher.iter(|| fixture.simd_cycle());
            },
        );
    }
    group.finish();
}

fn bench_vp8_payload_scanner(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("vp8_payload_scanner_scalar_vs_simd_batch");
    for payload_count in [128_usize, 512, 2048] {
        let fixture = Vp8PayloadScannerBenchmarkFixture::new(payload_count);
        group.throughput(Throughput::Elements(
            u64::try_from(payload_count).unwrap_or(u64::MAX),
        ));
        group.bench_with_input(
            BenchmarkId::new("current_scalar", payload_count),
            &payload_count,
            |bencher, _payload_count| {
                bencher.iter(|| fixture.scalar_cycle());
            },
        );
        group.bench_with_input(
            BenchmarkId::new("benchmark_simd_batch", payload_count),
            &payload_count,
            |bencher, _payload_count| {
                bencher.iter(|| fixture.simd_batch_cycle());
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_packet_fingerprint,
    bench_h264_payload_scanner,
    bench_vp8_payload_scanner
);
criterion_main!(benches);
