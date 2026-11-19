use std::fs::OpenOptions;
use std::path::PathBuf;
use std::sync::Once;

static INIT: Once = Once::new();
const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;

/// Log dosyasının yolu: %LOCALAPPDATA%\AirHomePod\airplay-core.log
pub fn log_path() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("AirHomePod")
        .join("airplay-core.log")
}

/// Tepsi uygulaması için dosyaya loglama (sorun bildirirken bu dosya paylaşılabilir).
pub fn init_file_logger() {
    INIT.call_once(|| {
        let path = log_path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let too_big = std::fs::metadata(&path)
            .map(|m| m.len() > MAX_LOG_BYTES)
            .unwrap_or(false);

        let mut builder = env_logger::Builder::new();
        builder.filter_level(log::LevelFilter::Info);

        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .append(!too_big)
            .truncate(too_big)
            .open(&path);
        if let Ok(f) = file {
            builder.target(env_logger::Target::Pipe(Box::new(f)));
        }
        let _ = builder.try_init();
    });
}

/// CLI için konsola loglama (RUST_LOG ile seviye değiştirilebilir).
pub fn init_console_logger() {
    INIT.call_once(|| {
        let _ = env_logger::Builder::new()
            .filter_level(log::LevelFilter::Info)
            .parse_default_env()
            .try_init();
    });
}
