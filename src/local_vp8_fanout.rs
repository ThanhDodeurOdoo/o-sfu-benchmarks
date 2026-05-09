use std::hint::black_box;

use o_sfu_rfc::rtp::vp8;

#[derive(Debug)]
pub struct LocalVp8FanoutBenchmarkFixture {
    base_payload: Vec<u8>,
    shared_payload: Vec<u8>,
    rewrite_values: Vec<Vp8RewriteValue>,
}

#[derive(Debug, Clone, Copy)]
struct Vp8RewriteValue {
    picture_id: u16,
    tl0_pic_idx: u8,
}

impl LocalVp8FanoutBenchmarkFixture {
    #[must_use]
    pub fn new(destination_count: usize) -> Self {
        let mut rewrite_values = Vec::with_capacity(destination_count);
        for index in 0..destination_count {
            rewrite_values.push(Vp8RewriteValue {
                picture_id: u16::try_from(index % 32_768).unwrap_or(0),
                tl0_pic_idx: u8::try_from(index % 256).unwrap_or(0),
            });
        }
        Self {
            base_payload: vp8_payload_with_rid_metadata(1_200),
            shared_payload: Vec::with_capacity(1_200),
            rewrite_values,
        }
    }

    #[must_use]
    pub fn destination_count_u64(&self) -> u64 {
        u64::try_from(self.rewrite_values.len()).unwrap_or(u64::MAX)
    }

    #[must_use]
    pub fn current_parse_per_destination_cycle(&mut self) -> usize {
        self.reset_shared_payload();
        let destination_count = self.rewrite_values.len();
        let mut written_bytes = 0_usize;
        let mut descriptor_count = 0_usize;
        for index in 0..destination_count {
            let rewrite_value = self.rewrite_values[index];
            let descriptor = vp8::payload_descriptor(&self.shared_payload);
            if let Some(descriptor) = descriptor {
                descriptor_count = descriptor_count.saturating_add(1);
                let mut payload = self.write_payload_for_destination(index, destination_count);
                rewrite_vp8_payload(&mut payload, descriptor, rewrite_value);
                written_bytes = written_bytes.saturating_add(payload.len());
                black_box(payload);
            }
        }
        black_box(written_bytes.saturating_add(descriptor_count))
    }

    #[must_use]
    pub fn cached_descriptor_cycle(&mut self) -> usize {
        self.reset_shared_payload();
        let descriptor = vp8::payload_descriptor(&self.shared_payload);
        let destination_count = self.rewrite_values.len();
        let mut written_bytes = 0_usize;
        let mut descriptor_count = 0_usize;
        for index in 0..destination_count {
            let rewrite_value = self.rewrite_values[index];
            if let Some(descriptor) = descriptor {
                descriptor_count = 1;
                let mut payload = self.write_payload_for_destination(index, destination_count);
                rewrite_vp8_payload(&mut payload, descriptor, rewrite_value);
                written_bytes = written_bytes.saturating_add(payload.len());
                black_box(payload);
            }
        }
        black_box(written_bytes.saturating_add(descriptor_count))
    }

    fn reset_shared_payload(&mut self) {
        self.shared_payload.clear();
        self.shared_payload.extend_from_slice(&self.base_payload);
    }

    fn write_payload_for_destination(
        &mut self,
        destination_index: usize,
        destination_count: usize,
    ) -> Vec<u8> {
        if destination_index + 1 == destination_count {
            return std::mem::take(&mut self.shared_payload);
        }
        self.shared_payload.clone()
    }
}

fn rewrite_vp8_payload(
    payload: &mut [u8],
    descriptor: vp8::PayloadDescriptor,
    rewrite_value: Vp8RewriteValue,
) {
    vp8::rewrite_payload_descriptor(
        payload,
        descriptor,
        vp8::PayloadDescriptorRewrite {
            picture_id: Some(rewrite_value.picture_id),
            tl0_pic_idx: Some(rewrite_value.tl0_pic_idx),
        },
    );
}

fn vp8_payload_with_rid_metadata(payload_len: usize) -> Vec<u8> {
    let mut payload = Vec::with_capacity(payload_len);
    payload.extend_from_slice(&[0x90, 0xc0, 0x80, 0x31, 0x05, 0x00]);
    for index in payload.len()..payload_len {
        payload.push(u8::try_from(index % 251).unwrap_or(0));
    }
    payload
}
