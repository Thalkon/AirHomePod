use crate::error::{AirPlayError, Result};
use mdns_sd::{ServiceDaemon, ServiceEvent};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct DiscoveredDevice {
    pub name: String,
    pub ip: IpAddr,
    pub port: u16,
    pub model: String,
    pub is_homepod: bool,
}

pub struct DeviceDiscovery {
    daemon: ServiceDaemon,
    is_scanning: Arc<AtomicBool>,
}

impl DeviceDiscovery {
    pub const AIRPLAY_SERVICE: &'static str = "_airplay._tcp.local.";

    pub fn new() -> Result<Self> {
        let daemon = ServiceDaemon::new().map_err(|e| AirPlayError::Mdns(e.to_string()))?;
        Ok(Self {
            daemon,
            is_scanning: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Thread-safe cihaz keşfi: Tokio gerektirmez, doğrudan standart thread üzerinde çalışır
    pub fn start_discovery(&self) -> Result<Receiver<DiscoveredDevice>> {
        let (tx, rx) = channel();
        let receiver = self
            .daemon
            .browse(Self::AIRPLAY_SERVICE)
            .map_err(|e| AirPlayError::Mdns(e.to_string()))?;

        self.is_scanning.store(true, Ordering::SeqCst);
        let is_scanning = self.is_scanning.clone();

        std::thread::spawn(move || {
            log::info!("mDNS AirPlay keşif döngüsü başlatıldı...");
            while is_scanning.load(Ordering::Relaxed) {
                match receiver.recv() {
                    Ok(event) => {
                        if let ServiceEvent::ServiceResolved(info) = event {
                            let name = info
                                .get_fullname()
                                .split('.')
                                .next()
                                .unwrap_or("HomePod")
                                .to_string();

                            if let Some(&ip) = info.get_addresses().iter().next() {
                                let port = info.get_port();
                                let mut txt = HashMap::new();
                                for p in info.get_properties().iter() {
                                    txt.insert(p.key().to_string(), p.val_str().to_string());
                                }

                                let model = txt
                                    .get("model")
                                    .cloned()
                                    .unwrap_or_else(|| "Unknown".to_string());
                                let is_homepod =
                                    model.contains("AudioAccessory") || model.contains("HomePod");

                                let dev = DiscoveredDevice {
                                    name,
                                    ip,
                                    port,
                                    model,
                                    is_homepod,
                                };

                                log::info!(
                                    "Cihaz algılandı: {} ({}) [{}:{}]",
                                    dev.name,
                                    dev.model,
                                    dev.ip,
                                    dev.port
                                );

                                if tx.send(dev).is_err() {
                                    break;
                                }
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(rx)
    }

    pub fn stop_discovery(&self) {
        self.is_scanning.store(false, Ordering::SeqCst);
        let _ = self.daemon.stop_browse(Self::AIRPLAY_SERVICE);
    }
}
