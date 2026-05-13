use std::hint::black_box;

use o_sfu_core::{
    ConnectionId, RoomInstanceId,
    server::{session::UserId, transport::TransportSessionKey},
};

const ROOM_INSTANCE_ID: u64 = 17;
const MEDIA_WORKER_ID: usize = 0;
const FIRST_CONNECTION_ID: u64 = 100;

#[derive(Debug, Clone, Copy)]
enum IngressEventKind {
    CachedRoute,
    Malformed,
    RecentMiss,
    SourceRateLimited,
    RecoveryMiss,
    RecoveryHit,
}

#[derive(Debug, Clone, Copy)]
struct IngressEvent {
    kind: IngressEventKind,
    session_index: u16,
    payload_len: u16,
    packet_fingerprint: u64,
}

#[derive(Debug, Default)]
struct IngressAccounting {
    routed: u64,
    dropped: u64,
    recovered: u64,
    diagnostics: u64,
}

#[derive(Debug)]
pub struct IngressColdPathBenchmarkFixture {
    events: Vec<IngressEvent>,
    sessions: Vec<TransportSessionKey>,
}

impl IngressColdPathBenchmarkFixture {
    #[must_use]
    pub fn new(event_count: usize) -> Self {
        let session_count = 256_usize;
        let mut sessions = Vec::with_capacity(session_count);
        for index in 0..session_count {
            sessions.push(benchmark_session_key(index));
        }
        let mut events = Vec::with_capacity(event_count);
        for index in 0..event_count {
            let kind = match index {
                value if value % 997 == 0 => IngressEventKind::Malformed,
                value if value % 761 == 0 => IngressEventKind::SourceRateLimited,
                value if value % 509 == 0 => IngressEventKind::RecoveryMiss,
                value if value % 251 == 0 => IngressEventKind::RecoveryHit,
                value if value % 193 == 0 => IngressEventKind::RecentMiss,
                _ => IngressEventKind::CachedRoute,
            };
            events.push(IngressEvent {
                kind,
                session_index: u16::try_from(index % session_count).unwrap_or(0),
                payload_len: u16::try_from(96 + index % 1200).unwrap_or(u16::MAX),
                packet_fingerprint: packet_fingerprint(index),
            });
        }
        Self { events, sessions }
    }

    #[must_use]
    pub fn event_count_u64(&self) -> u64 {
        u64::try_from(self.events.len()).unwrap_or(u64::MAX)
    }

    #[must_use]
    pub fn inline_rare_branch_cycle(&self) -> u64 {
        let mut accounting = IngressAccounting::default();
        for event in black_box(&self.events) {
            match event.kind {
                IngressEventKind::CachedRoute => {
                    account_cached_route(&mut accounting, event, &self.sessions);
                }
                IngressEventKind::Malformed => {
                    accounting.dropped = accounting.dropped.wrapping_add(1);
                    accounting.diagnostics = accounting
                        .diagnostics
                        .wrapping_add(event.packet_fingerprint.rotate_left(7));
                }
                IngressEventKind::RecentMiss => {
                    accounting.dropped = accounting.dropped.wrapping_add(1);
                    accounting.diagnostics = accounting
                        .diagnostics
                        .wrapping_add(event.packet_fingerprint.rotate_left(13));
                }
                IngressEventKind::SourceRateLimited => {
                    accounting.dropped = accounting.dropped.wrapping_add(1);
                    accounting.diagnostics = accounting
                        .diagnostics
                        .wrapping_add(event.packet_fingerprint.rotate_left(19));
                }
                IngressEventKind::RecoveryMiss => {
                    accounting.dropped = accounting.dropped.wrapping_add(1);
                    accounting.diagnostics = accounting
                        .diagnostics
                        .wrapping_add(event.packet_fingerprint.rotate_left(23));
                }
                IngressEventKind::RecoveryHit => {
                    accounting.recovered = accounting.recovered.wrapping_add(1);
                    account_cached_route(&mut accounting, event, &self.sessions);
                }
            }
        }
        black_box(finish_ingress_accounting(accounting))
    }

    #[must_use]
    pub fn cold_rare_branch_cycle(&self) -> u64 {
        let mut accounting = IngressAccounting::default();
        for event in black_box(&self.events) {
            match event.kind {
                IngressEventKind::CachedRoute => {
                    account_cached_route(&mut accounting, event, &self.sessions);
                }
                IngressEventKind::Malformed => {
                    account_malformed_datagram(&mut accounting, event);
                }
                IngressEventKind::RecentMiss => {
                    account_recent_miss_drop(&mut accounting, event);
                }
                IngressEventKind::SourceRateLimited => {
                    account_source_rate_limited_drop(&mut accounting, event);
                }
                IngressEventKind::RecoveryMiss => {
                    account_recovery_miss_drop(&mut accounting, event);
                }
                IngressEventKind::RecoveryHit => {
                    account_recovery_hit(&mut accounting, event, &self.sessions);
                }
            }
        }
        black_box(finish_ingress_accounting(accounting))
    }
}

#[derive(Debug, Clone, Copy)]
enum PacketGate {
    Open,
    Rid(u8),
}

impl PacketGate {
    #[inline(always)]
    fn permits(self, packet_rid: u8) -> bool {
        match self {
            Self::Open => true,
            Self::Rid(rid) => rid == packet_rid,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct RoutePacket {
    rid: u8,
    source_media_id: u32,
}

#[derive(Debug, Clone)]
struct RouteDestination {
    session_key: TransportSessionKey,
    gate: PacketGate,
    active: bool,
}

#[derive(Debug, Default)]
struct RouteAccounting {
    planned: u64,
    rejected: u64,
    checksum: u64,
}

#[derive(Debug)]
pub struct RoutePlanningColdPathBenchmarkFixture {
    packets: Vec<RoutePacket>,
    destinations: Vec<RouteDestination>,
}

impl RoutePlanningColdPathBenchmarkFixture {
    #[must_use]
    pub fn new(packet_count: usize, destination_count: usize) -> Self {
        let mut packets = Vec::with_capacity(packet_count);
        for index in 0..packet_count {
            packets.push(RoutePacket {
                rid: u8::try_from(index % 3).unwrap_or(0),
                source_media_id: u32::try_from(index % 64).unwrap_or(0),
            });
        }
        let mut destinations = Vec::with_capacity(destination_count);
        for index in 0..destination_count {
            let active = index % 97 != 0;
            let gate = if index % 43 == 0 {
                PacketGate::Rid(7)
            } else if index % 11 == 0 {
                PacketGate::Rid(u8::try_from(index % 3).unwrap_or(0))
            } else {
                PacketGate::Open
            };
            destinations.push(RouteDestination {
                session_key: benchmark_session_key(index),
                gate,
                active,
            });
        }
        Self {
            packets,
            destinations,
        }
    }

    #[must_use]
    pub fn route_attempt_count_u64(&self) -> u64 {
        let attempts = self.packets.len().saturating_mul(self.destinations.len());
        u64::try_from(attempts).unwrap_or(u64::MAX)
    }

    #[must_use]
    pub fn inline_reject_branch_cycle(&self) -> u64 {
        let mut accounting = RouteAccounting::default();
        for packet in black_box(&self.packets) {
            for destination in &self.destinations {
                if !destination.active {
                    accounting.rejected = accounting.rejected.wrapping_add(1);
                    accounting.checksum = accounting.checksum.wrapping_add(
                        session_checksum(&destination.session_key) ^ u64::from(packet.rid),
                    );
                    continue;
                }
                if !destination.gate.permits(packet.rid) {
                    accounting.rejected = accounting.rejected.wrapping_add(1);
                    accounting.checksum = accounting.checksum.wrapping_add(
                        session_checksum(&destination.session_key)
                            ^ u64::from(packet.source_media_id),
                    );
                    continue;
                }
                account_planned_route(&mut accounting, packet, destination);
            }
        }
        black_box(finish_route_accounting(accounting))
    }

    #[must_use]
    pub fn cold_reject_branch_cycle(&self) -> u64 {
        let mut accounting = RouteAccounting::default();
        for packet in black_box(&self.packets) {
            for destination in &self.destinations {
                if destination.active && destination.gate.permits(packet.rid) {
                    account_planned_route(&mut accounting, packet, destination);
                } else {
                    account_rejected_route(&mut accounting, packet, destination);
                }
            }
        }
        black_box(finish_route_accounting(accounting))
    }
}

#[derive(Debug, Clone, Copy)]
struct StatsPacket {
    payload_len: u16,
    room_slot: u16,
    first_ingress: bool,
    audio_policy_changed: bool,
    selected_rid_activated: bool,
    video_source: bool,
}

#[derive(Debug, Default)]
struct StatsAccounting {
    bytes: u64,
    dirty_rooms: u64,
    keyframes: u64,
    checksum: u64,
}

#[derive(Debug)]
pub struct IncomingStatsColdPathBenchmarkFixture {
    packets: Vec<StatsPacket>,
}

impl IncomingStatsColdPathBenchmarkFixture {
    #[must_use]
    pub fn new(packet_count: usize) -> Self {
        let mut packets = Vec::with_capacity(packet_count);
        for index in 0..packet_count {
            packets.push(StatsPacket {
                payload_len: u16::try_from(120 + index % 1180).unwrap_or(u16::MAX),
                room_slot: u16::try_from(index % 64).unwrap_or(0),
                first_ingress: index % 1021 == 0,
                audio_policy_changed: index % 173 == 0,
                selected_rid_activated: index % 17 == 0,
                video_source: index % 5 != 0,
            });
        }
        Self { packets }
    }

    #[must_use]
    pub fn packet_count_u64(&self) -> u64 {
        u64::try_from(self.packets.len()).unwrap_or(u64::MAX)
    }

    #[must_use]
    pub fn inline_observation_cycle(&self) -> u64 {
        let mut accounting = StatsAccounting::default();
        for packet in black_box(&self.packets) {
            accounting.bytes = accounting.bytes.wrapping_add(u64::from(packet.payload_len));
            if packet.audio_policy_changed {
                accounting.dirty_rooms = accounting.dirty_rooms.wrapping_add(1);
                accounting.checksum = accounting
                    .checksum
                    .wrapping_add(u64::from(packet.room_slot).rotate_left(5));
            }
            if packet.first_ingress {
                accounting.checksum = accounting
                    .checksum
                    .wrapping_add(u64::from(packet.payload_len).rotate_left(11));
                if packet.video_source && !packet.selected_rid_activated {
                    accounting.keyframes = accounting.keyframes.wrapping_add(1);
                }
            }
        }
        black_box(finish_stats_accounting(accounting))
    }

    #[must_use]
    pub fn cold_observation_cycle(&self) -> u64 {
        let mut accounting = StatsAccounting::default();
        for packet in black_box(&self.packets) {
            accounting.bytes = accounting.bytes.wrapping_add(u64::from(packet.payload_len));
            if packet.audio_policy_changed {
                account_audio_policy_change(&mut accounting, packet);
            }
            if packet.first_ingress {
                account_first_ingress(&mut accounting, packet);
            }
        }
        black_box(finish_stats_accounting(accounting))
    }
}

#[inline(always)]
fn account_cached_route(
    accounting: &mut IngressAccounting,
    event: &IngressEvent,
    sessions: &[TransportSessionKey],
) {
    let session_idx = usize::from(event.session_index) % sessions.len();
    accounting.routed = accounting.routed.wrapping_add(1);
    accounting.diagnostics = accounting.diagnostics.wrapping_add(
        session_checksum(&sessions[session_idx])
            ^ event.packet_fingerprint
            ^ u64::from(event.payload_len),
    );
}

#[cold]
#[inline(never)]
fn account_malformed_datagram(accounting: &mut IngressAccounting, event: &IngressEvent) {
    accounting.dropped = accounting.dropped.wrapping_add(1);
    accounting.diagnostics = accounting
        .diagnostics
        .wrapping_add(event.packet_fingerprint.rotate_left(7));
}

#[cold]
#[inline(never)]
fn account_recent_miss_drop(accounting: &mut IngressAccounting, event: &IngressEvent) {
    accounting.dropped = accounting.dropped.wrapping_add(1);
    accounting.diagnostics = accounting
        .diagnostics
        .wrapping_add(event.packet_fingerprint.rotate_left(13));
}

#[cold]
#[inline(never)]
fn account_source_rate_limited_drop(accounting: &mut IngressAccounting, event: &IngressEvent) {
    accounting.dropped = accounting.dropped.wrapping_add(1);
    accounting.diagnostics = accounting
        .diagnostics
        .wrapping_add(event.packet_fingerprint.rotate_left(19));
}

#[cold]
#[inline(never)]
fn account_recovery_miss_drop(accounting: &mut IngressAccounting, event: &IngressEvent) {
    accounting.dropped = accounting.dropped.wrapping_add(1);
    accounting.diagnostics = accounting
        .diagnostics
        .wrapping_add(event.packet_fingerprint.rotate_left(23));
}

#[cold]
#[inline(never)]
fn account_recovery_hit(
    accounting: &mut IngressAccounting,
    event: &IngressEvent,
    sessions: &[TransportSessionKey],
) {
    accounting.recovered = accounting.recovered.wrapping_add(1);
    account_cached_route(accounting, event, sessions);
}

#[inline(always)]
fn account_planned_route(
    accounting: &mut RouteAccounting,
    packet: &RoutePacket,
    destination: &RouteDestination,
) {
    accounting.planned = accounting.planned.wrapping_add(1);
    accounting.checksum = accounting.checksum.wrapping_add(
        session_checksum(&destination.session_key)
            ^ u64::from(packet.source_media_id)
            ^ u64::from(packet.rid),
    );
}

#[cold]
#[inline(never)]
fn account_rejected_route(
    accounting: &mut RouteAccounting,
    packet: &RoutePacket,
    destination: &RouteDestination,
) {
    accounting.rejected = accounting.rejected.wrapping_add(1);
    accounting.checksum = accounting.checksum.wrapping_add(
        session_checksum(&destination.session_key)
            ^ u64::from(packet.source_media_id)
            ^ u64::from(packet.rid),
    );
}

#[cold]
#[inline(never)]
fn account_audio_policy_change(accounting: &mut StatsAccounting, packet: &StatsPacket) {
    accounting.dirty_rooms = accounting.dirty_rooms.wrapping_add(1);
    accounting.checksum = accounting
        .checksum
        .wrapping_add(u64::from(packet.room_slot).rotate_left(5));
}

#[cold]
#[inline(never)]
fn account_first_ingress(accounting: &mut StatsAccounting, packet: &StatsPacket) {
    accounting.checksum = accounting
        .checksum
        .wrapping_add(u64::from(packet.payload_len).rotate_left(11));
    if packet.video_source && !packet.selected_rid_activated {
        accounting.keyframes = accounting.keyframes.wrapping_add(1);
    }
}

fn finish_ingress_accounting(accounting: IngressAccounting) -> u64 {
    accounting.routed
        ^ accounting.dropped.rotate_left(7)
        ^ accounting.recovered.rotate_left(13)
        ^ accounting.diagnostics
}

fn finish_route_accounting(accounting: RouteAccounting) -> u64 {
    accounting.planned ^ accounting.rejected.rotate_left(17) ^ accounting.checksum
}

fn finish_stats_accounting(accounting: StatsAccounting) -> u64 {
    accounting.bytes
        ^ accounting.dirty_rooms.rotate_left(3)
        ^ accounting.keyframes.rotate_left(29)
        ^ accounting.checksum
}

fn session_checksum(session_key: &TransportSessionKey) -> u64 {
    let user_key = match session_key.user_id() {
        UserId::Integer(value) => u64::try_from(*value).unwrap_or(0),
        UserId::String(value) => u64::try_from(value.len()).unwrap_or(u64::MAX),
    };
    session_key
        .room_instance_id()
        .as_u64()
        .wrapping_add(session_key.room_instance_id().as_u64().rotate_left(7))
        .wrapping_add(u64::try_from(session_key.media_worker_id()).unwrap_or(u64::MAX))
        .wrapping_add(user_key.rotate_left(11))
}

fn benchmark_session_key(index: usize) -> TransportSessionKey {
    let connection_id = FIRST_CONNECTION_ID.saturating_add(u64::try_from(index).unwrap_or(0));
    TransportSessionKey::new(
        RoomInstanceId::from_raw(ROOM_INSTANCE_ID),
        MEDIA_WORKER_ID,
        ConnectionId::from_raw(connection_id),
        UserId::Integer(i64::try_from(index).unwrap_or(0)),
    )
}

fn packet_fingerprint(index: usize) -> u64 {
    let value = u64::try_from(index).unwrap_or(u64::MAX);
    value
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .rotate_left(u32::try_from(index % 63).unwrap_or(0))
}
