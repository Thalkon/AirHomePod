use crate::error::Result;

pub struct BitWriter {
    buffer: Vec<u8>,
    accumulator: u64,
    bit_count: u8,
}

impl BitWriter {
    pub fn with_capacity(capacity_bytes: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(capacity_bytes),
            accumulator: 0,
            bit_count: 0,
        }
    }

    pub fn write_bits(&mut self, value: u64, bits: u8) {
        if bits == 0 {
            return;
        }
        let mask = if bits == 64 { !0 } else { (1u64 << bits) - 1 };
        self.accumulator = (self.accumulator << bits) | (value & mask);
        self.bit_count += bits;

        while self.bit_count >= 8 {
            self.bit_count -= 8;
            let byte = (self.accumulator >> self.bit_count) as u8;
            self.buffer.push(byte);
        }
    }

    pub fn finish(mut self) -> Vec<u8> {
        if self.bit_count > 0 {
            let byte = (self.accumulator << (8 - self.bit_count)) as u8;
            self.buffer.push(byte);
        }
        self.buffer
    }
}

pub struct AlacEncoder {
    sample_rate: u32,
    channels: u16,
    bit_depth: u8,
    frame_size: u32,
}

impl AlacEncoder {
    pub const DEFAULT_FRAME_SIZE: u32 = 352;

    pub fn new(sample_rate: u32, channels: u16, bit_depth: u8) -> Self {
        Self {
            sample_rate,
            channels,
            bit_depth,
            frame_size: Self::DEFAULT_FRAME_SIZE,
        }
    }

    pub fn frame_size(&self) -> u32 {
        self.frame_size
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }

    /// 24-byte Apple ALAC Magic Cookie (RTSP SETUP SDP / binary plist için)
    pub fn magic_cookie(&self) -> Vec<u8> {
        let mut cookie = Vec::with_capacity(24);
        cookie.extend_from_slice(&self.frame_size.to_be_bytes());
        cookie.push(0); // compatibleVersion
        cookie.push(self.bit_depth);
        cookie.push(40); // pb
        cookie.push(10); // mb
        cookie.push(14); // kb
        cookie.push(self.channels as u8);
        cookie.extend_from_slice(&255u16.to_be_bytes()); // maxRun
        let max_frame_bytes = self.frame_size * (self.channels as u32) * ((self.bit_depth / 8) as u32);
        cookie.extend_from_slice(&max_frame_bytes.to_be_bytes());
        cookie.extend_from_slice(&0u32.to_be_bytes()); // avgBitRate
        cookie.extend_from_slice(&self.sample_rate.to_be_bytes());
        cookie
    }

    pub fn sdp_fmtp_param(&self) -> String {
        let max_frame_bytes = self.frame_size * (self.channels as u32) * ((self.bit_depth / 8) as u32);
        format!(
            "{} 0 {} 40 10 14 {} 255 {} 0 {}",
            self.frame_size,
            self.bit_depth,
            self.channels,
            max_frame_bytes,
            self.sample_rate
        )
    }

    /// Bit-hizalı standart ALAC verbatim çerçevesi kodlar (%100 kayıpsız, sıfır işlemci gecikmesi)
    pub fn encode_frame(&self, pcm_samples: &[i16]) -> Result<Vec<u8>> {
        let est_bytes = 2 + pcm_samples.len() * 2 + 1;
        let mut writer = BitWriter::with_capacity(est_bytes);

        // ALAC CPE (Stereo) Başlığı:
        writer.write_bits(1, 3); // element tag = 1 (CPE)
        writer.write_bits(0, 4); // element instance tag = 0
        writer.write_bits(0, 1); // unused = 0
        writer.write_bits(0, 1); // partial frame = 0
        writer.write_bits(0, 2); // wasted bytes = 0
        writer.write_bits(1, 1); // verbatim mode = 1

        for &sample in pcm_samples {
            writer.write_bits(sample as u16 as u64, 16);
        }

        Ok(writer.finish())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alac_bit_alignment() {
        let encoder = AlacEncoder::new(44100, 2, 16);
        let samples = vec![1000i16, -1000i16];
        let frame = encoder.encode_frame(&samples).unwrap();
        // 12 bit başlık + 32 bit örnekler = 44 bit -> 6 bayt
        assert_eq!(frame.len(), 6);
    }
}
