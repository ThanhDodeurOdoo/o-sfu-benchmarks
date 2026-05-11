use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    hint::black_box,
    net::SocketAddr,
    time::{Duration, Instant},
};

use o_sfu_core::{
    ConnectionId, RoomInstanceId,
    server::{
        session::UserId,
        transport::{
            TransportMediaId, TransportSessionKey,
            packet_loop_verification::{
                DrainedSessionOutput, ForwardedPacket, PacketLoopEffects,
                PacketLoopForwardEffectKind, PacketLoopRelayTargetFixture, PacketLoopScratch,
                PacketLoopTime, PacketLoopTurn, PacketLoopTurnInput, RtcBootstrapState,
                install_local_destination_fixture, install_relay_target_fixture,
                install_source_fixture, sample_forwarded_packet,
            },
        },
    },
};

pub const DENSE_LOCAL_DESTINATIONS: usize = 64;
pub const RELAY_DESTINATIONS: usize = 32;
pub const WARMUP_ITERATIONS: usize = 512;
pub const SAMPLE_COUNT: usize = 32;
pub const SAMPLE_ITERATIONS: usize = 512;
pub const ALLOCATION_PROFILE_ITERATIONS: usize = 1024;

const SOURCE_MID: &str = "bench-up";
const DESTINATION_MID: &str = "bench-down";
const PACKET_PAYLOAD: &[u8] = b"benchmark-payload";

pub type BenchmarkResult<T = ()> = Result<T, BenchmarkFailure>;

#[derive(Debug, Clone, Copy)]
pub struct BenchmarkFailure {
    message: &'static str,
}

impl BenchmarkFailure {
    pub const fn new(message: &'static str) -> Self {
        Self { message }
    }
}

impl Display for BenchmarkFailure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message)
    }
}

impl Error for BenchmarkFailure {}

#[derive(Debug, Clone, Copy)]
pub struct ExpectedEffects {
    pub local_forwards: usize,
    pub intra_node_relay_forwards: usize,
    pub incoming_bitrate_effects: usize,
}

pub struct BenchmarkTurn {
    state: RtcBootstrapState,
    scratch: PacketLoopScratch,
    effects: PacketLoopEffects,
    session_outputs: Vec<DrainedSessionOutput>,
    relay_packets: Vec<ForwardedPacket>,
    source_session: TransportSessionKey,
    source_transport_media_id: TransportMediaId,
    relay_target_handles: Vec<PacketLoopRelayTargetFixture>,
}

pub fn one_ingress_turn() -> BenchmarkTurn {
    let mut turn = BenchmarkTurn::new(1, 1);
    turn.add_local_destinations(1);
    turn
}

pub fn dense_local_fanout_turn() -> BenchmarkTurn {
    let mut turn = BenchmarkTurn::new(2, 1);
    turn.add_local_destinations(DENSE_LOCAL_DESTINATIONS);
    turn
}

pub fn relay_fanout_turn() -> BenchmarkTurn {
    let mut turn = BenchmarkTurn::new(3, 1);
    turn.add_relay_targets(RELAY_DESTINATIONS);
    turn
}

pub fn run_sampled_idle_benchmark(name: &'static str) -> BenchmarkResult {
    let mut turn = BenchmarkTurn::new(4, 1);
    turn.step(0);
    turn.warm_idle(WARMUP_ITERATIONS);
    let mut samples = Vec::with_capacity(SAMPLE_COUNT);
    for _ in 0..SAMPLE_COUNT {
        let start = Instant::now();
        for _ in 0..SAMPLE_ITERATIONS {
            turn.step(0);
            black_box(turn.effect_count());
        }
        samples.push(nanos_per_iteration(start.elapsed(), SAMPLE_ITERATIONS));
    }
    let stats = BenchmarkStats::from_samples(samples)?;
    report_benchmark(name, &stats);
    Ok(())
}

pub fn run_sampled_turn_benchmark(
    name: &'static str,
    turn: &mut BenchmarkTurn,
    expected: ExpectedEffects,
) -> BenchmarkResult {
    turn.verify_next_turn(expected)?;
    turn.warm(WARMUP_ITERATIONS);
    let mut samples = Vec::with_capacity(SAMPLE_COUNT);
    for _ in 0..SAMPLE_COUNT {
        let packets = turn.packet_batch(SAMPLE_ITERATIONS);
        let start = Instant::now();
        turn.run_packets(packets);
        samples.push(nanos_per_iteration(start.elapsed(), SAMPLE_ITERATIONS));
    }
    let stats = BenchmarkStats::from_samples(samples)?;
    report_benchmark(name, &stats);
    Ok(())
}

pub fn run_sampled_operation(name: &'static str, mut operation: impl FnMut()) -> BenchmarkResult {
    for _ in 0..WARMUP_ITERATIONS {
        operation();
    }
    let mut samples = Vec::with_capacity(SAMPLE_COUNT);
    for _ in 0..SAMPLE_COUNT {
        let start = Instant::now();
        for _ in 0..SAMPLE_ITERATIONS {
            operation();
        }
        samples.push(nanos_per_iteration(start.elapsed(), SAMPLE_ITERATIONS));
    }
    let stats = BenchmarkStats::from_samples(samples)?;
    report_benchmark(name, &stats);
    Ok(())
}

impl BenchmarkTurn {
    fn new(room_instance_id: u64, source_connection_id: u64) -> Self {
        let mut state = RtcBootstrapState::default();
        let source_session = session_key(room_instance_id, source_connection_id);
        let source_transport_media_id =
            install_source_fixture(&mut state, source_session.clone(), SOURCE_MID);
        Self {
            state,
            scratch: PacketLoopScratch::new(),
            effects: PacketLoopEffects::default(),
            session_outputs: Vec::with_capacity(1),
            relay_packets: Vec::with_capacity(1),
            source_session,
            source_transport_media_id,
            relay_target_handles: Vec::new(),
        }
    }

    fn add_local_destinations(&mut self, destination_count: usize) {
        for idx in 0..destination_count {
            let connection_id = 10_000_u64.saturating_add(u64::try_from(idx).unwrap_or(u64::MAX));
            let session_key = session_key(
                self.source_session.room_instance_id().as_u64(),
                connection_id,
            );
            let _ = install_local_destination_fixture(
                &mut self.state,
                self.source_transport_media_id,
                session_key,
                DESTINATION_MID,
            );
        }
    }

    fn add_relay_targets(&mut self, target_count: usize) {
        self.relay_target_handles.reserve(target_count);
        for idx in 0..target_count {
            self.relay_target_handles.push(install_relay_target_fixture(
                &mut self.state,
                self.source_transport_media_id,
                u64::try_from(idx).unwrap_or(u64::MAX),
            ));
        }
    }

    pub fn verify_next_turn(&mut self, expected: ExpectedEffects) -> BenchmarkResult {
        let packet = self.next_packet();
        self.run_one_packet(packet);
        if self.effects.invalid_reference_count(&self.scratch) != 0 {
            return Err(BenchmarkFailure::new(
                "packet-loop benchmark emitted invalid scratch references",
            ));
        }
        if self
            .effects
            .forward_effect_count_by_kind(PacketLoopForwardEffectKind::LocalRtc)
            != expected.local_forwards
        {
            return Err(BenchmarkFailure::new(
                "packet-loop benchmark did not emit the expected local fanout",
            ));
        }
        if self
            .effects
            .forward_effect_count_by_kind(PacketLoopForwardEffectKind::IntraNodeRelay)
            != expected.intra_node_relay_forwards
        {
            return Err(BenchmarkFailure::new(
                "packet-loop benchmark did not emit the expected relay fanout",
            ));
        }
        if self.effects.incoming_bitrate_effect_count() != expected.incoming_bitrate_effects {
            return Err(BenchmarkFailure::new(
                "packet-loop benchmark did not observe the expected ingress packet",
            ));
        }
        let expected_forwards = expected.local_forwards + expected.intra_node_relay_forwards;
        if self.effects.forward_effect_count() != expected_forwards {
            return Err(BenchmarkFailure::new(
                "packet-loop benchmark emitted an unexpected forwarding effect count",
            ));
        }
        Ok(())
    }

    pub fn warm(&mut self, iteration_count: usize) {
        let packets = self.packet_batch(iteration_count);
        self.run_packets(packets);
    }

    fn warm_idle(&mut self, iteration_count: usize) {
        for _ in 0..iteration_count {
            self.step(0);
        }
    }

    pub fn packet_batch(&self, count: usize) -> Vec<ForwardedPacket> {
        let mut packets = Vec::with_capacity(count);
        for _ in 0..count {
            packets.push(self.next_packet());
        }
        packets
    }

    pub fn run_packets(&mut self, packets: Vec<ForwardedPacket>) {
        for packet in packets {
            self.run_one_packet(packet);
            black_box(self.effect_count());
        }
    }

    fn run_one_packet(&mut self, packet: ForwardedPacket) {
        self.relay_packets.push(packet);
        self.step(0);
    }

    fn step(&mut self, now_ms: u64) {
        PacketLoopTurn::step(
            &mut self.state,
            &mut self.scratch,
            &mut self.effects,
            PacketLoopTurnInput::without_packet_sinks(
                PacketLoopTime::from_millis(now_ms),
                &mut self.session_outputs,
                &mut self.relay_packets,
            ),
        );
    }

    fn effect_count(&self) -> usize {
        self.effects.effect_count()
    }

    fn next_packet(&self) -> ForwardedPacket {
        sample_forwarded_packet(self.source_session.clone(), SOURCE_MID, PACKET_PAYLOAD)
    }
}

pub fn warm_scratch(scratch: &mut PacketLoopScratch, item_count: u64, payload: &[u8]) {
    for idx in 0..item_count {
        let port = u16::try_from(idx).map_or(u16::MAX, |value| 40_000_u16.saturating_add(value));
        scratch.push_pending_transmit(SocketAddr::from(([127, 0, 0, 1], port)), payload);
        scratch.mark_source_policy_dirty(RoomInstanceId::from_raw(idx));
    }
}

fn session_key(room_instance_id: u64, connection_id: u64) -> TransportSessionKey {
    TransportSessionKey::new(
        RoomInstanceId::from_raw(room_instance_id),
        0,
        ConnectionId::from_raw(connection_id),
        UserId::Integer(i64::try_from(connection_id).unwrap_or(i64::MAX)),
    )
}

fn nanos_per_iteration(duration: Duration, iterations: usize) -> u128 {
    let iterations = u128::from(u64::try_from(iterations).unwrap_or(u64::MAX));
    duration.as_nanos() / iterations.max(1)
}

struct BenchmarkStats {
    min: u128,
    mean: u128,
    median: u128,
    p95: u128,
    max: u128,
}

impl BenchmarkStats {
    fn from_samples(mut samples: Vec<u128>) -> BenchmarkResult<Self> {
        if samples.is_empty() {
            return Err(BenchmarkFailure::new(
                "benchmark harness produced no timing samples",
            ));
        }
        samples.sort_unstable();
        let total = samples.iter().copied().fold(0_u128, u128::saturating_add);
        let len = samples.len();
        let mean_ns = total / u128::from(u64::try_from(len).unwrap_or(u64::MAX)).max(1);
        Ok(Self {
            min: samples.first().copied().unwrap_or(0),
            mean: mean_ns,
            median: percentile(&samples, 50),
            p95: percentile(&samples, 95),
            max: samples.last().copied().unwrap_or(0),
        })
    }
}

fn percentile(samples: &[u128], percentile: usize) -> u128 {
    let last_idx = samples.len().saturating_sub(1);
    let idx = last_idx
        .saturating_mul(percentile.min(100))
        .saturating_add(99)
        / 100;
    samples.get(idx).copied().unwrap_or(0)
}

fn report_benchmark(name: &str, stats: &BenchmarkStats) {
    println!(
        "{name}: min={}ns mean={}ns median={}ns p95={}ns max={}ns samples={} iterations_per_sample={}",
        stats.min, stats.mean, stats.median, stats.p95, stats.max, SAMPLE_COUNT, SAMPLE_ITERATIONS
    );
}
