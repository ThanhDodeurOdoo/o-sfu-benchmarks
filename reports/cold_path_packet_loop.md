# packet loop cold path benchmark

## scope

this pass benchmarks cold path hypotheses for the current `o-sfu` packet loop without changing `o-sfu`

the benchmark repo was first synchronized with the current `o-sfu` crate layout by replacing the stale `../o-sfu/core` and `../o-sfu/rfc` paths with `../o-sfu/crates/core` and `../o-sfu/crates/rfc`

the old `pure_packet_loop` target depended on a removed `packet_loop_verification` feature, so it was removed from this repo

the new `packet_loop_cold_path` target keeps private packet-loop production logic out of this repo and uses public `o-sfu-core` types where the harness needs real runtime identity keys

## source inspection

the likely cold path candidates are in the current packet loop files:

- `/Volumes/X9-Pro/odoo-dev/o-sfu/crates/core/src/runtime/rtc_engine/packet_loop/ingress_routing.rs`
- `/Volumes/X9-Pro/odoo-dev/o-sfu/crates/core/src/runtime/rtc_engine/packet_loop/forward_flush.rs`
- `/Volumes/X9-Pro/odoo-dev/o-sfu/crates/core/src/runtime/rtc_engine/packet_loop/buffers.rs`
- `/Volumes/X9-Pro/odoo-dev/o-sfu/crates/core/src/runtime/rtc_engine/forwarding_planner.rs`
- `/Volumes/X9-Pro/odoo-dev/o-sfu/crates/core/src/runtime/rtc_engine/routing_miss.rs`

## benchmark command

```bash
cargo bench --bench packet_loop_cold_path -- --warm-up-time 1 --measurement-time 2 --sample-size 20
```

## results

| area | size | inline mean | cold mean | result |
| --- | ---: | ---: | ---: | --- |
| ingress routing rare branches | 1024 events | 1.2579 us | 1.2976 us | cold is 3.2 percent slower |
| ingress routing rare branches | 16384 events | 21.009 us | 22.586 us | cold is 7.5 percent slower |
| ingress routing rare branches | 65536 events | 82.326 us | 89.517 us | cold is 8.7 percent slower |
| forward planning reject branches | 32 destinations | 6.8271 us | 7.9828 us | cold is 16.9 percent slower |
| forward planning reject branches | 128 destinations | 27.203 us | 32.951 us | cold is 21.1 percent slower |
| forward planning reject branches | 512 destinations | 107.24 us | 136.43 us | cold is 27.2 percent slower |
| incoming stats rare observations | 1024 packets | 669.70 ns | 432.84 ns | cold is 35.4 percent faster |
| incoming stats rare observations | 16384 packets | 9.2837 us | 6.9810 us | cold is 24.8 percent faster |
| incoming stats rare observations | 65536 packets | 36.417 us | 28.421 us | cold is 22.0 percent faster |

## conclusions

`#[cold]` is not a good blanket packet-loop optimization

the ingress routing ladder and forward planning reject branches should not be moved out of line yet because the benchmark makes those paths slower

the strongest candidate is packet observation in `forward_flush::record_incoming_stats`

the cold work there is first-ingress handling, first-video-keyframe requests and audio-policy dirty wakeups

those branches are naturally rare after warmup, they carry diagnostic plus policy side effects and the benchmark shows a 22 to 35 percent speedup for a matching branch shape

## recommended next production experiment

make a focused `o-sfu` change that extracts rare observation side effects from `record_incoming_stats` into small cold helpers

benchmark that production-backed change with a packet-loop benchmark before keeping it

reject the change if production-backed measurements fail to reproduce the incoming-stats speedup or if it makes first-ingress heavy workloads slower enough to matter
