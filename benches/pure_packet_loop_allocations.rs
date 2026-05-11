#![allow(
    clippy::print_stdout,
    reason = "allocation profiler output is part of the benchmark artifact"
)]

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

use o_sfu_benchmarks::pure_packet_loop::{
    ALLOCATION_PROFILE_ITERATIONS, BenchmarkFailure, BenchmarkResult, DENSE_LOCAL_DESTINATIONS,
    ExpectedEffects,
};

fn main() -> BenchmarkResult {
    let mut turn = o_sfu_benchmarks::pure_packet_loop::dense_local_fanout_turn();
    turn.verify_next_turn(ExpectedEffects {
        local_forwards: DENSE_LOCAL_DESTINATIONS,
        intra_node_relay_forwards: 0,
        incoming_bitrate_effects: 1,
    })?;
    turn.warm(ALLOCATION_PROFILE_ITERATIONS);
    let packets = turn.packet_batch(ALLOCATION_PROFILE_ITERATIONS);
    let _profiler = dhat::Profiler::builder().testing().build();
    turn.run_packets(packets);
    let stats = dhat::HeapStats::get();
    println!(
        "packet_loop_warmed_dense_local_turn_allocations total_blocks={} total_bytes={}",
        stats.total_blocks, stats.total_bytes
    );
    if stats.total_blocks == 0 {
        Ok(())
    } else {
        Err(BenchmarkFailure::new(
            "warmed dense local packet-loop turns allocated heap memory",
        ))
    }
}
