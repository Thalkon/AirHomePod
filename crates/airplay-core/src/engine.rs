use crate::audio::AudioCapture;
use crate::codec::AlacEncoder;
use crate::error::{AirPlayError, Result};
use crate::protocol::discovery::DeviceDiscovery;
use crate::protocol::pairing::HomeKitPairing;
use crate::protocol::rtsp::RtspSession;
use crate::protocol::streamer::{AirPlayStreamer, LatencyProfile};
use parking_lot::Mutex;
use std::net::SocketAddr;
use std::sync::Arc;

pub struct Engine {
    pub discovery: Option<Arc<DeviceDiscovery>>,
    pub streamer: Option<Arc<AirPlayStreamer>>,
    pub rtsp_session: Option<Arc<Mutex<RtspSession>>>,
    pub capture: Option<Arc<AudioCapture>>,
    pub target_addr: Option<SocketAddr>,
    pub current_volume: f32,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            discovery: None,
            streamer: None,
            rtsp_session: None,
            capture: None,
            target_addr: None,
            current_volume: 80.0,
        }
    }

    pub fn connect(
        &mut self,
        target_addr: SocketAddr,
        pin: &str,
        profile: LatencyProfile,
    ) -> Result<()> {
        self.disconnect();

        let mut rtsp = RtspSession::connect(target_addr)?;
        rtsp.send_options()?;

        let mut pairing = HomeKitPairing::new(pin);
        if let Err(e) = rtsp.perform_pairing(&mut pairing) {
            log::warn!("HomeKit pairing bildirimi: {}", e);
        }

        let encoder = Arc::new(AlacEncoder::new(44100, 2, 16));
        let capture = Arc::new(AudioCapture::new(44100, 2));
        let streamer = Arc::new(AirPlayStreamer::new(profile, 44100));

        let rtsp_shared = Arc::new(Mutex::new(rtsp));

        streamer.start(
            target_addr,
            rtsp_shared.clone(),
            encoder,
            capture.clone(),
        )?;

        // Başlangıç ses seviyesini ayarla
        {
            let mut session = rtsp_shared.lock();
            let _ = session.set_volume(self.current_volume);
        }

        self.streamer = Some(streamer);
        self.capture = Some(capture);
        self.rtsp_session = Some(rtsp_shared);
        self.target_addr = Some(target_addr);

        Ok(())
    }

    pub fn set_volume(&mut self, volume_percent: f32) {
        self.current_volume = volume_percent;
        if let Some(ref rtsp) = self.rtsp_session {
            let mut session = rtsp.lock();
            let _ = session.set_volume(volume_percent);
        }
    }

    pub fn disconnect(&mut self) {
        if let Some(streamer) = self.streamer.take() {
            streamer.stop();
        }
        if let Some(capture) = self.capture.take() {
            capture.stop();
        }
        if let Some(rtsp) = self.rtsp_session.take() {
            let mut session = rtsp.lock();
            let _ = session.teardown();
        }
        self.target_addr = None;
    }

    pub fn get_stats(&self) -> (u32, u32, bool) {
        if let Some(ref streamer) = self.streamer {
            (
                streamer.packets_sent(),
                streamer.retransmits(),
                streamer.is_streaming(),
            )
        } else {
            (0, 0, false)
        }
    }
}
