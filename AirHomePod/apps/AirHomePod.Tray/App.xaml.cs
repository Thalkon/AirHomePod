using System;
using System.Drawing;
using System.Windows;
using System.Windows.Forms;
using AirHomePod.Tray.ViewModels;
using AirHomePod.Tray.Views;
using Application = System.Windows.Application;

namespace AirHomePod.Tray
{
    public partial class App : Application
    {
        private NotifyIcon? _notifyIcon;
        private MainTrayViewModel? _viewModel;
        private TrayFlyoutWindow? _flyoutWindow;

        protected override void OnStartup(StartupEventArgs e)
        {
            base.OnStartup(e);

            _viewModel = new MainTrayViewModel();
            _flyoutWindow = new TrayFlyoutWindow(_viewModel);

            InitializeTrayIcon();
        }

        private void InitializeTrayIcon()
        {
            _notifyIcon = new NotifyIcon
            {
                Text = "AirHomePod - HomePod mini AirPlay 2",
                Icon = SystemIcons.Application, // Sistem varsayılan ikonu veya özel .ico
                Visible = true
            };

            // Sol tık: Flyout penceresini aç/kapa
            _notifyIcon.MouseClick += (s, e) =>
            {
                if (e.Button == MouseButtons.Left)
                {
                    _flyoutWindow?.ToggleVisibility();
                }
            };

            // Sağ tık: Hızlı içerik menüsü
            var contextMenu = new ContextMenuStrip();
            contextMenu.Items.Add("AirHomePod'u Aç", null, (s, e) => _flyoutWindow?.ToggleVisibility());
            contextMenu.Items.Add(new ToolStripSeparator());
            contextMenu.Items.Add("Bağlantıyı Kes", null, (s, e) => _viewModel?.Disconnect());
            contextMenu.Items.Add("Çıkış", null, (s, e) => ExitApplication());

            _notifyIcon.ContextMenuStrip = contextMenu;
        }

        private void ExitApplication()
        {
            _viewModel?.Shutdown();
            if (_notifyIcon != null)
            {
                _notifyIcon.Visible = false;
                _notifyIcon.Dispose();
            }
            Shutdown();
        }

        protected override void OnExit(ExitEventArgs e)
        {
            if (_notifyIcon != null)
            {
                _notifyIcon.Visible = false;
                _notifyIcon.Dispose();
            }
            base.OnExit(e);
        }
    }
}
