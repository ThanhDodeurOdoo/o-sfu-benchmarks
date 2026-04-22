use std::{
    collections::BTreeSet,
    hint::black_box,
    net::{IpAddr, Ipv4Addr, SocketAddr},
};

use o_sfu::testing::transport::{
    RemoteAddrDemux, SessionId, TransportSessionKey, test_transport_session_key,
};

const BENCHMARK_CHANNEL_RUNTIME_ID: u64 = 1;
const BENCHMARK_MEDIA_WORKER_ID: usize = 0;
const BENCHMARK_FIRST_CONNECTION_ID: u64 = 1;
const BENCHMARK_FIRST_REMOTE_PORT: u16 = 10_000;

#[derive(Debug)]
pub struct RtcUdpDemuxBenchmarkFixture {
    demux: RemoteAddrDemux,
    live_sessions: BTreeSet<TransportSessionKey>,
    probe_addrs: Vec<SocketAddr>,
}

impl RtcUdpDemuxBenchmarkFixture {
    #[must_use]
    pub fn new(session_count: usize) -> Option<Self> {
        if session_count == 0 {
            return None;
        }
        let mut demux = RemoteAddrDemux::default();
        let mut live_sessions = BTreeSet::new();
        let mut probe_addrs = Vec::with_capacity(session_count);
        for idx in 0..session_count {
            let session_key = benchmark_session_key(idx)?;
            let remote_addr = benchmark_remote_addr(idx)?;
            let _ = demux.remember_remote_addr(remote_addr, &session_key);
            live_sessions.insert(session_key);
            probe_addrs.push(remote_addr);
        }
        Some(Self {
            demux,
            live_sessions,
            probe_addrs,
        })
    }

    #[must_use]
    pub fn lookup_count_u64(&self) -> u64 {
        u64::try_from(self.probe_addrs.len()).unwrap_or(u64::MAX)
    }

    #[must_use]
    pub fn cached_lookup_cycle(&self) -> usize {
        let mut hits = 0_usize;
        for remote_addr in &self.probe_addrs {
            if self
                .demux
                .session_key_for_remote_addr(*remote_addr)
                .is_some_and(|session_key| self.live_sessions.contains(session_key))
            {
                hits = hits.saturating_add(1);
            }
        }
        black_box(hits)
    }

    #[must_use]
    pub fn linear_scan_cycle(&self) -> usize {
        let mut hits = 0_usize;
        for remote_addr in &self.probe_addrs {
            if self
                .demux
                .session_entries()
                .any(|(session_key, session_addrs)| {
                    self.live_sessions.contains(session_key) && session_addrs.contains(remote_addr)
                })
            {
                hits = hits.saturating_add(1);
            }
        }
        black_box(hits)
    }
}

#[derive(Debug)]
pub struct RtcUnknownSourceRecoveryBenchmarkFixture {
    candidate_index: RemoteAddrDemux,
    probe_addrs: Vec<SocketAddr>,
    session_candidate_addrs: Vec<(TransportSessionKey, Vec<SocketAddr>)>,
}

impl RtcUnknownSourceRecoveryBenchmarkFixture {
    #[must_use]
    pub fn new(session_count: usize) -> Option<Self> {
        if session_count == 0 {
            return None;
        }
        let mut candidate_index = RemoteAddrDemux::default();
        let mut probe_addrs = Vec::with_capacity(session_count);
        let mut session_candidate_addrs = Vec::with_capacity(session_count);
        for idx in 0..session_count {
            let session_key = benchmark_session_key(idx)?;
            let remote_addr = benchmark_remote_addr(idx)?;
            candidate_index.replace_session_remote_candidate_addrs(&session_key, [remote_addr]);
            probe_addrs.push(remote_addr);
            session_candidate_addrs.push((session_key, vec![remote_addr]));
        }
        Some(Self {
            candidate_index,
            probe_addrs,
            session_candidate_addrs,
        })
    }

    #[must_use]
    pub fn lookup_count_u64(&self) -> u64 {
        u64::try_from(self.probe_addrs.len()).unwrap_or(u64::MAX)
    }

    #[must_use]
    pub fn indexed_lookup_cycle(&self) -> usize {
        let mut hits = 0_usize;
        for remote_addr in &self.probe_addrs {
            if self
                .candidate_index
                .candidate_sessions_for_source_addr(*remote_addr)
                .is_some_and(|session_keys| !session_keys.is_empty())
            {
                hits = hits.saturating_add(1);
            }
        }
        black_box(hits)
    }

    #[must_use]
    pub fn linear_scan_cycle(&self) -> usize {
        let mut hits = 0_usize;
        for remote_addr in &self.probe_addrs {
            if self
                .session_candidate_addrs
                .iter()
                .any(|(_session_key, candidate_addrs)| candidate_addrs.contains(remote_addr))
            {
                hits = hits.saturating_add(1);
            }
        }
        black_box(hits)
    }
}

fn benchmark_session_key(idx: usize) -> Option<TransportSessionKey> {
    let session_id = SessionId::Integer(i64::try_from(idx).ok()?);
    let connection_id = BENCHMARK_FIRST_CONNECTION_ID.saturating_add(u64::try_from(idx).ok()?);
    Some(test_transport_session_key(
        BENCHMARK_CHANNEL_RUNTIME_ID,
        BENCHMARK_MEDIA_WORKER_ID,
        connection_id,
        session_id,
    ))
}

fn benchmark_remote_addr(idx: usize) -> Option<SocketAddr> {
    let offset = u16::try_from(idx).ok()?;
    let port = BENCHMARK_FIRST_REMOTE_PORT.checked_add(offset)?;
    Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port))
}
