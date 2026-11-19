use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub struct TimingServer {
    is_running: Arc<AtomicBool>,
    epoch_start: Instant,
    sample_rate: u32,
    base_rtp_timestamp: u32,
}

impl TimingServer {
    pub const NTP_OFFSET_SECS: u64 = 2_208_988_800;

    pub fn new(sample_rate: u32) -> Self {
        Self {
            is_running: Arc::new(AtomicBool::new(false)),
            epoch_start: Instant::now(),
            sample_rate,
            base_rtp_timestamp: rand::random::<u32>(),
        }
    }

    pub fn current_ntp_timestamp() -> u64 {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO);
        let ntp_secs = now.as_secs() + Self::NTP_OFFSET_SECS;
        let ntp_frac = ((now.subsec_nanos() as u64) << 32) / 1_000_000_000;
        (ntp_secs << 32) | (ntp_frac & 0xFFFF_FFFF)
    }

    pub fn current_rtp_timestamp(&self) -> u32 {
        let elapsed = self.epoch_start.elapsed().as_secs_f64();
        let samples = (elapsed * self.sample_rate as f64) as u32;
        self.base_rtp_timestamp.wrapping_add(samples)
    }

    pub fn stop(&self) {
        self.is_running.store(false, Ordering::SeqCst);
    }

    /// HomePod'un gönderdiği NTP zamanlama isteklerini yanıtlayan sunucu döngüsü
    pub fn start_responder(&self, socket: Arc<UdpSocket>) {
        let is_running = self.is_running.clone();
        is_running.store(true, Ordering::SeqCst);

        let epoch = self.epoch_start;
        let rate = self.sample_rate;
        let base_rtp = self.base_rtp_timestamp;

        std::thread::spawn(move || {
            let mut buf = [0u8; 128];
            let _ = socket.set_read_timeout(Some(Duration::from_millis(500)));

            while is_running.load(Ordering::Relaxed) {
                if let Ok((len, peer)) = socket.recv_from(&mut buf) {
                    if len >= 32 {
                        let rx_ntp = Self::current_ntp_timestamp();
                        let elapsed = epoch.elapsed().as_secs_f64();
                        let cur_rtp = base_rtp.wrapping_add((elapsed * rate as f64) as u32);

                        // AirPlay Timing Yanıtı (32 bayt)
                        let mut resp = [0u8; 32];
                        resp[0] = 0x80 | 0x04; // NTP reply flag
                        resp[1] = 0x53 | 0x80; // Timing packet type

                        // Originate timestamp (istemciden gelen transmit timestamp: offset 24..32)
                        resp[8..16].copy_from_slice(&buf[24..32]);
                        // Receive timestamp
                        resp[16..24].copy_from_slice(&rx_ntp.to_be_bytes());
                        // Transmit timestamp
                        let tx_ntp = Self::current_ntp_timestamp();
                        resp[24..32].copy_from_slice(&tx_ntp.to_be_bytes());

                        let _ = socket.send_to(&resp, peer);
                    }
                }
            }
        });
    }
}
