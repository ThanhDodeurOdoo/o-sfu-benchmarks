use std::{collections::BTreeMap, hint::black_box};

use o_sfu_core::{
    ConnectionId, RoomInstanceId,
    server::{
        session::UserId,
        transport::{TransportMediaId, TransportSessionKey},
    },
};

const CHANNEL_INSTANCE_ID: u64 = 21;
const MEDIA_WORKER_ID: usize = 0;
const SOURCE_MEDIA_ID: u64 = 100;
const FIRST_CONSUMER_MEDIA_ID: u64 = 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BenchmarkPacketGate {
    Open,
    Rid(u8),
    Block,
}

#[derive(Debug, Clone)]
struct BenchmarkRouteDestination {
    session_key: TransportSessionKey,
    media_id: TransportMediaId,
    active: bool,
    gate: BenchmarkPacketGate,
    pending_gate: Option<BenchmarkPacketGate>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct IdentityLookupKey {
    session_key: TransportSessionKey,
    media_id: TransportMediaId,
}

#[derive(Debug, Clone)]
struct BenchmarkGateUpdate {
    session_key: TransportSessionKey,
    media_id: TransportMediaId,
    identity_key: IdentityLookupKey,
}

#[derive(Debug)]
pub struct ConsumerPacketGateBatchBenchmarkFixture {
    destinations: Vec<BenchmarkRouteDestination>,
    media_id_index: BTreeMap<TransportMediaId, usize>,
    identity_index: BTreeMap<IdentityLookupKey, usize>,
    updates: Vec<BenchmarkGateUpdate>,
    next_gate_is_open: bool,
}

impl ConsumerPacketGateBatchBenchmarkFixture {
    #[must_use]
    pub fn new(destination_count: usize) -> Self {
        let destinations = benchmark_destinations(destination_count);
        let media_id_index = media_id_index(&destinations);
        let identity_index = identity_index(&destinations);
        let updates = benchmark_updates(&destinations);
        Self {
            destinations,
            media_id_index,
            identity_index,
            updates,
            next_gate_is_open: false,
        }
    }

    #[must_use]
    pub fn current_linear_batch_cycle(&mut self) -> usize {
        let gate = self.next_gate();
        let mut changed_count = 0_usize;
        let mut failed_count = 0_usize;
        for update in &self.updates {
            match self.destinations.iter_mut().find(|destination| {
                destination.session_key == update.session_key
                    && destination.media_id == update.media_id
            }) {
                Some(destination) => {
                    changed_count = changed_count
                        .saturating_add(usize::from(update_destination_gate(destination, gate)));
                }
                None => failed_count = failed_count.saturating_add(1),
            }
        }
        let aggregate = if changed_count == 0 {
            0
        } else {
            self.source_gate_aggregate()
        };
        black_box(
            changed_count
                .saturating_add(failed_count)
                .saturating_add(aggregate),
        )
    }

    #[must_use]
    pub fn media_id_indexed_batch_cycle(&mut self) -> usize {
        let gate = self.next_gate();
        let mut changed_count = 0_usize;
        let mut failed_count = 0_usize;
        for update in &self.updates {
            match self.media_id_index.get(&update.media_id).copied() {
                Some(position) if self.destinations[position].session_key == update.session_key => {
                    changed_count = changed_count.saturating_add(usize::from(
                        update_destination_gate(&mut self.destinations[position], gate),
                    ));
                }
                Some(_) | None => failed_count = failed_count.saturating_add(1),
            }
        }
        let aggregate = if changed_count == 0 {
            0
        } else {
            self.source_gate_aggregate()
        };
        black_box(
            changed_count
                .saturating_add(failed_count)
                .saturating_add(aggregate),
        )
    }

    #[must_use]
    pub fn identity_indexed_batch_cycle(&mut self) -> usize {
        let gate = self.next_gate();
        let mut changed_count = 0_usize;
        let mut failed_count = 0_usize;
        for update in &self.updates {
            match self.identity_index.get(&update.identity_key).copied() {
                Some(position) => {
                    changed_count = changed_count.saturating_add(usize::from(
                        update_destination_gate(&mut self.destinations[position], gate),
                    ));
                }
                None => failed_count = failed_count.saturating_add(1),
            }
        }
        let aggregate = if changed_count == 0 {
            0
        } else {
            self.source_gate_aggregate()
        };
        black_box(
            changed_count
                .saturating_add(failed_count)
                .saturating_add(aggregate),
        )
    }

    fn next_gate(&mut self) -> BenchmarkPacketGate {
        self.next_gate_is_open = !self.next_gate_is_open;
        if self.next_gate_is_open {
            BenchmarkPacketGate::Open
        } else {
            BenchmarkPacketGate::Rid(1)
        }
    }

    fn source_gate_aggregate(&self) -> usize {
        let mut open_count = 0_usize;
        let mut rid_count = 0_usize;
        for destination in &self.destinations {
            if !destination.active {
                continue;
            }
            match destination.gate {
                BenchmarkPacketGate::Open => open_count = open_count.saturating_add(1),
                BenchmarkPacketGate::Rid(_) => rid_count = rid_count.saturating_add(1),
                BenchmarkPacketGate::Block => {}
            }
        }
        open_count.saturating_add(rid_count)
    }
}

#[derive(Debug)]
pub struct ConsumerPacketGateRouteMaintenanceBenchmarkFixture {
    destinations: Vec<BenchmarkRouteDestination>,
    media_id_index: BTreeMap<TransportMediaId, usize>,
    removal_cursor: usize,
    next_destination_index: usize,
}

impl ConsumerPacketGateRouteMaintenanceBenchmarkFixture {
    #[must_use]
    pub fn new(destination_count: usize) -> Self {
        let destinations = benchmark_destinations(destination_count);
        let media_id_index = media_id_index(&destinations);
        Self {
            destinations,
            media_id_index,
            removal_cursor: 0,
            next_destination_index: destination_count,
        }
    }

    #[must_use]
    pub fn current_linear_remove_add_cycle(&mut self) -> usize {
        let target = self.next_removal_target();
        let Some(position) = self
            .destinations
            .iter()
            .position(|destination| destination.media_id == target.media_id)
        else {
            return black_box(0);
        };
        self.destinations.remove(position);
        self.destinations
            .push(benchmark_destination(self.next_destination_index));
        self.next_destination_index = self.next_destination_index.saturating_add(1);
        black_box(self.destinations.len())
    }

    #[must_use]
    pub fn media_id_indexed_swap_remove_add_cycle(&mut self) -> usize {
        let target = self.next_removal_target();
        let Some(position) = self.media_id_index.remove(&target.media_id) else {
            return black_box(0);
        };
        let removed_position = position;
        self.destinations.swap_remove(position);
        if removed_position < self.destinations.len() {
            self.media_id_index.insert(
                self.destinations[removed_position].media_id,
                removed_position,
            );
        }
        let new_position = self.destinations.len();
        let destination = benchmark_destination(self.next_destination_index);
        self.media_id_index
            .insert(destination.media_id, new_position);
        self.destinations.push(destination);
        self.next_destination_index = self.next_destination_index.saturating_add(1);
        black_box(self.destinations.len())
    }

    #[must_use]
    pub fn media_id_indexed_stable_remove_add_cycle(&mut self) -> usize {
        let target = self.next_removal_target();
        let Some(position) = self.media_id_index.remove(&target.media_id) else {
            return black_box(0);
        };
        self.destinations.remove(position);
        for shifted_position in position..self.destinations.len() {
            self.media_id_index.insert(
                self.destinations[shifted_position].media_id,
                shifted_position,
            );
        }
        let new_position = self.destinations.len();
        let destination = benchmark_destination(self.next_destination_index);
        self.media_id_index
            .insert(destination.media_id, new_position);
        self.destinations.push(destination);
        self.next_destination_index = self.next_destination_index.saturating_add(1);
        black_box(self.destinations.len())
    }

    fn next_removal_target(&mut self) -> BenchmarkRouteDestination {
        let position = self.removal_cursor % self.destinations.len();
        self.removal_cursor = self.removal_cursor.saturating_add(1);
        self.destinations[position].clone()
    }
}

#[must_use]
pub fn build_current_linear_route(destination_count: usize) -> usize {
    let mut destinations = Vec::with_capacity(destination_count);
    for index in 0..destination_count {
        destinations.push(benchmark_destination(index));
    }
    black_box(destinations.len())
}

#[must_use]
pub fn build_media_id_indexed_route(destination_count: usize) -> usize {
    let mut destinations = Vec::with_capacity(destination_count);
    let mut media_id_index = BTreeMap::new();
    for index in 0..destination_count {
        let position = destinations.len();
        let destination = benchmark_destination(index);
        media_id_index.insert(destination.media_id, position);
        destinations.push(destination);
    }
    black_box(media_id_index.len());
    black_box(destinations.len())
}

fn update_destination_gate(
    destination: &mut BenchmarkRouteDestination,
    gate: BenchmarkPacketGate,
) -> bool {
    let pending_gate = match gate {
        BenchmarkPacketGate::Rid(_) => Some(gate),
        BenchmarkPacketGate::Open | BenchmarkPacketGate::Block => None,
    };
    if destination.gate == gate && destination.pending_gate == pending_gate {
        return false;
    }
    destination.gate = gate;
    destination.pending_gate = pending_gate;
    true
}

fn benchmark_destinations(destination_count: usize) -> Vec<BenchmarkRouteDestination> {
    let mut destinations = Vec::with_capacity(destination_count);
    for index in 0..destination_count {
        destinations.push(benchmark_destination(index));
    }
    destinations
}

fn benchmark_destination(index: usize) -> BenchmarkRouteDestination {
    BenchmarkRouteDestination {
        session_key: benchmark_session_key(index),
        media_id: TransportMediaId::new(
            FIRST_CONSUMER_MEDIA_ID.saturating_add(u64::try_from(index).unwrap_or(0)),
        ),
        active: !index.is_multiple_of(7),
        gate: if index.is_multiple_of(11) {
            BenchmarkPacketGate::Block
        } else {
            BenchmarkPacketGate::Open
        },
        pending_gate: None,
    }
}

fn benchmark_updates(destinations: &[BenchmarkRouteDestination]) -> Vec<BenchmarkGateUpdate> {
    let mut updates = Vec::with_capacity(destinations.len());
    for destination in destinations.iter().rev() {
        let identity_key = IdentityLookupKey {
            session_key: destination.session_key.clone(),
            media_id: destination.media_id,
        };
        updates.push(BenchmarkGateUpdate {
            session_key: destination.session_key.clone(),
            media_id: destination.media_id,
            identity_key,
        });
    }
    updates
}

fn media_id_index(destinations: &[BenchmarkRouteDestination]) -> BTreeMap<TransportMediaId, usize> {
    destinations
        .iter()
        .enumerate()
        .map(|(position, destination)| (destination.media_id, position))
        .collect()
}

fn identity_index(
    destinations: &[BenchmarkRouteDestination],
) -> BTreeMap<IdentityLookupKey, usize> {
    destinations
        .iter()
        .enumerate()
        .map(|(position, destination)| {
            (
                IdentityLookupKey {
                    session_key: destination.session_key.clone(),
                    media_id: destination.media_id,
                },
                position,
            )
        })
        .collect()
}

fn benchmark_session_key(index: usize) -> TransportSessionKey {
    TransportSessionKey::new(
        RoomInstanceId::from_raw(CHANNEL_INSTANCE_ID),
        MEDIA_WORKER_ID,
        ConnectionId::from_raw(u64::try_from(index).unwrap_or(0).saturating_add(1)),
        UserId::Integer(i64::try_from(index).unwrap_or(0).saturating_add(1)),
    )
}

#[must_use]
pub const fn benchmark_source_media_id() -> u64 {
    SOURCE_MEDIA_ID
}
