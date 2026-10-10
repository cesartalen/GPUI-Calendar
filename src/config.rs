use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize)]
pub struct Config {
    pub url: String,
    pub username: String,
    pub password: String,
}

pub fn path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_default()
        .join("gpuical")
        .join("config.toml")
}

pub fn load() -> Result<Config, String> {
    let path = path();
    let text = std::fs::read_to_string(&path)
        .map_err(|_| format!("Create {} with url, username, password", path.display()))?;
    toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}
