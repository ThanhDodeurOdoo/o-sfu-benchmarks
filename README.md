# o-sfu-benchmarks

microbenchmarks for `o-sfu`

this repository owns runnable hot-path benchmarks
some targets are exploratory

the benchmarks depend on `../o-sfu` through a relative path, if you want to use
it out of the box, both repos need to be in the same parent folder.

`packet_loop_cold_path` benchmarks branch-shape hypotheses for the current
packet-loop hot path
it uses public `o-sfu-core` types where possible
private packet-loop functions are not copied into this repository

## Run

```bash
cargo bench
```

Run one benchmark target:

```bash
cargo bench --bench rtc_udp_demux
cargo bench --bench performance_hardening
cargo bench --bench simd_packet_hotpath
cargo bench --bench packet_loop_cold_path
# ...
```
