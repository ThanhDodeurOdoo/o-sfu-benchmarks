use std::{collections::BTreeMap, hint::black_box};

use o_sfu::testing::{
    concurrency::SourcePolicyDirtyState,
    transport::{SessionId, TransportSessionKey, test_transport_session_key},
};

const CHANNEL_INSTANCE_ID: u64 = 11;
const MEDIA_WORKER_ID: usize = 0;

#[derive(Debug)]
pub struct SourcePolicyWakeBenchmarkFixture {
    dirty: SourcePolicyDirtyState,
    channel_ids: Vec<u64>,
    scratch: Vec<u64>,
}

impl SourcePolicyWakeBenchmarkFixture {
    #[must_use]
    pub fn new(mark_count: usize) -> Self {
        let mut channel_ids = Vec::with_capacity(mark_count);
        for index in 0..mark_count {
            channel_ids.push(u64::try_from(index % 8).unwrap_or(0));
        }
        Self {
            dirty: SourcePolicyDirtyState::default(),
            channel_ids,
            scratch: Vec::with_capacity(mark_count),
        }
    }

    #[must_use]
    pub fn coalesced_worker_buffer_cycle(&mut self) -> usize {
        self.scratch.clear();
        self.scratch.extend_from_slice(&self.channel_ids);
        self.scratch.sort_unstable();
        self.scratch.dedup();
        for _channel_id in &self.scratch {
            self.dirty.mark_dirty();
        }
        let woke = usize::from(self.dirty.take_dirty());
        black_box(self.scratch.len().saturating_add(woke))
    }
}

#[derive(Debug, Clone, Copy)]
enum BenchmarkKeyframeKind {
    Pli,
    Fir,
}

impl BenchmarkKeyframeKind {
    const fn coalesce(self, other: Self) -> Self {
        match (self, other) {
            (Self::Fir, _) | (_, Self::Fir) => Self::Fir,
            _ => Self::Pli,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct PendingKeyframeRequest {
    source_media_id: u64,
    rid: Option<u16>,
    kind: BenchmarkKeyframeKind,
}

#[derive(Debug, Clone, Copy)]
struct CoalescedKeyframeRequest {
    source_media_id: u64,
    rid: Option<u16>,
    kind: BenchmarkKeyframeKind,
}

impl CoalescedKeyframeRequest {
    fn coalesce(&mut self, request: PendingKeyframeRequest) {
        self.rid = self.rid.or(request.rid);
        self.kind = self.kind.coalesce(request.kind);
    }
}

#[derive(Debug)]
pub struct KeyframeCoalescingBenchmarkFixture {
    requests: Vec<PendingKeyframeRequest>,
    scratch: Vec<CoalescedKeyframeRequest>,
}

impl KeyframeCoalescingBenchmarkFixture {
    #[must_use]
    pub fn new(request_count: usize) -> Self {
        let mut requests = Vec::with_capacity(request_count);
        for index in 0..request_count {
            requests.push(PendingKeyframeRequest {
                source_media_id: u64::try_from(index % 16).unwrap_or(0),
                rid: (index % 3 == 0).then_some(u16::try_from(index % 4).unwrap_or(0)),
                kind: if index % 11 == 0 {
                    BenchmarkKeyframeKind::Fir
                } else {
                    BenchmarkKeyframeKind::Pli
                },
            });
        }
        Self {
            requests,
            scratch: Vec::with_capacity(request_count),
        }
    }

    #[must_use]
    pub fn reusable_vec_coalescing_cycle(&mut self) -> usize {
        self.scratch.clear();
        self.scratch.extend(
            self.requests
                .iter()
                .map(|request| CoalescedKeyframeRequest {
                    source_media_id: request.source_media_id,
                    rid: request.rid,
                    kind: request.kind,
                }),
        );
        self.scratch.sort_by_key(|request| request.source_media_id);
        let mut coalesced_count = 0_usize;
        let mut current_request: Option<CoalescedKeyframeRequest> = None;
        for request in &self.scratch {
            match &mut current_request {
                Some(current) if current.source_media_id == request.source_media_id => {
                    current.coalesce(PendingKeyframeRequest {
                        source_media_id: request.source_media_id,
                        rid: request.rid,
                        kind: request.kind,
                    });
                }
                Some(_) => {
                    coalesced_count = coalesced_count.saturating_add(1);
                    current_request = Some(*request);
                }
                None => {
                    current_request = Some(*request);
                }
            }
        }
        if current_request.is_some() {
            coalesced_count = coalesced_count.saturating_add(1);
        }
        black_box(coalesced_count)
    }

    #[must_use]
    pub fn fresh_btree_coalescing_cycle(&self) -> usize {
        let mut coalesced = BTreeMap::<u64, CoalescedKeyframeRequest>::new();
        for request in &self.requests {
            coalesced
                .entry(request.source_media_id)
                .and_modify(|coalesced_request| coalesced_request.coalesce(*request))
                .or_insert(CoalescedKeyframeRequest {
                    source_media_id: request.source_media_id,
                    rid: request.rid,
                    kind: request.kind,
                });
        }
        black_box(coalesced.len())
    }
}

#[derive(Debug, Clone, Copy)]
enum PacketGate {
    Open,
    Rid(u8),
}

impl PacketGate {
    fn permits(self, packet: RoutePacket) -> bool {
        match self {
            Self::Open => true,
            Self::Rid(rid) => packet.rid == Some(rid),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct RoutePacket {
    rid: Option<u8>,
}

#[derive(Debug, Clone)]
struct RouteDestination {
    session_key: TransportSessionKey,
    gate: PacketGate,
    active: bool,
}

#[derive(Debug)]
pub struct RoutePlanningBenchmarkFixture {
    packets: Vec<RoutePacket>,
    destinations: Vec<RouteDestination>,
}

impl RoutePlanningBenchmarkFixture {
    #[must_use]
    pub fn new(packet_count: usize, destination_count: usize) -> Self {
        let mut packets = Vec::with_capacity(packet_count);
        for index in 0..packet_count {
            packets.push(RoutePacket {
                rid: Some(u8::try_from(index % 3).unwrap_or(0)),
            });
        }
        let mut destinations = Vec::with_capacity(destination_count);
        for index in 0..destination_count {
            destinations.push(RouteDestination {
                session_key: benchmark_session_key(index),
                gate: if index % 4 == 0 {
                    PacketGate::Open
                } else {
                    PacketGate::Rid(u8::try_from(index % 3).unwrap_or(0))
                },
                active: index % 7 != 0,
            });
        }
        Self {
            packets,
            destinations,
        }
    }

    #[must_use]
    pub fn destination_gate_cycle(&self) -> usize {
        let mut route_count = 0_usize;
        for packet in &self.packets {
            for destination in &self.destinations {
                black_box(&destination.session_key);
                if destination.active && destination.gate.permits(*packet) {
                    route_count = route_count.saturating_add(1);
                }
            }
        }
        black_box(route_count)
    }
}

#[derive(Debug)]
pub struct LocalEgressFanoutBenchmarkFixture {
    destinations: Vec<RouteDestination>,
    payload: Vec<u8>,
    scratch: Vec<(TransportSessionKey, usize)>,
}

impl LocalEgressFanoutBenchmarkFixture {
    #[must_use]
    pub fn new(destination_count: usize) -> Self {
        let route_planning = RoutePlanningBenchmarkFixture::new(1, destination_count);
        Self {
            destinations: route_planning.destinations,
            payload: vec![0; 1200],
            scratch: Vec::with_capacity(destination_count),
        }
    }

    #[must_use]
    pub fn shared_payload_planning_cycle(&mut self) -> usize {
        self.scratch.clear();
        for destination in &self.destinations {
            if destination.active {
                self.scratch
                    .push((destination.session_key.clone(), self.payload.len()));
            }
        }
        black_box(self.scratch.len())
    }
}

fn benchmark_session_key(index: usize) -> TransportSessionKey {
    test_transport_session_key(
        CHANNEL_INSTANCE_ID,
        MEDIA_WORKER_ID,
        u64::try_from(index).unwrap_or(0).saturating_add(1),
        SessionId::Integer(i64::try_from(index).unwrap_or(0).saturating_add(1)),
    )
}
