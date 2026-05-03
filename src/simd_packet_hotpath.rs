use std::hint::black_box;

use o_sfu_rfc::rtp::{h264, vp8};

const H264_NAL_UNIT_TYPE_MASK: u8 = 0x1f;
const H264_NAL_UNIT_TYPE_IDR: u8 = 5;
const H264_NAL_UNIT_TYPE_STAP_A: u8 = 24;
const H264_NAL_UNIT_TYPE_FU_A: u8 = 28;
const H264_FU_START_BIT: u8 = 0x80;

const VP8_X_BIT: u8 = 0x80;
const VP8_S_BIT: u8 = 0x10;
const VP8_PARTITION_ID_MASK: u8 = 0x0f;
const VP8_INTERFRAME_BIT: u8 = 0x01;

#[derive(Debug)]
pub struct PacketFingerprintBenchmarkFixture {
    packets: Vec<Vec<u8>>,
}

impl PacketFingerprintBenchmarkFixture {
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
        Self { packets }
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
    pub fn simd_cycle(&self) -> u64 {
        let mut value = 0_u64;
        for packet in &self.packets {
            value ^= packet_fingerprint_simd(black_box(packet.as_slice()));
        }
        black_box(value)
    }
}

#[derive(Debug)]
pub struct H264PayloadScannerBenchmarkFixture {
    payloads: Vec<Vec<u8>>,
}

impl H264PayloadScannerBenchmarkFixture {
    #[must_use]
    pub fn new(payload_count: usize, stap_a_nal_count: usize) -> Self {
        let mut payloads = Vec::with_capacity(payload_count);
        for index in 0..payload_count {
            payloads.push(match index % 4 {
                0 => vec![0x65, 0x88, deterministic_byte(index, 1)],
                1 => vec![0x41, 0x9a, deterministic_byte(index, 2)],
                2 => stap_a_payload(stap_a_nal_count, index, true),
                _ => stap_a_payload(stap_a_nal_count, index, false),
            });
        }
        Self { payloads }
    }

    #[must_use]
    pub fn scalar_cycle(&self) -> usize {
        let mut detected = 0_usize;
        for payload in &self.payloads {
            detected = detected.saturating_add(usize::from(h264::payload_starts_idr(black_box(
                payload.as_slice(),
            ))));
        }
        black_box(detected)
    }

    #[must_use]
    pub fn simd_cycle(&self) -> usize {
        let mut detected = 0_usize;
        for payload in &self.payloads {
            detected = detected.saturating_add(usize::from(h264_payload_starts_idr_simd(
                black_box(payload.as_slice()),
            )));
        }
        black_box(detected)
    }
}

#[derive(Debug)]
pub struct Vp8PayloadScannerBenchmarkFixture {
    payloads: Vec<[u8; 8]>,
}

impl Vp8PayloadScannerBenchmarkFixture {
    #[must_use]
    pub fn new(payload_count: usize) -> Self {
        let mut payloads = Vec::with_capacity(payload_count);
        for index in 0..payload_count {
            payloads.push(match index % 8 {
                0 | 1 => [0x10, 0x00, deterministic_byte(index, 2), 0, 0, 0, 0, 0],
                2 | 3 => [0x10, 0x01, deterministic_byte(index, 2), 0, 0, 0, 0, 0],
                4 => [0x00, 0x00, deterministic_byte(index, 2), 0, 0, 0, 0, 0],
                5 => [0x11, 0x00, deterministic_byte(index, 2), 0, 0, 0, 0, 0],
                6 => [0x90, 0x80, 0x80, 0x80, 0x00, 0x00, 0, 0],
                _ => [0x90, 0x80, 0x80, 0x80, 0x01, 0x00, 0, 0],
            });
        }
        Self { payloads }
    }

    #[must_use]
    pub fn scalar_cycle(&self) -> usize {
        let mut detected = 0_usize;
        for payload in &self.payloads {
            detected = detected.saturating_add(usize::from(vp8::payload_starts_keyframe(
                black_box(payload.as_slice()),
            )));
        }
        black_box(detected)
    }

    #[must_use]
    pub fn simd_batch_cycle(&self) -> usize {
        black_box(vp8_payload_starts_keyframe_simd_batch(&self.payloads))
    }
}

fn deterministic_byte(packet_index: usize, byte_index: usize) -> u8 {
    let mixed = packet_index
        .wrapping_mul(31)
        .wrapping_add(byte_index.wrapping_mul(17))
        .wrapping_add(13);
    u8::try_from(mixed & 0xff).unwrap_or(0)
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

fn packet_fingerprint_simd(packet: &[u8]) -> u64 {
    if packet.len() < 16 {
        return packet_fingerprint_scalar(packet);
    }
    let len = u64::try_from(packet.len()).unwrap_or(u64::MAX);
    let prefix = simd_first_u64(packet);
    let suffix = simd_last_u64(packet);
    len.rotate_left(17) ^ prefix.rotate_left(29) ^ suffix.rotate_left(43)
}

fn h264_payload_starts_idr_simd(payload: &[u8]) -> bool {
    let Some((&nal_header, rest)) = payload.split_first() else {
        return false;
    };
    match nal_header & H264_NAL_UNIT_TYPE_MASK {
        H264_NAL_UNIT_TYPE_IDR => true,
        H264_NAL_UNIT_TYPE_STAP_A => stap_a_contains_idr_simd(rest),
        H264_NAL_UNIT_TYPE_FU_A => fu_a_starts_idr(rest),
        _ => false,
    }
}

fn stap_a_contains_idr_simd(mut payload: &[u8]) -> bool {
    let mut nal_types = [0_u8; 16];
    let mut nal_type_count = 0_usize;
    while payload.len() >= 2 {
        let nal_len = usize::from(u16::from_be_bytes([payload[0], payload[1]]));
        let rest = &payload[2..];
        if nal_len == 0 || rest.len() < nal_len {
            return false;
        }
        nal_types[nal_type_count] = rest[0] & H264_NAL_UNIT_TYPE_MASK;
        nal_type_count = nal_type_count.saturating_add(1);
        if nal_type_count == nal_types.len() {
            if simd_any_eq_u8x16(&nal_types, H264_NAL_UNIT_TYPE_IDR) {
                return true;
            }
            nal_type_count = 0;
        }
        payload = &rest[nal_len..];
    }
    nal_types[..nal_type_count].contains(&H264_NAL_UNIT_TYPE_IDR)
}

fn fu_a_starts_idr(payload: &[u8]) -> bool {
    payload.first().is_some_and(|fu_header| {
        fu_header & H264_FU_START_BIT != 0
            && fu_header & H264_NAL_UNIT_TYPE_MASK == H264_NAL_UNIT_TYPE_IDR
    })
}

fn vp8_payload_starts_keyframe_simd_batch(payloads: &[[u8; 8]]) -> usize {
    let mut detected = 0_usize;
    let mut descriptors = [0_u8; 16];
    let mut payload_headers = [0_u8; 16];
    let mut batch_len = 0_usize;

    for payload in payloads {
        let descriptor = payload[0];
        if descriptor & VP8_X_BIT != 0 {
            detected = detected.saturating_add(usize::from(vp8::payload_starts_keyframe(
                payload.as_slice(),
            )));
            continue;
        }
        descriptors[batch_len] = descriptor;
        payload_headers[batch_len] = payload[1];
        batch_len = batch_len.saturating_add(1);
        if batch_len == descriptors.len() {
            detected = detected.saturating_add(vp8_simple_keyframe_count_simd(
                &descriptors,
                &payload_headers,
            ));
            batch_len = 0;
        }
    }

    for index in 0..batch_len {
        detected = detected.saturating_add(usize::from(vp8_simple_payload_starts_keyframe(
            descriptors[index],
            payload_headers[index],
        )));
    }
    detected
}

fn vp8_simple_payload_starts_keyframe(descriptor: u8, payload_header: u8) -> bool {
    descriptor & VP8_S_BIT != 0
        && descriptor & VP8_PARTITION_ID_MASK == 0
        && payload_header & VP8_INTERFRAME_BIT == 0
}

fn stap_a_payload(nal_count: usize, seed: usize, include_idr: bool) -> Vec<u8> {
    let mut payload = Vec::with_capacity(nal_count.saturating_mul(8).saturating_add(1));
    payload.push(H264_NAL_UNIT_TYPE_STAP_A);
    let idr_index = nal_count / 2;
    for nal_index in 0..nal_count {
        let nal_type = if include_idr && nal_index == idr_index {
            H264_NAL_UNIT_TYPE_IDR
        } else {
            1
        };
        let nal = [
            nal_type,
            deterministic_byte(seed, nal_index),
            deterministic_byte(seed, nal_index.saturating_add(1)),
            deterministic_byte(seed, nal_index.saturating_add(2)),
        ];
        let nal_len = u16::try_from(nal.len()).unwrap_or(u16::MAX);
        payload.extend_from_slice(&nal_len.to_be_bytes());
        payload.extend_from_slice(&nal);
    }
    payload
}

#[cfg(target_arch = "aarch64")]
fn simd_first_u64(packet: &[u8]) -> u64 {
    use std::arch::aarch64::{
        uint8x16_t, uint64x2_t, vgetq_lane_u64, vld1q_u8, vreinterpretq_u64_u8,
    };

    // SAFETY: `packet_fingerprint_simd` calls this only for slices with at least
    // 16 bytes, so the unaligned 16-byte vector load stays within the slice.
    unsafe {
        let vector: uint8x16_t = vld1q_u8(packet.as_ptr());
        let lanes: uint64x2_t = vreinterpretq_u64_u8(vector);
        vgetq_lane_u64::<0>(lanes)
    }
}

#[cfg(target_arch = "aarch64")]
fn simd_last_u64(packet: &[u8]) -> u64 {
    use std::arch::aarch64::{
        uint8x16_t, uint64x2_t, vgetq_lane_u64, vld1q_u8, vreinterpretq_u64_u8,
    };

    // SAFETY: `packet_fingerprint_simd` calls this only for slices with at least
    // 16 bytes, and `len - 16` starts a full in-bounds vector load.
    unsafe {
        let vector: uint8x16_t = vld1q_u8(packet.as_ptr().add(packet.len() - 16));
        let lanes: uint64x2_t = vreinterpretq_u64_u8(vector);
        vgetq_lane_u64::<1>(lanes)
    }
}

#[cfg(target_arch = "x86_64")]
fn simd_first_u64(packet: &[u8]) -> u64 {
    use std::arch::x86_64::{__m128i, _mm_cvtsi128_si64, _mm_loadu_si128};

    // SAFETY: `packet_fingerprint_simd` calls this only for slices with at least
    // 16 bytes, so the unaligned 16-byte vector load stays within the slice.
    let vector = unsafe { _mm_loadu_si128(packet.as_ptr().cast::<__m128i>()) };
    u64::from_le_bytes(_mm_cvtsi128_si64(vector).to_le_bytes())
}

#[cfg(target_arch = "x86_64")]
fn simd_last_u64(packet: &[u8]) -> u64 {
    use std::arch::x86_64::{__m128i, _mm_cvtsi128_si64, _mm_loadu_si128, _mm_srli_si128};

    // SAFETY: `packet_fingerprint_simd` calls this only for slices with at least
    // 16 bytes, and `len - 16` starts a full in-bounds vector load.
    let vector =
        unsafe { _mm_loadu_si128(packet.as_ptr().add(packet.len() - 16).cast::<__m128i>()) };
    let high_lane = _mm_srli_si128::<8>(vector);
    u64::from_le_bytes(_mm_cvtsi128_si64(high_lane).to_le_bytes())
}

#[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
fn simd_first_u64(packet: &[u8]) -> u64 {
    load_u64_padded(packet)
}

#[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
fn simd_last_u64(packet: &[u8]) -> u64 {
    load_u64_padded(
        packet
            .get(packet.len().saturating_sub(8)..)
            .unwrap_or(packet),
    )
}

#[cfg(target_arch = "aarch64")]
fn simd_any_eq_u8x16(values: &[u8; 16], needle: u8) -> bool {
    use std::arch::aarch64::{vceqq_u8, vdupq_n_u8, vld1q_u8, vmaxvq_u8};

    // SAFETY: `values` is exactly 16 contiguous bytes, so the vector load is
    // fully in-bounds and does not require alignment.
    unsafe {
        let vector = vld1q_u8(values.as_ptr());
        let matches = vceqq_u8(vector, vdupq_n_u8(needle));
        vmaxvq_u8(matches) != 0
    }
}

#[cfg(target_arch = "x86_64")]
fn simd_any_eq_u8x16(values: &[u8; 16], needle: u8) -> bool {
    use std::arch::x86_64::{
        __m128i, _mm_cmpeq_epi8, _mm_loadu_si128, _mm_movemask_epi8, _mm_set1_epi8,
    };

    // SAFETY: `values` is exactly 16 contiguous bytes, so the vector load is
    // fully in-bounds and does not require alignment. SSE2 is baseline on x86_64.
    let vector = unsafe { _mm_loadu_si128(values.as_ptr().cast::<__m128i>()) };
    let matches = unsafe { _mm_cmpeq_epi8(vector, _mm_set1_epi8(i8::from_ne_bytes([needle]))) };
    _mm_movemask_epi8(matches) != 0
}

#[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
fn simd_any_eq_u8x16(values: &[u8; 16], needle: u8) -> bool {
    values.iter().any(|value| *value == needle)
}

fn vp8_simple_keyframe_count_simd(descriptors: &[u8; 16], payload_headers: &[u8; 16]) -> usize {
    vp8_simple_keyframe_mask_simd(descriptors, payload_headers).count_ones() as usize
}

#[cfg(target_arch = "aarch64")]
fn vp8_simple_keyframe_mask_simd(descriptors: &[u8; 16], payload_headers: &[u8; 16]) -> u32 {
    use std::arch::aarch64::{vandq_u8, vceqq_u8, vdupq_n_u8, vld1q_u8, vst1q_u8};

    // SAFETY: both arrays are exactly 16 bytes, so all unaligned vector loads
    // and the final store into the local mask buffer are fully in-bounds.
    let mut lanes = [0_u8; 16];
    unsafe {
        let descriptor_vector = vld1q_u8(descriptors.as_ptr());
        let header_vector = vld1q_u8(payload_headers.as_ptr());
        let descriptor_mask = vceqq_u8(
            vandq_u8(
                descriptor_vector,
                vdupq_n_u8(VP8_S_BIT | VP8_PARTITION_ID_MASK),
            ),
            vdupq_n_u8(VP8_S_BIT),
        );
        let header_mask = vceqq_u8(
            vandq_u8(header_vector, vdupq_n_u8(VP8_INTERFRAME_BIT)),
            vdupq_n_u8(0),
        );
        let combined = vandq_u8(descriptor_mask, header_mask);
        vst1q_u8(lanes.as_mut_ptr(), combined);
    }
    mask_from_full_byte_lanes(&lanes)
}

#[cfg(target_arch = "x86_64")]
fn vp8_simple_keyframe_mask_simd(descriptors: &[u8; 16], payload_headers: &[u8; 16]) -> u32 {
    use std::arch::x86_64::{
        __m128i, _mm_and_si128, _mm_cmpeq_epi8, _mm_loadu_si128, _mm_movemask_epi8, _mm_set1_epi8,
    };

    // SAFETY: both arrays are exactly 16 bytes, so the unaligned vector loads
    // are fully in-bounds. SSE2 is baseline on x86_64.
    let descriptor_vector = unsafe { _mm_loadu_si128(descriptors.as_ptr().cast::<__m128i>()) };
    let header_vector = unsafe { _mm_loadu_si128(payload_headers.as_ptr().cast::<__m128i>()) };
    let descriptor_mask = unsafe {
        _mm_cmpeq_epi8(
            _mm_and_si128(
                descriptor_vector,
                _mm_set1_epi8(i8::from_ne_bytes([VP8_S_BIT | VP8_PARTITION_ID_MASK])),
            ),
            _mm_set1_epi8(i8::from_ne_bytes([VP8_S_BIT])),
        )
    };
    let header_mask = unsafe {
        _mm_cmpeq_epi8(
            _mm_and_si128(
                header_vector,
                _mm_set1_epi8(i8::from_ne_bytes([VP8_INTERFRAME_BIT])),
            ),
            _mm_set1_epi8(0),
        )
    };
    let combined = unsafe { _mm_and_si128(descriptor_mask, header_mask) };
    u32::try_from(_mm_movemask_epi8(combined)).unwrap_or(0)
}

#[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
fn vp8_simple_keyframe_mask_simd(descriptors: &[u8; 16], payload_headers: &[u8; 16]) -> u32 {
    let mut mask = 0_u32;
    for index in 0..16 {
        if vp8_simple_payload_starts_keyframe(descriptors[index], payload_headers[index]) {
            mask |= 1 << index;
        }
    }
    mask
}

#[cfg(target_arch = "aarch64")]
fn mask_from_full_byte_lanes(lanes: &[u8; 16]) -> u32 {
    let mut mask = 0_u32;
    for (index, lane) in lanes.iter().enumerate() {
        if *lane != 0 {
            mask |= 1 << index;
        }
    }
    mask
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simd_packet_fingerprint_matches_scalar() {
        for len in [0_usize, 1, 7, 8, 15, 16, 17, 64, 1200] {
            let fixture = PacketFingerprintBenchmarkFixture::new(8, len);
            for packet in fixture.packets {
                assert_eq!(
                    packet_fingerprint_simd(packet.as_slice()),
                    packet_fingerprint_scalar(packet.as_slice())
                );
            }
        }
    }

    #[test]
    fn simd_h264_payload_scanner_matches_current_scanner() {
        for nal_count in [1_usize, 4, 16, 64] {
            let fixture = H264PayloadScannerBenchmarkFixture::new(64, nal_count);
            for payload in fixture.payloads {
                assert_eq!(
                    h264_payload_starts_idr_simd(payload.as_slice()),
                    h264::payload_starts_idr(payload.as_slice())
                );
            }
        }
    }

    #[test]
    fn simd_vp8_batch_scanner_matches_current_scanner() {
        let fixture = Vp8PayloadScannerBenchmarkFixture::new(129);
        let expected = fixture
            .payloads
            .iter()
            .filter(|payload| vp8::payload_starts_keyframe(payload.as_slice()))
            .count();

        assert_eq!(
            vp8_payload_starts_keyframe_simd_batch(&fixture.payloads),
            expected
        );
    }
}
