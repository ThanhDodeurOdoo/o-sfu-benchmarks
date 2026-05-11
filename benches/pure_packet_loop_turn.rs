#![allow(clippy::print_stdout, reason = "cargo bench output")]

use std::{hint::black_box, net::SocketAddr};

use o_sfu_benchmarks::pure_packet_loop::{
    BenchmarkResult, DENSE_LOCAL_DESTINATIONS, ExpectedEffects, RELAY_DESTINATIONS,
    run_sampled_idle_benchmark, run_sampled_operation, run_sampled_turn_benchmark,
};
use o_sfu_core::server::transport::packet_loop_verification::{
    KeyframeRequestKind, PacketLoopRoutingMissKey, PacketLoopRoutingState, PacketLoopScratch,
    PacketLoopTime, coalesce_keyframe_kind,
};

fn main() -> BenchmarkResult {
    packet_loop_turn_idle()?;
    packet_loop_turn_one_ingress()?;
    packet_loop_turn_dense_local_fanout()?;
    packet_loop_turn_relay_fanout()?;
    packet_loop_demux_cached()?;
    packet_loop_demux_unknown_source()?;
    packet_loop_keyframe_coalescing()?;
    packet_loop_source_policy_dirty_coalescing()?;
    packet_loop_scratch_reuse()?;
    Ok(())
}

fn packet_loop_turn_idle() -> BenchmarkResult {
    run_sampled_idle_benchmark("packet_loop_turn_idle")
}

fn packet_loop_turn_one_ingress() -> BenchmarkResult {
    let mut turn = o_sfu_benchmarks::pure_packet_loop::one_ingress_turn();
    run_sampled_turn_benchmark(
        "packet_loop_turn_one_ingress",
        &mut turn,
        ExpectedEffects {
            local_forwards: 1,
            intra_node_relay_forwards: 0,
            incoming_bitrate_effects: 1,
        },
    )
}

fn packet_loop_turn_dense_local_fanout() -> BenchmarkResult {
    let mut turn = o_sfu_benchmarks::pure_packet_loop::dense_local_fanout_turn();
    run_sampled_turn_benchmark(
        "packet_loop_turn_dense_local_fanout",
        &mut turn,
        ExpectedEffects {
            local_forwards: DENSE_LOCAL_DESTINATIONS,
            intra_node_relay_forwards: 0,
            incoming_bitrate_effects: 1,
        },
    )
}

fn packet_loop_turn_relay_fanout() -> BenchmarkResult {
    let mut turn = o_sfu_benchmarks::pure_packet_loop::relay_fanout_turn();
    run_sampled_turn_benchmark(
        "packet_loop_turn_relay_fanout",
        &mut turn,
        ExpectedEffects {
            local_forwards: 0,
            intra_node_relay_forwards: RELAY_DESTINATIONS,
            incoming_bitrate_effects: 1,
        },
    )
}

fn packet_loop_demux_cached() -> BenchmarkResult {
    let source = SocketAddr::from(([127, 0, 0, 1], 42_000));
    let candidate = SocketAddr::from(([127, 0, 0, 1], 42_001));
    let packet = [0x80, 0x60, 0x00, 0x01];
    let key = PacketLoopRoutingMissKey::new(source, candidate, &packet);
    let mut demux = PacketLoopRoutingState::new();
    demux.record_miss(key, &packet, source, PacketLoopTime::from_millis(0));
    run_sampled_operation("packet_loop_demux_cached", || {
        let _ = black_box(demux.should_skip_scan(key, &packet));
    })
}

fn packet_loop_demux_unknown_source() -> BenchmarkResult {
    let source = SocketAddr::from(([127, 0, 0, 1], 42_010));
    let candidate = SocketAddr::from(([127, 0, 0, 1], 42_011));
    let packet = [0x80, 0x60, 0x00, 0x01];
    let key = PacketLoopRoutingMissKey::new(source, candidate, &packet);
    let mut demux = PacketLoopRoutingState::new();
    run_sampled_operation("packet_loop_demux_unknown_source", || {
        demux.record_miss(key, &packet, source, PacketLoopTime::from_millis(0));
        demux.record_fallback_route_success(key, &packet, source);
    })
}

fn packet_loop_keyframe_coalescing() -> BenchmarkResult {
    run_sampled_operation("packet_loop_keyframe_coalescing", || {
        black_box(coalesce_keyframe_kind(
            KeyframeRequestKind::Pli,
            KeyframeRequestKind::Fir,
        ));
    })
}

fn packet_loop_source_policy_dirty_coalescing() -> BenchmarkResult {
    let mut scratch = PacketLoopScratch::new();
    run_sampled_operation("packet_loop_source_policy_dirty_coalescing", || {
        scratch.clear();
        for idx in 0_u64..64 {
            scratch.mark_source_policy_dirty(o_sfu_core::RoomInstanceId::from_raw(idx));
        }
        black_box(scratch.dirty_source_policy_rooms().len());
    })
}

fn packet_loop_scratch_reuse() -> BenchmarkResult {
    let mut scratch = PacketLoopScratch::new();
    let payload = vec![0xA5; 64];
    o_sfu_benchmarks::pure_packet_loop::warm_scratch(&mut scratch, 64, &payload);
    let warmed = scratch.capacities();
    run_sampled_operation("packet_loop_scratch_reuse", || {
        scratch.clear();
        o_sfu_benchmarks::pure_packet_loop::warm_scratch(&mut scratch, 16, &payload);
        black_box(scratch.capacities().retained_at_least(warmed));
    })
}
