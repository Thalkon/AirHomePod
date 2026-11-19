using System;
using System.Runtime.InteropServices;

namespace AirHomePod.Tray.Native
{
    public static class AirPlayNative
    {
        private const string DllName = "airplay_core.dll";

        [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
        public delegate void DeviceDiscoveredCallback(
            [MarshalAs(UnmanagedType.LPUTF8Str)] string name,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string ip,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string model,
            ushort port
        );

        [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
        public delegate void StatusCallback(
            int statusCode,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string message
        );

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int airplay_init();

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int airplay_start_discovery(DeviceDiscoveredCallback callback);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int airplay_stop_discovery();

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int airplay_connect(
            [MarshalAs(UnmanagedType.LPUTF8Str)] string ip,
            ushort port,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string pin,
            int latencyMode,
            StatusCallback callback
        );

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int airplay_disconnect();

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int airplay_set_volume(float volumePercent);

        [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int airplay_get_stats(
            out uint outPacketsSent,
            out uint outRetransmits,
            out int outIsStreaming
        );
    }
}
