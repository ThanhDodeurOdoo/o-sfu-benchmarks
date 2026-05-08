use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    hint::black_box,
    net::{IpAddr, Ipv4Addr, SocketAddr},
};

use o_sfu_core::{
    ConnectionId, RoomInstanceId,
    server::{session::UserId, transport::TransportSessionKey},
};

const BENCHMARK_CHANNEL_RUNTIME_ID: u64 = 1;
const BENCHMARK_MEDIA_WORKER_ID: usize = 0;
const BENCHMARK_FIRST_CONNECTION_ID: u64 = 1;
const BENCHMARK_FIRST_REMOTE_PORT: u16 = 10_000;

#[derive(Debug, Default)]
struct BenchmarkRemoteAddrDemux {
    remote_addr_index: HashMap<SocketAddr, TransportSessionKey>,
    remote_addrs_by_session: BTreeMap<TransportSessionKey, Vec<SocketAddr>>,
    remote_candidate_addr_index: HashMap<SocketAddr, Vec<TransportSessionKey>>,
    remote_candidate_addrs_by_session: BTreeMap<TransportSessionKey, Vec<SocketAddr>>,
}

impl BenchmarkRemoteAddrDemux {
    fn session_key_for_remote_addr(&self, source_addr: SocketAddr) -> Option<&TransportSessionKey> {
        self.remote_addr_index.get(&source_addr)
    }

    fn remember_remote_addr(
        &mut self,
        source_addr: SocketAddr,
        session_key: &TransportSessionKey,
    ) -> bool {
        if self
            .remote_addr_index
            .get(&source_addr)
            .is_some_and(|current_session| current_session == session_key)
        {
            return false;
        }
        let previous_session = self
            .remote_addr_index
            .insert(source_addr, session_key.clone());
        if let Some(previous_session) = previous_session {
            self.remove_remote_addr_from_session(&previous_session, source_addr);
        }
        let session_addrs = self
            .remote_addrs_by_session
            .entry(session_key.clone())
            .or_default();
        if !session_addrs.contains(&source_addr) {
            session_addrs.push(source_addr);
        }
        true
    }

    fn session_entries(&self) -> impl Iterator<Item = (&TransportSessionKey, &[SocketAddr])> {
        self.remote_addrs_by_session
            .iter()
            .map(|(session_key, addrs)| (session_key, addrs.as_slice()))
    }

    fn replace_session_remote_candidate_addrs(
        &mut self,
        session_key: &TransportSessionKey,
        addrs: impl IntoIterator<Item = SocketAddr>,
    ) {
        if let Some(previous_addrs) = self.remote_candidate_addrs_by_session.remove(session_key) {
            for addr in previous_addrs {
                self.remove_candidate_session(addr, session_key);
            }
        }
        let mut stored_addrs = Vec::new();
        for addr in addrs {
            let sessions = self.remote_candidate_addr_index.entry(addr).or_default();
            if !sessions.contains(session_key) {
                sessions.push(session_key.clone());
            }
            stored_addrs.push(addr);
        }
        self.remote_candidate_addrs_by_session
            .insert(session_key.clone(), stored_addrs);
    }

    fn candidate_sessions_for_source_addr(
        &self,
        source_addr: SocketAddr,
    ) -> Option<&[TransportSessionKey]> {
        self.remote_candidate_addr_index
            .get(&source_addr)
            .map(Vec::as_slice)
    }

    fn remove_remote_addr_from_session(
        &mut self,
        session_key: &TransportSessionKey,
        source_addr: SocketAddr,
    ) {
        let Some(session_addrs) = self.remote_addrs_by_session.get_mut(session_key) else {
            return;
        };
        session_addrs.retain(|addr| *addr != source_addr);
        if session_addrs.is_empty() {
            self.remote_addrs_by_session.remove(session_key);
        }
    }

    fn remove_candidate_session(
        &mut self,
        source_addr: SocketAddr,
        session_key: &TransportSessionKey,
    ) {
        let Some(sessions) = self.remote_candidate_addr_index.get_mut(&source_addr) else {
            return;
        };
        sessions.retain(|candidate_session| candidate_session != session_key);
        if sessions.is_empty() {
            self.remote_candidate_addr_index.remove(&source_addr);
        }
    }
}

#[derive(Debug)]
pub struct RtcUdpDemuxBenchmarkFixture {
    demux: BenchmarkRemoteAddrDemux,
    live_sessions: BTreeSet<TransportSessionKey>,
    probe_addrs: Vec<SocketAddr>,
}

impl RtcUdpDemuxBenchmarkFixture {
    #[must_use]
    pub fn new(session_count: usize) -> Option<Self> {
        if session_count == 0 {
            return None;
        }
        let mut demux = BenchmarkRemoteAddrDemux::default();
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
    candidate_index: BenchmarkRemoteAddrDemux,
    probe_addrs: Vec<SocketAddr>,
    session_candidate_addrs: Vec<(TransportSessionKey, Vec<SocketAddr>)>,
}

impl RtcUnknownSourceRecoveryBenchmarkFixture {
    #[must_use]
    pub fn new(session_count: usize) -> Option<Self> {
        if session_count == 0 {
            return None;
        }
        let mut candidate_index = BenchmarkRemoteAddrDemux::default();
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
    let session_id = UserId::Integer(i64::try_from(idx).ok()?);
    let connection_id = BENCHMARK_FIRST_CONNECTION_ID.saturating_add(u64::try_from(idx).ok()?);
    Some(TransportSessionKey::new(
        RoomInstanceId::from_raw(BENCHMARK_CHANNEL_RUNTIME_ID),
        BENCHMARK_MEDIA_WORKER_ID,
        ConnectionId::from_raw(connection_id),
        session_id,
    ))
}

fn benchmark_remote_addr(idx: usize) -> Option<SocketAddr> {
    let offset = u16::try_from(idx).ok()?;
    let port = BENCHMARK_FIRST_REMOTE_PORT.checked_add(offset)?;
    Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port))
}
