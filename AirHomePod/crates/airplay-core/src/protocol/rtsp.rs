use crate::codec::AlacEncoder;
use crate::error::{AirPlayError, Result};
use crate::protocol::pairing::HomeKitPairing;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

pub struct RtspSession {
    stream: TcpStream,
    target_addr: SocketAddr,
    cseq: u32,
    pub session_id: Option<String>,
    pub server_audio_port: u16,
    pub server_control_port: u16,
    pub server_timing_port: u16,
}

impl RtspSession {
    pub fn connect(target_addr: SocketAddr) -> Result<Self> {
        let stream = TcpStream::connect_timeout(&target_addr, Duration::from_secs(4))
            .map_err(|e| AirPlayError::Rtsp(format!("HomePod RTSP portuna bağlanılamadı: {}", e)))?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;

        Ok(Self {
            stream,
            target_addr,
            cseq: 1,
            session_id: None,
            server_audio_port: 0,
            server_control_port: 0,
            server_timing_port: 0,
        })
    }

    fn next_cseq(&mut self) -> u32 {
        let c = self.cseq;
        self.cseq += 1;
        c
    }

    pub fn send_request(
        &mut self,
        method: &str,
        uri: &str,
        headers: &[(&str, &str)],
        body: Option<&[u8]>,
    ) -> Result<(String, HashMap<String, String>, Vec<u8>)> {
        let cseq = self.next_cseq();
        let mut req = format!(
            "{} {} RTSP/1.0\r\nCSeq: {}\r\nUser-Agent: AirPlay/605.1\r\nDACP-ID: 0123456789ABCDEF\r\nActive-Remote: 1984\r\n",
            method, uri, cseq
        );

        if let Some(ref sess) = self.session_id {
            req.push_str(&format!("Session: {}\r\n", sess));
        }

        for &(k, v) in headers {
            req.push_str(&format!("{}: {}\r\n", k, v));
        }

        if let Some(b) = body {
            req.push_str(&format!("Content-Length: {}\r\n\r\n", b.len()));
            self.stream.write_all(req.as_bytes())?;
            self.stream.write_all(b)?;
        } else {
            req.push_str("\r\n");
            self.stream.write_all(req.as_bytes())?;
        }
        self.stream.flush()?;

        // Yanıt başlığını ve gövdesini tam olarak oku
        let mut header_buf = Vec::new();
        let mut byte = [0u8; 1];
        let mut header_ended = false;

        while !header_ended {
            self.stream.read_exact(&mut byte)?;
            header_buf.push(byte[0]);

            if header_buf.len() >= 4 && &header_buf[header_buf.len() - 4..] == b"\r\n\r\n" {
                header_ended = true;
            }
        }

        let header_str = String::from_utf8_lossy(&header_buf);
        let mut lines = header_str.split("\r\n");
        let status_line = lines.next().unwrap_or("").to_string();

        let mut resp_headers = HashMap::new();
        let mut content_length = 0usize;

        for line in lines {
            if line.is_empty() {
                break;
            }
            if let Some((k, v)) = line.split_once(':') {
                let key = k.trim().to_lowercase();
                let val = v.trim().to_string();
                if key == "content-length" {
                    content_length = val.parse().unwrap_or(0);
                }
                resp_headers.insert(key, val);
            }
        }

        let mut body_bytes = vec![0u8; content_length];
        if content_length > 0 {
            self.stream.read_exact(&mut body_bytes)?;
        }

        Ok((status_line, resp_headers, body_bytes))
    }

    pub fn send_options(&mut self) -> Result<()> {
        let (status, _, _) = self.send_request("OPTIONS", "*", &[], None)?;
        log::info!("RTSP OPTIONS: {}", status);
        Ok(())
    }

    pub fn perform_pairing(&mut self, pairing: &mut HomeKitPairing) -> Result<()> {
        // M1
        let m1 = HomeKitPairing::create_setup_m1();
        let (status, _, m2_resp) = self.send_request(
            "POST",
            "/pair-setup",
            &[
                ("Content-Type", "application/octet-stream"),
                ("X-Apple-HKP", "4"),
            ],
            Some(&m1),
        )?;
        log::info!("Pair-Setup M1/M2 yanıtı: {}", status);

        // M3
        let m3 = pairing.handle_setup_m2(&m2_resp)?;
        let (status, _, m4_resp) = self.send_request(
            "POST",
            "/pair-setup",
            &[
                ("Content-Type", "application/octet-stream"),
                ("X-Apple-HKP", "4"),
            ],
            Some(&m3),
        )?;
        log::info!("Pair-Setup M3/M4 yanıtı: {}", status);

        pairing.handle_setup_m4(&m4_resp)?;
        log::info!("HomeKit SRP-6a el sıkışması başarıyla tamamlandı!");
        Ok(())
    }

    pub fn setup_stream(
        &mut self,
        encoder: &AlacEncoder,
        local_ctrl_port: u16,
        local_timing_port: u16,
    ) -> Result<()> {
        let sdp = format!(
            "v=0\r\no=AirHomePod 0 0 IN IP4 0.0.0.0\r\ns=AirHomePod Audio\r\nc=IN IP4 {}\r\nt=0 0\r\nm=audio 0 RTP/AVP 96\r\na=rtpmap:96 AppleLossless\r\na=fmtp:96 {}\r\n",
            self.target_addr.ip(),
            encoder.sdp_fmtp_param()
        );

        let transport = format!(
            "RTP/AVP/UDP;unicast;interleaved=0-1;mode=record;control_port={};timing_port={}",
            local_ctrl_port, local_timing_port
        );

        let (status, headers, _) = self.send_request(
            "SETUP",
            "rtsp://0.0.0.0/stream",
            &[
                ("Transport", &transport),
                ("Content-Type", "application/sdp"),
            ],
            Some(sdp.as_bytes()),
        )?;

        log::info!("RTSP SETUP: {}", status);

        if let Some(sess) = headers.get("session") {
            self.session_id = Some(sess.clone());
        }

        if let Some(t) = headers.get("transport") {
            for part in t.split(';') {
                if let Some((k, v)) = part.split_once('=') {
                    match k.trim() {
                        "server_port" => self.server_audio_port = v.parse().unwrap_or(0),
                        "control_port" => self.server_control_port = v.parse().unwrap_or(0),
                        "timing_port" => self.server_timing_port = v.parse().unwrap_or(0),
                        _ => {}
                    }
                }
            }
        }

        log::info!(
            "HomePod Portları: Ses UDP={}, Kontrol={}, Zamanlama={}",
            self.server_audio_port,
            self.server_control_port,
            self.server_timing_port
        );

        Ok(())
    }

    pub fn record(&mut self, start_rtp_time: u32) -> Result<()> {
        let rtp_info = format!("seq=1;rtptime={}", start_rtp_time);
        let (status, _, _) = self.send_request(
            "RECORD",
            "rtsp://0.0.0.0/stream",
            &[("Range", "npt=0-"), ("RTP-Info", &rtp_info)],
            None,
        )?;
        log::info!("RTSP RECORD: {}", status);
        Ok(())
    }

    pub fn set_volume(&mut self, volume_percent: f32) -> Result<()> {
        let db = if volume_percent <= 0.0 {
            -144.0 // Mute
        } else {
            -30.0 + (volume_percent / 100.0) * 30.0
        };

        let body = format!("volume: {:.6}\r\n", db);
        let _ = self.send_request(
            "SET_PARAMETER",
            "rtsp://0.0.0.0/stream",
            &[("Content-Type", "text/parameters")],
            Some(body.as_bytes()),
        );
        Ok(())
    }

    pub fn keep_alive(&mut self) -> Result<()> {
        let _ = self.send_request("POST", "/feedback", &[], None);
        Ok(())
    }

    pub fn teardown(&mut self) -> Result<()> {
        let _ = self.send_request("TEARDOWN", "rtsp://0.0.0.0/stream", &[], None);
        Ok(())
    }
}
