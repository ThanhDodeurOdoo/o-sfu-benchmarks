use std::{
    hint::black_box,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};

const BITRATE_WINDOW_NANOS: u64 = 1_000_000_000;
const SAME_WINDOW_NANOS: u64 = 123_456_789;

#[derive(Debug)]
pub struct BitrateCounterWriteBenchmarkFixture {
    payloads: Vec<u64>,
    bytes_in_window: AtomicU64,
    window_start_nanos: AtomicU64,
    observed: AtomicBool,
}

impl BitrateCounterWriteBenchmarkFixture {
    #[must_use]
    pub fn new(operation_count: usize) -> Self {
        let mut payloads = Vec::with_capacity(operation_count);
        for index in 0..operation_count {
            payloads.push(payload_bytes(index));
        }
        Self {
            payloads,
            bytes_in_window: AtomicU64::new(0),
            window_start_nanos: AtomicU64::new(0),
            observed: AtomicBool::new(false),
        }
    }

    #[must_use]
    pub fn operation_count_u64(&self) -> u64 {
        u64::try_from(self.payloads.len()).unwrap_or(u64::MAX)
    }

    #[must_use]
    pub fn current_fetch_update_write_cycle(&self) -> u64 {
        self.reset_bytes();
        for payload_bytes in &self.payloads {
            let _ =
                self.bytes_in_window
                    .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                        Some(current.saturating_add(*payload_bytes))
                    });
        }
        black_box(self.bytes_in_window.load(Ordering::Acquire))
    }

    #[must_use]
    pub fn fetch_add_write_cycle(&self) -> u64 {
        self.reset_bytes();
        for payload_bytes in &self.payloads {
            self.bytes_in_window
                .fetch_add(*payload_bytes, Ordering::Release);
        }
        black_box(self.bytes_in_window.load(Ordering::Acquire))
    }

    #[must_use]
    pub fn single_writer_load_store_write_cycle(&self) -> u64 {
        self.reset_bytes();
        for payload_bytes in &self.payloads {
            let current = self.bytes_in_window.load(Ordering::Relaxed);
            self.bytes_in_window
                .store(current.saturating_add(*payload_bytes), Ordering::Release);
        }
        black_box(self.bytes_in_window.load(Ordering::Acquire))
    }

    #[must_use]
    pub fn current_fetch_update_record_cycle(&self) -> u64 {
        self.reset_record();
        let mut first_observations = 0_u64;
        for payload_bytes in &self.payloads {
            self.record_with_fetch_update(*payload_bytes);
            first_observations = first_observations
                .saturating_add(u64::from(!self.observed.swap(true, Ordering::AcqRel)));
        }
        black_box(
            self.bytes_in_window
                .load(Ordering::Acquire)
                .saturating_add(first_observations),
        )
    }

    #[must_use]
    pub fn fetch_add_record_cycle(&self) -> u64 {
        self.reset_record();
        let mut first_observations = 0_u64;
        for payload_bytes in &self.payloads {
            self.record_with_fetch_add(*payload_bytes);
            first_observations = first_observations
                .saturating_add(u64::from(!self.observed.swap(true, Ordering::AcqRel)));
        }
        black_box(
            self.bytes_in_window
                .load(Ordering::Acquire)
                .saturating_add(first_observations),
        )
    }

    #[must_use]
    pub fn single_writer_load_store_record_cycle(&self) -> u64 {
        self.reset_record();
        let mut first_observations = 0_u64;
        for payload_bytes in &self.payloads {
            self.record_with_load_store(*payload_bytes);
            first_observations = first_observations
                .saturating_add(u64::from(!self.observed.swap(true, Ordering::AcqRel)));
        }
        black_box(
            self.bytes_in_window
                .load(Ordering::Acquire)
                .saturating_add(first_observations),
        )
    }

    fn record_with_fetch_update(&self, payload_bytes: u64) {
        self.reset_expired_window();
        let _ = self
            .bytes_in_window
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                Some(current.saturating_add(payload_bytes))
            });
    }

    fn record_with_fetch_add(&self, payload_bytes: u64) {
        self.reset_expired_window();
        self.bytes_in_window
            .fetch_add(payload_bytes, Ordering::Release);
    }

    fn record_with_load_store(&self, payload_bytes: u64) {
        self.reset_expired_window();
        let current = self.bytes_in_window.load(Ordering::Relaxed);
        self.bytes_in_window
            .store(current.saturating_add(payload_bytes), Ordering::Release);
    }

    fn reset_expired_window(&self) {
        let window_start = self.window_start_nanos.load(Ordering::Acquire);
        if SAME_WINDOW_NANOS.saturating_sub(window_start) >= BITRATE_WINDOW_NANOS {
            self.bytes_in_window.store(0, Ordering::Release);
            self.window_start_nanos
                .store(SAME_WINDOW_NANOS, Ordering::Release);
        }
    }

    fn reset_record(&self) {
        self.reset_bytes();
        self.window_start_nanos.store(0, Ordering::Release);
        self.observed.store(false, Ordering::Release);
    }

    fn reset_bytes(&self) {
        self.bytes_in_window.store(0, Ordering::Release);
    }
}

fn payload_bytes(index: usize) -> u64 {
    let packet_size = 172_usize.saturating_add(index % 1_229);
    u64::try_from(packet_size).unwrap_or(u64::MAX)
}
