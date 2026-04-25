# o-sfu-benchmarks

Criterion microbenchmarks for `o-sfu`.

This repo is intentionally separate from `../o-sfu` so the main server workspace
does not need Criterion, benchmark-only cargo features, or runtime-specific
fixture modules.

The benchmarks depend on `../o-sfu` through a relative path and only reuse the
hidden `o_sfu::testing::transport` exports for pure transport data structures
that are already useful to deterministic tests.

## Run

```bash
cargo bench
```

Run one benchmark target:

```bash
cargo bench --bench rtc_udp_demux
cargo bench --bench performance_hardening
```
