# o-sfu-benchmarks

microbenchmarks for `o-sfu`

this repository owns runnable hot-path benchmarks. Some targets are exploratory.
The packet-loop targets are production-backed and call `../o-sfu/core` through
the `packet-loop-verification` feature.

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
cargo bench --bench pure_packet_loop_turn
cargo bench --bench pure_packet_loop_allocations
# ...
```

Packet-loop benchmarks must stay production-backed. Do not copy route planning,
demux or turn logic into this repository.
