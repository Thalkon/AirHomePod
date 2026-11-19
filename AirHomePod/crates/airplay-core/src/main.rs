use airplay_core::engine::Engine;
use airplay_core::logging;
use airplay_core::protocol::discovery::DeviceDiscovery;
use airplay_core::protocol::streamer::LatencyProfile;
use std::net::SocketAddr;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    logging::init_console_logger();

    println!("=======================================================");
    println!("  AirHomePod CLI: HomePod mini Düşük Gecikmeli Yayın  ");
    println!("=======================================================");

    let discovery = DeviceDiscovery::new()?;
    let rx = discovery.start_discovery()?;

    println!("Ağdaki HomePod ve AirPlay cihazları taranıyor (5 saniye)...");

    let mut found_devices = Vec::new();
    let start = std::time::Instant::now();

    while start.elapsed() < Duration::from_secs(5) {
        if let Ok(dev) = rx.recv_timeout(Duration::from_millis(500)) {
            println!("-> Bulundu: {} ({}) @ {}:{}", dev.name, dev.model, dev.ip, dev.port);
            found_devices.push(dev);
        }
    }

    if found_devices.is_empty() {
        println!("Ağda HomePod cihazı bulunamadı. Lütfen cihazın açık ve aynı Wi-Fi ağında olduğundan emin olun.");
        return Ok(());
    }

    let target = &found_devices[0];
    println!("\n'{}' cihazına bağlanılıyor ({}:{})...", target.name, target.ip, target.port);

    let target_addr: SocketAddr = format!("{}:{}", target.ip, target.port).parse()?;
    let mut engine = Engine::new();

    engine.connect(target_addr, "3939", LatencyProfile::LowLatency)?;

    println!("\nSes akışı başlatıldı! (Düşük Gecikme Modu ~200ms)");
    println!("Çıkmak için Enter tuşuna basın...\n");

    let mut input = String::new();
    let _ = std::io::stdin().read_line(&mut input);

    println!("Yayın durduruluyor...");
    engine.disconnect();

    Ok(())
}
