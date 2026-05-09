use std::{
    collections::{BTreeMap, BTreeSet},
    hint::black_box,
};

#[derive(Debug)]
pub struct SourcePolicyRefreshBenchmarkFixture {
    sources: Vec<BenchmarkSource>,
    routes: Vec<BenchmarkRoute>,
    receiver_bandwidth: Vec<(usize, u64)>,
    active_speaker_source_owners: BTreeSet<usize>,
    cached_ladders: Vec<Vec<u64>>,
}

#[derive(Debug, Clone)]
struct BenchmarkSource {
    owner: usize,
    encodings: Vec<BenchmarkEncoding>,
    active: bool,
}

#[derive(Debug, Clone)]
struct BenchmarkEncoding {
    max_bitrate_bps: u64,
}

#[derive(Debug, Clone)]
struct BenchmarkRoute {
    consumer: usize,
    source: usize,
    active: bool,
    current_selector: usize,
}

#[derive(Debug, Clone)]
struct CurrentRouteInput {
    consumer: usize,
    source: usize,
    featured: bool,
    visible_scalable_route_count: usize,
    receiver_bandwidth_bps: Option<u64>,
    current_selector: usize,
    encodings: Vec<u64>,
}

impl SourcePolicyRefreshBenchmarkFixture {
    #[must_use]
    pub fn new(user_count: usize, sources_per_user: usize) -> Self {
        let mut sources = Vec::with_capacity(user_count.saturating_mul(sources_per_user));
        for owner in 0..user_count {
            for source_offset in 0..sources_per_user {
                sources.push(BenchmarkSource {
                    owner,
                    encodings: benchmark_encodings(owner, source_offset),
                    active: true,
                });
            }
        }
        let mut routes = Vec::new();
        for consumer in 0..user_count {
            for (source_index, source) in sources.iter().enumerate() {
                if source.owner == consumer {
                    continue;
                }
                routes.push(BenchmarkRoute {
                    consumer,
                    source: source_index,
                    active: true,
                    current_selector: (consumer + source_index) % 3,
                });
            }
        }
        let receiver_bandwidth = (0..user_count)
            .map(|user| {
                (
                    user,
                    1_200_000_u64.saturating_add(u64::try_from(user % 8).unwrap_or(0) * 180_000),
                )
            })
            .collect();
        let active_speaker_source_owners = (0..user_count.min(4)).collect();
        let cached_ladders = sources.iter().map(selectable_encoding_ladder).collect();
        Self {
            sources,
            routes,
            receiver_bandwidth,
            active_speaker_source_owners,
            cached_ladders,
        }
    }

    #[must_use]
    pub fn route_count_u64(&self) -> u64 {
        u64::try_from(self.routes.len()).unwrap_or(u64::MAX)
    }

    #[must_use]
    pub fn current_rebuild_cycle(&self) -> usize {
        let receiver_bandwidth_by_user = self.receiver_bandwidth_by_user();
        let visible_scalable_route_counts = self.visible_scalable_route_counts();
        let selectable_ladders = self.selectable_ladders_by_source();
        let mut route_inputs = Vec::with_capacity(self.routes.len());
        for route in &self.routes {
            let source = &self.sources[route.source];
            if !route.active || !source.active {
                continue;
            }
            let Some(encodings) = selectable_ladders.get(&route.source) else {
                continue;
            };
            route_inputs.push(CurrentRouteInput {
                consumer: route.consumer,
                source: route.source,
                featured: self.active_speaker_source_owners.contains(&source.owner),
                visible_scalable_route_count: visible_scalable_route_counts
                    .get(&route.consumer)
                    .copied()
                    .unwrap_or(1),
                receiver_bandwidth_bps: receiver_bandwidth_by_user.get(&route.consumer).copied(),
                current_selector: route.current_selector,
                encodings: encodings.clone(),
            });
        }
        let mut routes_by_receiver = BTreeMap::<usize, Vec<&CurrentRouteInput>>::new();
        for route in &route_inputs {
            routes_by_receiver
                .entry(route.consumer)
                .or_default()
                .push(route);
        }
        black_box(plan_current_routes(routes_by_receiver))
    }

    #[must_use]
    pub fn cached_ladder_cycle(&self) -> usize {
        let receiver_bandwidth_by_user = self.receiver_bandwidth_by_user();
        let visible_scalable_route_counts = self.visible_scalable_route_counts();
        let mut routes_by_receiver = BTreeMap::<usize, Vec<usize>>::new();
        for (route_index, route) in self.routes.iter().enumerate() {
            let source = &self.sources[route.source];
            if !route.active || !source.active || self.cached_ladders[route.source].is_empty() {
                continue;
            }
            routes_by_receiver
                .entry(route.consumer)
                .or_default()
                .push(route_index);
        }
        let mut action_score = 0_usize;
        for (consumer, route_indexes) in routes_by_receiver {
            let receiver_bandwidth_bps = receiver_bandwidth_by_user.get(&consumer).copied();
            let mut selected_bitrate_bps = 0_u64;
            let mut planned_count = 0_usize;
            for route_index in route_indexes {
                let route = &self.routes[route_index];
                let source = &self.sources[route.source];
                let encodings = &self.cached_ladders[route.source];
                let visible_count = visible_scalable_route_counts
                    .get(&route.consumer)
                    .copied()
                    .unwrap_or(1);
                let selector = selected_encoding_index(
                    encodings,
                    route.current_selector,
                    self.active_speaker_source_owners.contains(&source.owner),
                    visible_count,
                );
                selected_bitrate_bps = selected_bitrate_bps
                    .saturating_add(encodings.get(selector).copied().unwrap_or(0));
                planned_count = planned_count.saturating_add(1);
            }
            action_score = action_score.saturating_add(planned_count);
            if receiver_bandwidth_bps.is_some_and(|budget| selected_bitrate_bps > budget) {
                action_score = action_score.saturating_add(1);
            }
        }
        black_box(action_score)
    }

    fn receiver_bandwidth_by_user(&self) -> BTreeMap<usize, u64> {
        self.receiver_bandwidth.iter().copied().collect()
    }

    fn visible_scalable_route_counts(&self) -> BTreeMap<usize, usize> {
        let mut counts = BTreeMap::new();
        for route in &self.routes {
            let source = &self.sources[route.source];
            if route.active && source.active {
                *counts.entry(route.consumer).or_default() += 1;
            }
        }
        counts
    }

    fn selectable_ladders_by_source(&self) -> BTreeMap<usize, Vec<u64>> {
        self.sources
            .iter()
            .enumerate()
            .map(|(source_index, source)| (source_index, selectable_encoding_ladder(source)))
            .filter(|(_source_index, encodings)| !encodings.is_empty())
            .collect()
    }
}

fn plan_current_routes(routes_by_receiver: BTreeMap<usize, Vec<&CurrentRouteInput>>) -> usize {
    let mut action_score = 0_usize;
    for routes in routes_by_receiver.into_values() {
        let mut selected_bitrate_bps = 0_u64;
        let mut receiver_bandwidth_bps = None;
        for route in &routes {
            receiver_bandwidth_bps = receiver_bandwidth_bps.or(route.receiver_bandwidth_bps);
            let selector = selected_encoding_index(
                &route.encodings,
                route.current_selector,
                route.featured,
                route.visible_scalable_route_count,
            );
            selected_bitrate_bps = selected_bitrate_bps
                .saturating_add(route.encodings.get(selector).copied().unwrap_or(0));
        }
        action_score = action_score.saturating_add(routes.len());
        if receiver_bandwidth_bps.is_some_and(|budget| selected_bitrate_bps > budget) {
            action_score = action_score.saturating_add(1);
        }
        action_score = action_score
            .saturating_add(routes.iter().filter(|route| route.source % 2 == 0).count());
    }
    action_score
}

fn selected_encoding_index(
    encodings: &[u64],
    current_selector: usize,
    featured: bool,
    visible_scalable_route_count: usize,
) -> usize {
    if encodings.is_empty() {
        return 0;
    }
    if featured {
        return encodings.len().saturating_sub(1);
    }
    if visible_scalable_route_count > 9 {
        return 0;
    }
    current_selector.min(encodings.len().saturating_sub(1))
}

fn selectable_encoding_ladder(source: &BenchmarkSource) -> Vec<u64> {
    let mut encodings = (0..source.encodings.len()).collect::<Vec<_>>();
    encodings.sort_by_key(|index| source.encodings[*index].max_bitrate_bps);
    encodings
        .into_iter()
        .map(|index| source.encodings[index].max_bitrate_bps)
        .collect()
}

fn benchmark_encodings(owner: usize, source_offset: usize) -> Vec<BenchmarkEncoding> {
    let seed = u64::try_from(owner.saturating_add(source_offset)).unwrap_or(0);
    vec![
        BenchmarkEncoding {
            max_bitrate_bps: 180_000_u64.saturating_add(seed % 32_000),
        },
        BenchmarkEncoding {
            max_bitrate_bps: 540_000_u64.saturating_add(seed % 64_000),
        },
        BenchmarkEncoding {
            max_bitrate_bps: 1_400_000_u64.saturating_add(seed % 128_000),
        },
    ]
}
