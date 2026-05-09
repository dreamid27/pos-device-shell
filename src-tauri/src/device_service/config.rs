use std::path::PathBuf;
use std::sync::RwLock;

use once_cell::sync::Lazy;

use super::types::Config;

static CONFIG_PATH: Lazy<RwLock<Option<PathBuf>>> = Lazy::new(|| RwLock::new(None));

pub fn set_config_path(path: PathBuf) {
    if let Ok(mut guard) = CONFIG_PATH.write() {
        *guard = Some(path);
    }
}

fn resolve_path() -> PathBuf {
    if let Ok(guard) = CONFIG_PATH.read() {
        if let Some(path) = guard.as_ref() {
            return path.clone();
        }
    }
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("config.json")
}

pub fn load_config() -> Config {
    let path = resolve_path();
    if !path.exists() {
        log::info!("[device-service] no config.json at {path:?}, using defaults");
        return Config::default();
    }

    match std::fs::read_to_string(&path) {
        Ok(raw) => match serde_json::from_str::<Config>(&raw) {
            Ok(cfg) => cfg,
            Err(e) => {
                log::error!("[device-service] failed to parse config: {e}");
                Config::default()
            }
        },
        Err(e) => {
            log::error!("[device-service] failed to read config: {e}");
            Config::default()
        }
    }
}
