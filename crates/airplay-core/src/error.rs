use thiserror::Error;

#[derive(Error, Debug)]
pub enum AirPlayError {
    #[error("G/Ç hatası: {0}")]
    Io(#[from] std::io::Error),

    #[error("mDNS keşif hatası: {0}")]
    Mdns(String),

    #[error("HomeKit eşleşme hatası: {0}")]
    Pairing(String),

    #[error("RTSP hatası: {0}")]
    Rtsp(String),

    #[error("Şifreleme hatası: {0}")]
    Crypto(String),

    #[error("Ses yakalama hatası: {0}")]
    AudioCapture(String),

    #[error("Plist hatası: {0}")]
    Plist(String),

    #[error("{0}")]
    Other(String),
}

impl From<plist::Error> for AirPlayError {
    fn from(e: plist::Error) -> Self {
        AirPlayError::Plist(e.to_string())
    }
}

#[cfg(windows)]
impl From<windows::core::Error> for AirPlayError {
    fn from(e: windows::core::Error) -> Self {
        AirPlayError::AudioCapture(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, AirPlayError>;
