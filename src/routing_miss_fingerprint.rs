use std::hint::black_box;

use o_sfu_core::server::transport::benchmark_support::routing_miss_packet_fingerprint;

pub const REALISTIC_PACKET_LENGTHS: [usize; 13] =
    [1, 8, 15, 16, 17, 32, 64, 96, 160, 256, 512, 900, 1200];

pub const MISS_GATE_PACKET_LENGTH_WEIGHTS: [(usize, usize); 13] = [
    (1, 1),
    (8, 1),
    (15, 1),
    (16, 1),
    (17, 1),
    (32, 12),
    (64, 14),
    (96, 12),
    (160, 18),
    (256, 10),
    (512, 8),
    (900, 10),
    (1200, 11),
];

#[derive(Debug)]
pub struct RoutingMissFingerprintBenchmarkFixture {
    packets: Vec<Vec<u8>>,
    byte_len: usize,
}

impl RoutingMissFingerprintBenchmarkFixture {
    #[must_use]
    pub fn new(packet_count: usize, packet_len: usize) -> Self {
        let mut packets = Vec::with_capacity(packet_count);
        for packet_index in 0..packet_count {
            let mut packet = Vec::with_capacity(packet_len);
            for byte_index in 0..packet_len {
                packet.push(deterministic_byte(packet_index, byte_index));
            }
            packets.push(packet);
        }
        let byte_len = packet_count.saturating_mul(packet_len);
        Self { packets, byte_len }
    }

    #[must_use]
    pub fn miss_gate_mix(cycle_count: usize) -> Self {
        let packet_count_per_cycle = MISS_GATE_PACKET_LENGTH_WEIGHTS
            .iter()
            .map(|(_, weight)| weight)
            .sum::<usize>();
        let mut packets = Vec::with_capacity(cycle_count.saturating_mul(packet_count_per_cycle));
        let mut byte_len = 0_usize;
        for cycle_index in 0..cycle_count {
            for (packet_len, weight) in MISS_GATE_PACKET_LENGTH_WEIGHTS {
                for weight_index in 0..weight {
                    let packet_index = packets
                        .len()
                        .wrapping_add(cycle_index)
                        .wrapping_add(weight_index);
                    packets.push(deterministic_packet(packet_index, packet_len));
                    byte_len = byte_len.saturating_add(packet_len);
                }
            }
        }
        Self { packets, byte_len }
    }

    #[must_use]
    pub const fn byte_len(&self) -> usize {
        self.byte_len
    }

    #[must_use]
    pub fn scalar_cycle(&self) -> u64 {
        let mut value = 0_u64;
        for packet in &self.packets {
            value ^= packet_fingerprint_scalar(black_box(packet.as_slice()));
        }
        black_box(value)
    }

    #[must_use]
    pub fn production_cycle(&self) -> u64 {
        let mut value = 0_u64;
        for packet in &self.packets {
            value ^= routing_miss_packet_fingerprint(black_box(packet.as_slice()));
        }
        black_box(value)
    }

    pub fn assert_production_matches_scalar(&self) {
        for packet in &self.packets {
            assert_eq!(
                routing_miss_packet_fingerprint(packet.as_slice()),
                packet_fingerprint_scalar(packet.as_slice())
            );
        }
    }
}

fn deterministic_packet(packet_index: usize, packet_len: usize) -> Vec<u8> {
    let mut packet = Vec::with_capacity(packet_len);
    for byte_index in 0..packet_len {
        packet.push(deterministic_byte(packet_index, byte_index));
    }
    packet
}

fn packet_fingerprint_scalar(packet: &[u8]) -> u64 {
    let len = u64::try_from(packet.len()).unwrap_or(u64::MAX);
    let prefix = load_u64_padded(packet);
    let suffix = load_u64_padded(
        packet
            .get(packet.len().saturating_sub(8)..)
            .unwrap_or(packet),
    );
    len.rotate_left(17) ^ prefix.rotate_left(29) ^ suffix.rotate_left(43)
}

fn load_u64_padded(bytes: &[u8]) -> u64 {
    let mut buffer = [0_u8; 8];
    for (slot, byte) in buffer.iter_mut().zip(bytes.iter().copied()) {
        *slot = byte;
    }
    u64::from_le_bytes(buffer)
}

fn deterministic_byte(packet_index: usize, byte_index: usize) -> u8 {
    let mixed = packet_index
        .wrapping_mul(37)
        .wrapping_add(byte_index.wrapping_mul(19))
        .wrapping_add(byte_index.rotate_left(3))
        .wrapping_add(11);
    u8::try_from(mixed & 0xff).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::RoutingMissFingerprintBenchmarkFixture;

    #[test]
    fn production_packet_fingerprint_matches_scalar_reference() {
        for len in [
            0_usize, 1, 7, 8, 15, 16, 17, 32, 64, 96, 160, 256, 512, 900, 1200,
        ] {
            RoutingMissFingerprintBenchmarkFixture::new(32, len).assert_production_matches_scalar();
        }
    }

    #[test]
    fn production_packet_fingerprint_matches_scalar_reference_for_miss_gate_mix() {
        RoutingMissFingerprintBenchmarkFixture::miss_gate_mix(4).assert_production_matches_scalar();
    }
}
