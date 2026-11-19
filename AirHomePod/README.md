# AirHomePod: Windows 11 Ultra-Low Latency AirPlay 2 Streamer for HomePod mini

Bu proje, Windows 11 sistem sesini Apple HomePod mini'ye en düşük gecikme (<100ms), %100 kayıpsız ses kalitesi (ALAC) ve tam HomeKit protokol uyumluluğu ile aktaran native bir sistem tepsisi uygulamasıdır.

## Mimari

* **`crates/airplay-core` (Rust)**:
  - **WASAPI Loopback Capture**: Windows 11 sistem sesini 5-10ms tamponla sıfır gecikmeli okur.
  - **ALAC Sıkıştırıcı**: Apple Lossless Audio Codec ile bit-perfect kayıpsız ses paketleme.
  - **HomeKit Pairing**: SRP-6a ve ChaCha20-Poly1305 ile şifreli transient el sıkışması.
  - **AirPlay 2 RTSP & PTP**: RTSP oturum yönetimi ve PTP mikro-saniye saat senkronizasyonu.
  - **RTP / UDP**: Düşük gecikmeli canlı ses yayını.
  - **C-ABI**: C# arayüzü ile iletişim için `airplay_core.dll` çıktısı verir.

* **`apps/AirHomePod.Tray` (C# .NET 8 / WPF Windows 11 Fluent)**:
  - Sistem tepsisinde (System Tray / Notification Area) çalışan modern Windows 11 arayüzü.
  - Otomatik mDNS cihaz bulma, tek tıkla bağlanma, donanımsal ses kontrolü ve gecikme profili seçimi.

## Windows 11 Üzerinde Derleme Gereksinimleri

1. **Rust**: `rustup` ve `stable-x86_64-pc-windows-msvc` toolchain.
2. **.NET SDK**: .NET 8.0 SDK veya üzeri.
3. **HomePod Ayarı**: iPhone/iPad "Ev" uygulamasında *Ev Ayarları > Hoparlör ve TV Erişimine İzin Ver* -> **"Aynı Ağdaki Herkes"** olmalıdır.

## Hızlı Başlatma

```cmd
:: 1. Rust motorunu derleyin
cd crates\airplay-core
cargo build --release

:: 2. C# Arayüzünü çalıştırın
cd ..\..\apps\AirHomePod.Tray
dotnet run -c Release
```
