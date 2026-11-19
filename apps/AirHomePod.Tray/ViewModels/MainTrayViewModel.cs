using System;
using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Linq;
using System.Runtime.CompilerServices;
using System.Windows;
using System.Windows.Threading;
using AirHomePod.Tray.Models;
using AirHomePod.Tray.Native;

namespace AirHomePod.Tray.ViewModels
{
    public class MainTrayViewModel : INotifyPropertyChanged
    {
        private readonly Dispatcher _dispatcher;
        private HomePodDevice? _selectedDevice;
        private bool _isConnected;
        private bool _isConnecting;
        private string _statusMessage = "Cihazlar taranıyor...";
        private int _latencyMode = 0; // 0: LowLatency, 1: Balanced, 2: Stable
        private float _volume = 80.0f;
        private uint _packetsSent = 0;
        private uint _retransmits = 0;
        private DispatcherTimer? _statsTimer;
        private DispatcherTimer? _volumeThrottleTimer;
        private float _pendingVolume;

        // GC tarafından toplanmaması için kalıcı referanslar
        private readonly AirPlayNative.DeviceDiscoveredCallback _deviceCallback;
        private readonly AirPlayNative.StatusCallback _statusCallback;

        public ObservableCollection<HomePodDevice> Devices { get; } = new();

        public event PropertyChangedEventHandler? PropertyChanged;

        public MainTrayViewModel()
        {
            _dispatcher = Application.Current?.Dispatcher ?? Dispatcher.CurrentDispatcher;
            _deviceCallback = OnDeviceDiscovered;
            _statusCallback = OnStatusUpdate;

            InitializeEngine();
        }

        public HomePodDevice? SelectedDevice
        {
            get => _selectedDevice;
            set
            {
                if (_selectedDevice != value)
                {
                    _selectedDevice = value;
                    OnPropertyChanged();
                    OnPropertyChanged(nameof(CanConnect));
                }
            }
        }

        public bool IsConnected
        {
            get => _isConnected;
            set
            {
                if (_isConnected != value)
                {
                    _isConnected = value;
                    OnPropertyChanged();
                    OnPropertyChanged(nameof(CanConnect));
                    OnPropertyChanged(nameof(CanDisconnect));
                }
            }
        }

        public bool IsConnecting
        {
            get => _isConnecting;
            set
            {
                if (_isConnecting != value)
                {
                    _isConnecting = value;
                    OnPropertyChanged();
                    OnPropertyChanged(nameof(CanConnect));
                }
            }
        }

        public string StatusMessage
        {
            get => _statusMessage;
            set
            {
                if (_statusMessage != value)
                {
                    _statusMessage = value;
                    OnPropertyChanged();
                }
            }
        }

        public int LatencyMode
        {
            get => _latencyMode;
            set
            {
                if (_latencyMode != value)
                {
                    _latencyMode = value;
                    OnPropertyChanged();
                }
            }
        }

        public float Volume
        {
            get => _volume;
            set
            {
                if (Math.Abs(_volume - value) > 0.5f)
                {
                    _volume = value;
                    OnPropertyChanged();
                    QueueVolumeUpdate(value);
                }
            }
        }

        public uint PacketsSent
        {
            get => _packetsSent;
            set
            {
                if (_packetsSent != value)
                {
                    _packetsSent = value;
                    OnPropertyChanged();
                }
            }
        }

        public uint Retransmits
        {
            get => _retransmits;
            set
            {
                if (_retransmits != value)
                {
                    _retransmits = value;
                    OnPropertyChanged();
                }
            }
        }

        public bool CanConnect => SelectedDevice != null && !IsConnected && !IsConnecting;
        public bool CanDisconnect => IsConnected;

        private void InitializeEngine()
        {
            try
            {
                AirPlayNative.airplay_init();
                AirPlayNative.airplay_start_discovery(_deviceCallback);

                _statsTimer = new DispatcherTimer
                {
                    Interval = TimeSpan.FromSeconds(1)
                };
                _statsTimer.Tick += (s, e) =>
                {
                    if (IsConnected)
                    {
                        AirPlayNative.airplay_get_stats(out uint packets, out uint retr, out int isStreaming);
                        PacketsSent = packets;
                        Retransmits = retr;
                    }
                };
                _statsTimer.Start();

                _volumeThrottleTimer = new DispatcherTimer
                {
                    Interval = TimeSpan.FromMilliseconds(80)
                };
                _volumeThrottleTimer.Tick += (s, e) =>
                {
                    _volumeThrottleTimer.Stop();
                    if (IsConnected)
                    {
                        AirPlayNative.airplay_set_volume(_pendingVolume);
                    }
                };
            }
            catch (Exception ex)
            {
                StatusMessage = $"Motor başlatılamadı: {ex.Message}";
            }
        }

        private void QueueVolumeUpdate(float vol)
        {
            _pendingVolume = vol;
            if (_volumeThrottleTimer != null && !_volumeThrottleTimer.IsEnabled)
            {
                _volumeThrottleTimer.Start();
            }
        }

        private void OnDeviceDiscovered(string name, string ip, string model, ushort port)
        {
            _dispatcher.Invoke(() =>
            {
                var existing = Devices.FirstOrDefault(d => d.Name == name);
                if (existing != null)
                {
                    // Prefer IPv4 if existing address was IPv6
                    if (existing.Ip.Contains(':') && !ip.Contains(':'))
                    {
                        existing.Ip = ip;
                        existing.Port = port;
                        if (SelectedDevice == existing)
                        {
                            StatusMessage = $"'{name}' hazır ({ip}).";
                        }
                    }
                    return;
                }

                var dev = new HomePodDevice
                {
                    Name = name,
                    Ip = ip,
                    Port = port,
                    Model = model
                };
                Devices.Add(dev);

                if (SelectedDevice == null)
                {
                    SelectedDevice = dev;
                    StatusMessage = $"'{name}' hazır ({ip}).";
                }
            });
        }

        private void OnStatusUpdate(int statusCode, string message)
        {
            _dispatcher.Invoke(() =>
            {
                StatusMessage = message;
                if (statusCode == 200)
                {
                    IsConnected = true;
                    IsConnecting = false;
                    AirPlayNative.airplay_set_volume(_volume);
                }
                else if (statusCode < 0)
                {
                    IsConnected = false;
                    IsConnecting = false;
                }
            });
        }

        public void Connect()
        {
            if (SelectedDevice == null || IsConnected || IsConnecting)
                return;

            IsConnecting = true;
            StatusMessage = $"'{SelectedDevice.Name}' ile el sıkışılıyor ({SelectedDevice.Ip})...";

            int res = AirPlayNative.airplay_connect(
                SelectedDevice.Ip,
                SelectedDevice.Port,
                "3939",
                LatencyMode,
                _statusCallback
            );

            if (res != 0)
            {
                IsConnecting = false;
                StatusMessage = $"Bağlantı başlatılamadı (Hata: {res}). IP formatı kontrol ediliyor.";
            }
        }

        public void Disconnect()
        {
            if (!IsConnected)
                return;

            AirPlayNative.airplay_disconnect();
            IsConnected = false;
            IsConnecting = false;
            StatusMessage = "Bağlantı sonlandırıldı.";
        }

        public void Shutdown()
        {
            Disconnect();
            AirPlayNative.airplay_stop_discovery();
            _statsTimer?.Stop();
            _volumeThrottleTimer?.Stop();
        }

        protected void OnPropertyChanged([CallerMemberName] string? name = null)
        {
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));
        }
    }
}
