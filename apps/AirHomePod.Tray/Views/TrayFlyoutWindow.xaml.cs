using System;
using System.Windows;
using AirHomePod.Tray.ViewModels;

namespace AirHomePod.Tray.Views
{
    public partial class TrayFlyoutWindow : Window
    {
        public MainTrayViewModel ViewModel => (MainTrayViewModel)DataContext;

        public TrayFlyoutWindow(MainTrayViewModel viewModel)
        {
            InitializeComponent();
            DataContext = viewModel;
        }

        public void PositionNearTray()
        {
            var workArea = SystemParameters.WorkArea;
            Left = workArea.Right - Width - 12;
            Top = workArea.Bottom - Height - 12;
        }

        public void ToggleVisibility()
        {
            if (IsVisible)
            {
                Hide();
            }
            else
            {
                PositionNearTray();
                Show();
                Activate();
            }
        }

        private void Window_Deactivated(object sender, EventArgs e)
        {
            // Kullanıcı pencere dışına tıkladığında arayüzü gizle
            Hide();
        }

        private void ConnectButton_Click(object sender, RoutedEventArgs e)
        {
            ViewModel.Connect();
        }

        private void DisconnectButton_Click(object sender, RoutedEventArgs e)
        {
            ViewModel.Disconnect();
        }

        private void LatencyUltraLow_Checked(object sender, RoutedEventArgs e)
        {
            if (ViewModel != null) ViewModel.LatencyMode = 0;
        }

        private void LatencyBalanced_Checked(object sender, RoutedEventArgs e)
        {
            if (ViewModel != null) ViewModel.LatencyMode = 1;
        }

        private void LatencyStable_Checked(object sender, RoutedEventArgs e)
        {
            if (ViewModel != null) ViewModel.LatencyMode = 2;
        }

        private void ExitButton_Click(object sender, RoutedEventArgs e)
        {
            ViewModel.Shutdown();
            Application.Current.Shutdown();
        }
    }
}
