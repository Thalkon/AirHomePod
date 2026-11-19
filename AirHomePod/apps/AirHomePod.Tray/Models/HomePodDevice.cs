namespace AirHomePod.Tray.Models
{
    public class HomePodDevice
    {
        public string Name { get; set; } = string.Empty;
        public string Ip { get; set; } = string.Empty;
        public ushort Port { get; set; }
        public string Model { get; set; } = string.Empty;

        public string DisplayText => $"{Name} ({Ip})";
        public string Subtitle => string.IsNullOrEmpty(Model) || Model == "Unknown" 
            ? "HomePod mini" 
            : Model;

        public override string ToString() => DisplayText;
    }
}
