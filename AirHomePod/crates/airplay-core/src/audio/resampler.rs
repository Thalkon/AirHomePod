pub struct AudioResampler {
    from_rate: u32,
    to_rate: u32,
    channels: usize,
    fractional_pos: f64,
    last_samples: Vec<i16>,
}

impl AudioResampler {
    pub fn new(from_rate: u32, to_rate: u32, channels: u16) -> Self {
        let ch = channels as usize;
        Self {
            from_rate,
            to_rate,
            channels: ch,
            fractional_pos: 0.0,
            last_samples: vec![0i16; ch],
        }
    }

    pub fn resample(&mut self, input: &[i16]) -> Vec<i16> {
        if self.from_rate == self.to_rate || input.is_empty() {
            return input.to_vec();
        }

        let ratio = self.from_rate as f64 / self.to_rate as f64;
        let in_frames = input.len() / self.channels;
        let out_est = ((in_frames as f64) / ratio).ceil() as usize;
        let mut output = Vec::with_capacity(out_est * self.channels);

        let ch = self.channels;
        let mut pos = self.fractional_pos;

        while (pos as usize) < in_frames {
            let idx = pos as usize;
            let frac = pos - idx as f64;

            for c in 0..ch {
                let s0 = if idx == 0 {
                    self.last_samples[c] as f64
                } else {
                    input[(idx - 1) * ch + c] as f64
                };
                let s1 = input[idx * ch + c] as f64;

                let interpolated = s0 + (s1 - s0) * frac;
                output.push(interpolated.clamp(-32768.0, 32767.0) as i16);
            }

            pos += ratio;
        }

        // Kalan pozisyonu ve son örneği sakla
        self.fractional_pos = pos - in_frames as f64;
        if in_frames > 0 {
            for c in 0..ch {
                self.last_samples[c] = input[(in_frames - 1) * ch + c];
            }
        }

        output
    }
}
