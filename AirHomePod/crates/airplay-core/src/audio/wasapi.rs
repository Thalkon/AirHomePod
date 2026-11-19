use crate::audio::resampler::AudioResampler;
use crate::error::Result;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;

pub struct AudioCapture {
    is_running: Arc<AtomicBool>,
    target_sample_rate: u32,
    target_channels: u16,
}

impl AudioCapture {
    pub fn new(target_sample_rate: u32, target_channels: u16) -> Self {
        Self {
            is_running: Arc::new(AtomicBool::new(false)),
            target_sample_rate,
            target_channels,
        }
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    pub fn stop(&self) {
        self.is_running.store(false, Ordering::SeqCst);
    }

    #[cfg(windows)]
    pub fn start(&self, tx: Sender<Vec<i16>>) -> Result<()> {
        use windows::Win32::Foundation::*;
        use windows::Win32::Media::Audio::*;
        use windows::Win32::System::Com::*;

        if self.is_running.swap(true, Ordering::SeqCst) {
            return Ok(());
        }

        let is_running = self.is_running.clone();
        let target_rate = self.target_sample_rate;
        let target_ch = self.target_channels;

        std::thread::spawn(move || {
            unsafe {
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

                let enumerator: IMMDeviceEnumerator = match CoCreateInstance(
                    &MMDeviceEnumerator,
                    None,
                    CLSCTX_ALL,
                ) {
                    Ok(e) => e,
                    Err(e) => {
                        log::error!("WASAPI Device Enumerator hatası: {:?}", e);
                        is_running.store(false, Ordering::SeqCst);
                        return;
                    }
                };

                let device = match enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia) {
                    Ok(d) => d,
                    Err(e) => {
                        log::error!("Varsayılan ses çıkış cihazı alınamadı: {:?}", e);
                        is_running.store(false, Ordering::SeqCst);
                        return;
                    }
                };

                let audio_client: IAudioClient = match device.Activate(CLSCTX_ALL, None) {
                    Ok(c) => c,
                    Err(e) => {
                        log::error!("IAudioClient başlatılamadı: {:?}", e);
                        is_running.store(false, Ordering::SeqCst);
                        return;
                    }
                };

                let pwfx_ptr = match audio_client.GetMixFormat() {
                    Ok(f) => f,
                    Err(e) => {
                        log::error!("Mix formatı okunamadı: {:?}", e);
                        is_running.store(false, Ordering::SeqCst);
                        return;
                    }
                };

                let pwfx = *pwfx_ptr;
                let native_rate = pwfx.nSamplesPerSec;
                let native_ch = pwfx.nChannels;
                let native_bits = pwfx.wBitsPerSample;

                log::info!(
                    "WASAPI Loopback başlatıldı: {} Hz, {} kanal, {}-bit -> Hedef: {} Hz",
                    native_rate,
                    native_ch,
                    native_bits,
                    target_rate
                );

                let buffer_duration = 100_000i64; // 10ms
                if let Err(e) = audio_client.Initialize(
                    AUDCLNT_SHAREMODE_SHARED,
                    AUDCLNT_STREAMFLAGS_LOOPBACK,
                    buffer_duration,
                    0,
                    pwfx_ptr,
                    None,
                ) {
                    log::error!("AudioClient Initialize hatası: {:?}", e);
                    CoTaskMemFree(Some(pwfx_ptr as *const _));
                    is_running.store(false, Ordering::SeqCst);
                    return;
                }
                CoTaskMemFree(Some(pwfx_ptr as *const _));

                let capture_client: IAudioCaptureClient = match audio_client.GetService() {
                    Ok(c) => c,
                    Err(e) => {
                        log::error!("IAudioCaptureClient alınamadı: {:?}", e);
                        is_running.store(false, Ordering::SeqCst);
                        return;
                    }
                };

                if let Err(e) = audio_client.Start() {
                    log::error!("AudioClient Start hatası: {:?}", e);
                    is_running.store(false, Ordering::SeqCst);
                    return;
                }

                let mut resampler = AudioResampler::new(native_rate, target_rate, target_ch);

                while is_running.load(Ordering::SeqCst) {
                    let packet_len = match capture_client.GetNextPacketSize() {
                        Ok(len) => len,
                        Err(_) => 0,
                    };

                    if packet_len == 0 {
                        std::thread::sleep(std::time::Duration::from_millis(5));
                        continue;
                    }

                    let mut data_ptr = std::ptr::null_mut();
                    let mut frames_available = 0u32;
                    let mut flags = 0u32;

                    let hr = capture_client.GetBuffer(
                        &mut data_ptr,
                        &mut frames_available,
                        &mut flags,
                        None,
                        None,
                    );

                    if hr.is_ok() && frames_available > 0 && !data_ptr.is_null() {
                        let is_silent = (flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32) != 0;
                        let mut pcm: Vec<i16> = Vec::with_capacity((frames_available * target_ch as u32) as usize);

                        if is_silent {
                            pcm.resize((frames_available * target_ch as u32) as usize, 0);
                        } else if native_bits == 32 {
                            let floats = std::slice::from_raw_parts(
                                data_ptr as *const f32,
                                (frames_available * native_ch as u32) as usize,
                            );
                            for chunk in floats.chunks_exact(native_ch as usize) {
                                let left = (chunk[0].clamp(-1.0, 1.0) * 32767.0) as i16;
                                let right = if native_ch > 1 {
                                    (chunk[1].clamp(-1.0, 1.0) * 32767.0) as i16
                                } else {
                                    left
                                };
                                pcm.push(left);
                                pcm.push(right);
                            }
                        } else if native_bits == 16 {
                            let shorts = std::slice::from_raw_parts(
                                data_ptr as *const i16,
                                (frames_available * native_ch as u32) as usize,
                            );
                            for chunk in shorts.chunks_exact(native_ch as usize) {
                                pcm.push(chunk[0]);
                                pcm.push(if native_ch > 1 { chunk[1] } else { chunk[0] });
                            }
                        }

                        let _ = capture_client.ReleaseBuffer(frames_available);

                        let resampled = resampler.resample(&pcm);
                        if tx.send(resampled).is_err() {
                            break;
                        }
                    }
                }

                let _ = audio_client.Stop();
                CoUninitialize();
            }
        });

        Ok(())
    }

    #[cfg(not(windows))]
    pub fn start(&self, tx: Sender<Vec<i16>>) -> Result<()> {
        if self.is_running.swap(true, Ordering::SeqCst) {
            return Ok(());
        }

        let is_running = self.is_running.clone();
        let target_rate = self.target_sample_rate;
        let target_ch = self.target_channels;

        std::thread::spawn(move || {
            log::info!("Simülasyon modunda ses yakalama (10ms döngü)");
            let frame_samples = (target_rate / 100) as usize; // 10ms
            let mut phase: f32 = 0.0;

            while is_running.load(Ordering::SeqCst) {
                let start = std::time::Instant::now();
                let mut buffer = Vec::with_capacity(frame_samples * target_ch as usize);

                for _ in 0..frame_samples {
                    let sample = (phase.sin() * 4000.0) as i16;
                    phase += 2.0 * std::f32::consts::PI * 440.0 / target_rate as f32;
                    if phase > 2.0 * std::f32::consts::PI {
                        phase -= 2.0 * std::f32::consts::PI;
                    }
                    for _ in 0..target_ch {
                        buffer.push(sample);
                    }
                }

                if tx.send(buffer).is_err() {
                    break;
                }

                let elapsed = start.elapsed();
                if elapsed < std::time::Duration::from_millis(10) {
                    std::thread::sleep(std::time::Duration::from_millis(10) - elapsed);
                }
            }
        });

        Ok(())
    }
}
