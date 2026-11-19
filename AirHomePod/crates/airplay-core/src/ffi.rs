use crate::engine::Engine;
use crate::logging;
use crate::protocol::discovery::DeviceDiscovery;
use crate::protocol::streamer::LatencyProfile;
use parking_lot::Mutex;
use std::ffi::{CStr, CString};
use std::net::SocketAddr;
use std::os::raw::{c_char, c_int};
use std::sync::Arc;

pub type DeviceDiscoveredCallback = extern "C" fn(
    name: *const c_char,
    ip: *const c_char,
    model: *const c_char,
    port: u16,
);

pub type StatusCallback = extern "C" fn(status_code: c_int, message: *const c_char);

static ENGINE: Mutex<Option<Engine>> = Mutex::new(None);

#[no_mangle]
pub extern "C" fn airplay_init() -> c_int {
    logging::init_file_logger();
    let mut state = ENGINE.lock();
    if state.is_none() {
        *state = Some(Engine::new());
    }
    0
}

#[no_mangle]
pub extern "C" fn airplay_start_discovery(callback: DeviceDiscoveredCallback) -> c_int {
    let mut state_guard = ENGINE.lock();
    let engine = match state_guard.as_mut() {
        Some(e) => e,
        None => return -1,
    };

    let discovery = match DeviceDiscovery::new() {
        Ok(d) => Arc::new(d),
        Err(e) => {
            log::error!("mDNS başlatılamadı: {:?}", e);
            return -2;
        }
    };

    let rx = match discovery.start_discovery() {
        Ok(rx) => rx,
        Err(e) => {
            log::error!("mDNS browse hatası: {:?}", e);
            return -3;
        }
    };

    engine.discovery = Some(discovery);

    std::thread::spawn(move || {
        while let Ok(dev) = rx.recv() {
            let name_c = CString::new(dev.name).unwrap_or_default();
            let ip_c = CString::new(dev.ip.to_string()).unwrap_or_default();
            let model_c = CString::new(dev.model).unwrap_or_default();

            callback(name_c.as_ptr(), ip_c.as_ptr(), model_c.as_ptr(), dev.port);
        }
    });

    0
}

#[no_mangle]
pub extern "C" fn airplay_stop_discovery() -> c_int {
    let mut state_guard = ENGINE.lock();
    if let Some(engine) = state_guard.as_mut() {
        if let Some(d) = engine.discovery.take() {
            d.stop_discovery();
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn airplay_connect(
    ip_ptr: *const c_char,
    port: u16,
    pin_ptr: *const c_char,
    latency_mode: c_int,
    callback: StatusCallback,
) -> c_int {
    if ip_ptr.is_null() {
        return -1;
    }

    let ip_str = unsafe {
        match CStr::from_ptr(ip_ptr).to_str() {
            Ok(s) => s.to_string(),
            Err(_) => return -2,
        }
    };

    let pin_str = if pin_ptr.is_null() {
        "3939".to_string()
    } else {
        unsafe {
            CStr::from_ptr(pin_ptr)
                .to_str()
                .unwrap_or("3939")
                .to_string()
        }
    };

    let target_addr: SocketAddr = match format!("{}:{}", ip_str, port).parse() {
        Ok(a) => a,
        Err(_) => return -3,
    };

    let profile = match latency_mode {
        0 => LatencyProfile::LowLatency,
        1 => LatencyProfile::Balanced,
        _ => LatencyProfile::Stable,
    };

    std::thread::spawn(move || {
        let msg = CString::new("HomePod ile güvenli bağlantı kuruluyor...").unwrap();
        callback(1, msg.as_ptr());

        let mut state_guard = ENGINE.lock();
        if let Some(engine) = state_guard.as_mut() {
            match engine.connect(target_addr, &pin_str, profile) {
                Ok(_) => {
                    let ok_msg = CString::new("HomePod mini'ye başarıyla bağlanıldı!").unwrap();
                    callback(200, ok_msg.as_ptr());
                }
                Err(e) => {
                    let err_msg = CString::new(format!("Bağlantı hatası: {}", e)).unwrap();
                    callback(-1, err_msg.as_ptr());
                }
            }
        }
    });

    0
}

#[no_mangle]
pub extern "C" fn airplay_disconnect() -> c_int {
    let mut state_guard = ENGINE.lock();
    if let Some(engine) = state_guard.as_mut() {
        engine.disconnect();
    }
    0
}

#[no_mangle]
pub extern "C" fn airplay_set_volume(volume_percent: f32) -> c_int {
    let mut state_guard = ENGINE.lock();
    if let Some(engine) = state_guard.as_mut() {
        engine.set_volume(volume_percent);
        return 0;
    }
    -1
}

#[no_mangle]
pub extern "C" fn airplay_get_stats(
    out_packets_sent: *mut u32,
    out_retransmits: *mut u32,
    out_is_streaming: *mut c_int,
) -> c_int {
    let state_guard = ENGINE.lock();
    if let Some(engine) = state_guard.as_ref() {
        let (packets, retransmits, is_streaming) = engine.get_stats();
        unsafe {
            if !out_packets_sent.is_null() {
                *out_packets_sent = packets;
            }
            if !out_retransmits.is_null() {
                *out_retransmits = retransmits;
            }
            if !out_is_streaming.is_null() {
                *out_is_streaming = if is_streaming { 1 } else { 0 };
            }
        }
        return 0;
    }
    -1
}
