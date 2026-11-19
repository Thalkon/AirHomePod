#!/usr/bin/env python3
"""
HomePod mini Baglanti, mDNS ve Gecikme Test Araci
Windows veya Mac uzerinde calistirarak yerel agdaki HomePod'un erisilebilirligini
ve AirPlay 2 portlarini test eder.
"""

import socket
import struct
import time
import sys

MDNS_ADDR = "224.0.0.251"
MDNS_PORT = 5353

def test_tcp_port(ip, port, timeout=2.0):
    start = time.time()
    try:
        sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        sock.settimeout(timeout)
        result = sock.connect_ex((ip, port))
        elapsed_ms = (time.time() - start) * 1000.0
        sock.close()
        return result == 0, elapsed_ms
    except Exception as e:
        return False, 0.0

def query_mdns_devices(timeout=3.0):
    print(f"[*] mDNS (_airplay._tcp.local) uzerinden HomePod araniyor...")
    query = bytearray([
        0x00, 0x00, # Transaction ID
        0x00, 0x00, # Flags
        0x00, 0x01, # Questions: 1
        0x00, 0x00, # Answer RRs
        0x00, 0x00, # Authority RRs
        0x00, 0x00, # Additional RRs
        # QNAME: _airplay._tcp.local
        0x08, ord('_'), ord('a'), ord('i'), ord('r'), ord('p'), ord('l'), ord('a'), ord('y'),
        0x04, ord('_'), ord('t'), ord('c'), ord('p'),
        0x05, ord('l'), ord('o'), ord('c'), ord('a'), ord('l'),
        0x00,
        0x00, 0x0c, # Type: PTR (12)
        0x00, 0x01, # Class: IN (1)
    ])

    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM, socket.IPPROTO_UDP)
    sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    sock.settimeout(timeout)

    try:
        sock.sendto(query, (MDNS_ADDR, MDNS_PORT))
    except Exception as e:
        print(f"[-] mDNS sorgusu gonderilemedi: {e}")
        return []

    discovered = []
    end_time = time.time() + timeout

    while time.time() < end_time:
        try:
            data, addr = sock.recvfrom(4096)
            ip = addr[0]
            if ip not in discovered:
                discovered.append(ip)
                print(f"[+] AirPlay servisi yanit verdi: {ip}")
        except socket.timeout:
            break
        except Exception:
            break

    sock.close()
    return discovered

def main():
    print("=" * 60)
    print("  HomePod mini AirPlay Baglanti ve Gecikme Testi")
    print("=" * 60)

    devices = query_mdns_devices()

    if not devices:
        print("\n[!] mDNS ile otomatik cihaz bulunamadi.")
        manual_ip = input("HomePod'un yerel IP adresini girin (veya cikmak icin Enter): ").strip()
        if manual_ip:
            devices = [manual_ip]
        else:
            print("Cikis yapiliyor.")
            return

    for ip in devices:
        print(f"\n[*] Cihaz Test Ediliyor: {ip}")
        
        # Port 7000 (AirPlay 2 / RTSP)
        ok7000, ms7000 = test_tcp_port(ip, 7000)
        status_7000 = f"ACIK ({ms7000:.1f} ms)" if ok7000 else "KAPALI"
        print(f"  - Port 7000 (AirPlay 2 RTSP/HomeKit): {status_7000}")

        # Port 5000 (AirPlay Standart)
        ok5000, ms5000 = test_tcp_port(ip, 5000)
        status_5000 = f"ACIK ({ms5000:.1f} ms)" if ok5000 else "KAPALI"
        print(f"  - Port 5000 (AirPlay Audio): {status_5000}")

        if ok7000 or ok5000:
            active_ms = ms7000 if ok7000 else ms5000
            print(f"\n[SONUC] {ip} basariyla yanit veriyor!")
            print(f"  Tahmini Ag RTT Gecikmesi: {active_ms:.1f} ms")
            if active_ms < 10.0:
                print("  -> Mukemmel ag kalitesi! Ultra-Low Latency (<100ms) icin hazir.")
            elif active_ms < 30.0:
                print("  -> Iyi ag kalitesi. Denge modu (~150ms) onerilir.")
            else:
                print("  -> Ag gecikmesi yuksek. Wi-Fi sinyalini veya 5GHz kanalini kontrol edin.")

if __name__ == "__main__":
    main()
