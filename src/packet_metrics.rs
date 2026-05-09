use std::{
    hint::black_box,
    sync::{
        Arc, Barrier,
        atomic::{AtomicU64, Ordering},
    },
    thread,
};

const OPERATIONS_PER_WORKER: usize = 262_144;

#[derive(Debug)]
pub struct PacketMetricsContentionBenchmarkFixture {
    worker_count: usize,
    payloads: Vec<u64>,
}

#[derive(Debug, Default)]
struct GlobalCounters {
    ingress_packets: AtomicU64,
    ingress_payload_bytes: AtomicU64,
    egress_packets: AtomicU64,
    egress_payload_bytes: AtomicU64,
    forwarded_packets: AtomicU64,
    forwarded_payload_bytes: AtomicU64,
}

#[repr(align(64))]
#[derive(Debug, Default)]
struct PaddedCounter {
    value: AtomicU64,
}

#[derive(Debug, Default)]
struct WorkerCounters {
    ingress_packets: PaddedCounter,
    ingress_payload_bytes: PaddedCounter,
    egress_packets: PaddedCounter,
    egress_payload_bytes: PaddedCounter,
    forwarded_packets: PaddedCounter,
    forwarded_payload_bytes: PaddedCounter,
}

impl PacketMetricsContentionBenchmarkFixture {
    #[must_use]
    pub fn new(worker_count: usize) -> Self {
        let mut payloads = Vec::with_capacity(OPERATIONS_PER_WORKER);
        for index in 0..OPERATIONS_PER_WORKER {
            payloads.push(payload_bytes(index));
        }
        Self {
            worker_count,
            payloads,
        }
    }

    #[must_use]
    pub fn operation_count_u64(&self) -> u64 {
        u64::try_from(self.worker_count.saturating_mul(self.payloads.len())).unwrap_or(u64::MAX)
    }

    #[must_use]
    pub fn shared_global_atomic_cycle(&self) -> u64 {
        let counters = GlobalCounters::default();
        let barrier = Arc::new(Barrier::new(self.worker_count));
        thread::scope(|scope| {
            for worker in 0..self.worker_count {
                let barrier = Arc::clone(&barrier);
                let payloads = &self.payloads;
                let counters = &counters;
                scope.spawn(move || {
                    barrier.wait();
                    for payload_bytes in payloads {
                        record_global(counters, payload_bytes.saturating_add(worker as u64));
                    }
                });
            }
        });
        black_box(counters.total())
    }

    #[must_use]
    pub fn per_worker_atomic_cycle(&self) -> u64 {
        let counters = (0..self.worker_count)
            .map(|_| WorkerCounters::default())
            .collect::<Vec<_>>();
        let barrier = Arc::new(Barrier::new(self.worker_count));
        thread::scope(|scope| {
            for (worker, worker_counters) in counters.iter().enumerate() {
                let barrier = Arc::clone(&barrier);
                let payloads = &self.payloads;
                scope.spawn(move || {
                    barrier.wait();
                    for payload_bytes in payloads {
                        record_worker(worker_counters, payload_bytes.saturating_add(worker as u64));
                    }
                });
            }
        });
        black_box(counters.iter().map(WorkerCounters::total).sum())
    }

    #[must_use]
    pub fn thread_local_aggregate_cycle(&self) -> u64 {
        let barrier = Arc::new(Barrier::new(self.worker_count));
        let mut totals = Vec::with_capacity(self.worker_count);
        thread::scope(|scope| {
            let mut handles = Vec::with_capacity(self.worker_count);
            for worker in 0..self.worker_count {
                let barrier = Arc::clone(&barrier);
                let payloads = &self.payloads;
                handles.push(scope.spawn(move || {
                    barrier.wait();
                    let mut counters = LocalCounters::default();
                    for payload_bytes in payloads {
                        counters.record(payload_bytes.saturating_add(worker as u64));
                    }
                    counters.total()
                }));
            }
            for handle in handles {
                totals.push(handle.join().unwrap_or(0));
            }
        });
        black_box(totals.into_iter().sum())
    }
}

#[derive(Debug, Default)]
struct LocalCounters {
    ingress_packets: u64,
    ingress_payload_bytes: u64,
    egress_packets: u64,
    egress_payload_bytes: u64,
    forwarded_packets: u64,
    forwarded_payload_bytes: u64,
}

impl LocalCounters {
    fn record(&mut self, payload_bytes: u64) {
        self.ingress_packets = self.ingress_packets.saturating_add(1);
        self.ingress_payload_bytes = self.ingress_payload_bytes.saturating_add(payload_bytes);
        self.egress_packets = self.egress_packets.saturating_add(1);
        self.egress_payload_bytes = self.egress_payload_bytes.saturating_add(payload_bytes);
        self.forwarded_packets = self.forwarded_packets.saturating_add(1);
        self.forwarded_payload_bytes = self.forwarded_payload_bytes.saturating_add(payload_bytes);
    }

    fn total(&self) -> u64 {
        self.ingress_packets
            .saturating_add(self.ingress_payload_bytes)
            .saturating_add(self.egress_packets)
            .saturating_add(self.egress_payload_bytes)
            .saturating_add(self.forwarded_packets)
            .saturating_add(self.forwarded_payload_bytes)
    }
}

impl GlobalCounters {
    fn total(&self) -> u64 {
        self.ingress_packets
            .load(Ordering::Relaxed)
            .saturating_add(self.ingress_payload_bytes.load(Ordering::Relaxed))
            .saturating_add(self.egress_packets.load(Ordering::Relaxed))
            .saturating_add(self.egress_payload_bytes.load(Ordering::Relaxed))
            .saturating_add(self.forwarded_packets.load(Ordering::Relaxed))
            .saturating_add(self.forwarded_payload_bytes.load(Ordering::Relaxed))
    }
}

impl WorkerCounters {
    fn total(&self) -> u64 {
        self.ingress_packets
            .value
            .load(Ordering::Relaxed)
            .saturating_add(self.ingress_payload_bytes.value.load(Ordering::Relaxed))
            .saturating_add(self.egress_packets.value.load(Ordering::Relaxed))
            .saturating_add(self.egress_payload_bytes.value.load(Ordering::Relaxed))
            .saturating_add(self.forwarded_packets.value.load(Ordering::Relaxed))
            .saturating_add(self.forwarded_payload_bytes.value.load(Ordering::Relaxed))
    }
}

fn record_global(counters: &GlobalCounters, payload_bytes: u64) {
    counters.ingress_packets.fetch_add(1, Ordering::Relaxed);
    counters
        .ingress_payload_bytes
        .fetch_add(payload_bytes, Ordering::Relaxed);
    counters.egress_packets.fetch_add(1, Ordering::Relaxed);
    counters
        .egress_payload_bytes
        .fetch_add(payload_bytes, Ordering::Relaxed);
    counters.forwarded_packets.fetch_add(1, Ordering::Relaxed);
    counters
        .forwarded_payload_bytes
        .fetch_add(payload_bytes, Ordering::Relaxed);
}

fn record_worker(counters: &WorkerCounters, payload_bytes: u64) {
    counters
        .ingress_packets
        .value
        .fetch_add(1, Ordering::Relaxed);
    counters
        .ingress_payload_bytes
        .value
        .fetch_add(payload_bytes, Ordering::Relaxed);
    counters
        .egress_packets
        .value
        .fetch_add(1, Ordering::Relaxed);
    counters
        .egress_payload_bytes
        .value
        .fetch_add(payload_bytes, Ordering::Relaxed);
    counters
        .forwarded_packets
        .value
        .fetch_add(1, Ordering::Relaxed);
    counters
        .forwarded_payload_bytes
        .value
        .fetch_add(payload_bytes, Ordering::Relaxed);
}

fn payload_bytes(index: usize) -> u64 {
    u64::try_from(172_usize.saturating_add(index % 1_229)).unwrap_or(u64::MAX)
}
