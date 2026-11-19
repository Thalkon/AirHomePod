use crate::audio::AudioCapture;
use crate::codec::AlacEncoder;
use crate::error::Result;
use crate::protocol::rtsp::RtspSession;
use crate::protocol::timing::TimingServer;
use parking_lot::Mutex;
use socket2::{Domain, Protocol, Socket, Type};
use std::collections::VecDeque;
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LatencyProfile {
    LowLatency, // ~180 - 250 ms (Düşük Gecikme Modu)
    Balanced,   // ~350 - 500 ms (Genel Kullanım)
    Stable,     // ~1000 - 1500 ms (Müzik / Yüksek Kararlılık)
}

impl LatencyProfile {
    pub fn target_latency_ms(&self) -> u32 {
        match self {
            Self::LowLatency => 200,
            Self::Balanced => 400,
            Self::Stable => 1200,
        }
    }
}

pub struct AirPlayStreamer {
    is_streaming: Arc<AtomicBool>,
    latency_profile: LatencyProfile,
    sample_rate: u32,
    packets_sent: Arc<AtomicU32>,
    retransmit_count: Arc<AtomicU32>,
}

impl AirPlayStreamer {
    pub fn new(latency_profile: LatencyProfile, sample_rate: u32) -> Self {
        Self {
            is_streaming: Arc::new(AtomicBool::new(false)),
            latency_profile,
            sample_rate,
            packets_sent: Arc::new(AtomicU32::new(0)),
            retransmit_count: Arc::new(AtomicU32::new(0)),
        }
    }

    pub fn is_streaming(&self) -> bool {
        self.is_streaming.load(Ordering::SeqCst)
    }

    pub fn packets_sent(&self) -> u32 {
        self.packets_sent.load(Ordering::Relaxed)
    }

    pub fn retransmits(&self) -> u32 {
        self.retransmit_count.load(Ordering::Relaxed)
    }

    pub fn stop(&self) {
        self.is_streaming.store(false, Ordering::SeqCst);
    }

    pub fn start(
        &self,
        target_addr: SocketAddr,
        rtsp_session: Arc<Mutex<RtspSession>>,
        encoder: Arc<AlacEncoder>,
        capture: Arc<AudioCapture>,
    ) -> Result<()> {
        if self.is_streaming.swap(true, Ordering::SeqCst) {
            return Ok(());
        }

        let is_streaming = self.is_streaming.clone();
        let packets_sent = self.packets_sent.clone();
        let retransmits = self.retransmit_count.clone();
        let sample_rate = self.sample_rate;

        // 1. DSCP QoS (EF = 0xb8) destekli ses soketi oluştur
        let audio_socket = {
            let sock = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
            sock.set_nonblocking(false)?;
            let _ = sock.set_tos(0xb8); // Expedited Forwarding (QoS)
            sock.bind(&"0.0.0.0:0".parse::<SocketAddr>().unwrap().into())?;
            Arc::new(UdpSocket::from(sock))
        };

        let timing_socket = Arc::new(UdpSocket::bind("0.0.0.0:0")?);
        let control_socket = Arc::new(UdpSocket::bind("0.0.0.0:0")?);

        let local_timing_port = timing_socket.local_addr()?.port();
        let local_ctrl_port = control_socket.local_addr()?.port();

        // 2. RTSP SETUP
        {
            let mut session = rtsp_session.lock();
            session.setup_stream(&encoder, local_ctrl_port, local_timing_port)?;
        }

        let (server_audio_addr, server_timing_addr) = {
            let session = rtsp_session.lock();
            (
                SocketAddr::new(target_addr.ip(), session.server_audio_port),
                SocketAddr::new(target_addr.ip(), session.server_timing_port),
            )
        };

        // 3. NTP Timing Sunucusu
        let timing_server = Arc::new(TimingServer::new(sample_rate));
        let start_rtp_time = timing_server.current_rtp_timestamp();
        timing_server.start_responder(timing_socket.clone());

        // 4. RTSP RECORD
        {
            let mut session = rtsp_session.lock();
            session.record(start_rtp_time)?;
        }

        // 5. Keep-Alive döngüsü
        let rtsp_keep = rtsp_session.clone();
        let keep_running = is_streaming.clone();
        std::thread::spawn(move || {
            while keep_running.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_secs(3));
                let mut session = rtsp_keep.lock();
                let _ = session.keep_alive();
            }
        });

        // 6. Retransmit halka tamponu (Son 256 paket)
        let ring_buffer: Arc<Mutex<VecDeque<(u16, Vec<u8>)>>> =
            Arc::new(Mutex::new(VecDeque::with_capacity(256)));

        // Retransmit dinleyicisi
        let ctrl_sock = control_socket.clone();
        let ring_clone = ring_buffer.clone();
        let retr_running = is_streaming.clone();
        let retr_count = retransmits.clone();
        let audio_sock_clone = audio_socket.clone();

        std::thread::spawn(move || {
            let mut buf = [0u8; 128];
            let _ = ctrl_sock.set_read_timeout(Some(Duration::from_millis(500)));

            while retr_running.load(Ordering::Relaxed) {
                if let Ok((len, _peer)) = ctrl_sock.recv_from(&mut buf) {
                    // AirPlay Retransmit İsteği: 0x80 | 0x55 (Paket Tipi)
                    if len >= 8 && (buf[1] & 0x7F) == 0x55 {
                        let req_seq = u16::from_be_bytes([buf[4], buf[5]]);
                        let req_count = u16::from_be_bytes([buf[6], buf[7]]).max(1);

                        let ring = ring_clone.lock();
                        for i in 0..req_count {
                            let target_seq = req_seq.wrapping_add(i);
                            if let Some((_, pkt)) = ring.iter().find(|(s, _)| *s == target_seq) {
                                let _ = audio_sock_clone.send_to(pkt, server_audio_addr);
                                retr_count.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                    }
                }
            }
        });

        // 7. Ses Yakalama ve İletim Borusu
        let (tx, rx): (std::sync::mpsc::Sender<Vec<i16>>, Receiver<Vec<i16>>) = channel();
        capture.start(tx)?;

        let streamer_running = is_streaming.clone();
        let frame_size = encoder.frame_size() as usize;

        std::thread::spawn(move || {
            let mut sample_accumulator = Vec::with_capacity(frame_size * 2);
            let mut rtp_seq: u16 = 1;
            let mut rtp_timestamp = start_rtp_time;

            while streamer_running.load(Ordering::Relaxed) {
                match rx.recv_timeout(Duration::from_millis(50)) {
                    Ok(pcm) => {
                        sample_accumulator.extend_from_slice(&pcm);

                        while sample_accumulator.len() >= frame_size * 2 {
                            let frame_pcm: Vec<i16> =
                                sample_accumulator.drain(..frame_size * 2).collect();

                            if let Ok(encoded) = encoder.encode_frame(&frame_pcm) {
                                let mut packet = Vec::with_capacity(12 + encoded.len());
                                packet.push(0x80); // Version 2
                                packet.push(0x60); // Payload Type 96 (Dynamic ALAC)
                                packet.extend_from_slice(&rtp_seq.to_be_bytes());
                                packet.extend_from_slice(&rtp_timestamp.to_be_bytes());
                                packet.extend_from_slice(&0x11223344u32.to_be_bytes()); // SSRC
                                packet.extend_from_slice(&encoded);

                                let _ = audio_socket.send_to(&packet, server_audio_addr);

                                // Retransmit tamponuna ekle
                                {
                                    let mut ring = ring_buffer.lock();
                                    if ring.len() >= 256 {
                                        ring.pop_front();
                                    }
                                    ring.push_back((rtp_seq, packet));
                                }

                                rtp_seq = rtp_seq.wrapping_add(1);
                                rtp_timestamp = rtp_timestamp.wrapping_add(frame_size as u32);
                                packets_sent.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                    }
                    Err(_) => {
                        // Sessizlik anlarında boş çerçeve (352 adet sıfır örnek) üretip gönder
                        let silence_pcm = vec![0i16; frame_size * 2];
                        if let Ok(encoded) = encoder.encode_frame(&silence_pcm) {
                            let mut packet = Vec::with_capacity(12 + encoded.len());
                            packet.push(0x80);
                            packet.push(0x60);
                            packet.extend_from_slice(&rtp_seq.to_be_bytes());
                            packet.extend_from_slice(&rtp_timestamp.to_be_bytes());
                            packet.extend_from_slice(&0x11223344u32.to_be_bytes());
                            packet.extend_from_slice(&encoded);

                            let _ = audio_socket.send_to(&packet, server_audio_addr);
                            rtp_seq = rtp_seq.wrapping_add(1);
                            rtp_timestamp = rtp_timestamp.wrapping_add(frame_size as u32);
                        }
                    }
                }
            }

            log::info!("Ses aktarım döngüsü temiz bir şekilde sonlandırıldı.");
        });

        Ok(())
    }
}
