# o-sfu-benchmarks

microbenchmarks for `o-sfu`

this is mostly a testbed/playground to test ideas.

The benchmarks depend on `../o-sfu` through a relative path, if you want to use
it out of the box, both repos need to be in the same parent folder.

## Run

```bash
cargo bench
```

Run one benchmark target:

```bash
cargo bench --bench rtc_udp_demux
cargo bench --bench performance_hardening
cargo bench --bench simd_packet_hotpath
# ...
```
