use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Deserialize, Serialize, Clone)]
pub struct Defaults {
    pub username: String,
    pub port: u16,
    pub dns_server: Option<String>,
    pub identity_file: Option<String>,
    pub terminal: Option<String>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Host {
    pub ip: String,
    pub hostname: Option<String>,
    pub username: Option<String>,
    pub port: Option<u16>,
    pub tag: Option<String>,
    pub identity_file: Option<String>,
    pub notes: Option<String>,
    pub health_status: Option<String>,
    pub health_report: Option<String>,
    pub post_connect: Option<String>,
    pub active: bool,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Network {
    pub name: String,
    pub network: String,
    pub username: Option<String>,
    pub port: Option<u16>,
    pub identity_file: Option<String>,
    pub scan_timeout: Option<u64>,
    pub hosts: Vec<Host>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct NetworksFile {
    pub networks: Vec<Network>,
}

// Used on a first run, when config.toml does not exist yet. Nothing is written until the user saves.
pub fn default_config() -> Config {
    Config {
        defaults: Defaults {
            username: String::from("root"),
            port: 22,
            dns_server: None,
            identity_file: None,
            terminal: None,
        },
        ai: None,
    }
}

pub fn load_config(path: &str) -> Result<Config, String> {
    let content = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(default_config()),
        Err(e) => return Err(format!("Could not read {}: {}", path, e)),
    };
    match toml::from_str(&content) {
        Ok(c) => Ok(c),
        Err(e) => Err(format!("Could not parse {}: {}", path, e)),
    }
}

pub fn load_networks(path: &str) -> Result<NetworksFile, String> {
    let content = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(NetworksFile { networks: vec![] }),
        Err(e) => return Err(format!("Could not read {}: {}", path, e)),
    };
    match toml::from_str(&content) {
        Ok(n) => Ok(n),
        Err(e) => Err(format!("Could not parse {}: {}", path, e)),
    }
}

pub fn save_networks(path: &str, networks_file: &NetworksFile) -> Result<(), String> {
    let content = match toml::to_string(networks_file) {
        Ok(c) => c,
        Err(e) => return Err(format!("Could not serialize networks: {}", e)),
    };
    match fs::write(path, content) {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("Could not write {}: {}", path, e)),
    }
}

pub fn resolve_username(host: &Host, network: &Network, defaults: &Defaults) -> String {
    host.username.clone()
        .or(network.username.clone())
        .unwrap_or(defaults.username.clone())
}

pub fn resolve_port(host: &Host, network: &Network, defaults: &Defaults) -> u16 {
    host.port
        .or(network.port)
        .unwrap_or(defaults.port)
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ConnectionRecord {
    pub ip: String,
    pub hostname: Option<String>,
    pub username: String,
    pub port: u16,
    pub network_name: String,
    pub connected_at: String,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct History {
    pub connections: Vec<ConnectionRecord>,
}

pub fn load_history(path: &str) -> History {
    let content = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(_) => return History { connections: vec![] },
    };
    toml::from_str(&content).unwrap_or(History { connections: vec![] })
}

pub fn save_history(path: &str, history: &History) -> Result<(), String> {
    // máximo 50 entradas
    let mut h = history.clone();
    if h.connections.len() > 50 {
        h.connections = h.connections.into_iter().rev().take(50).rev().collect();
    }
    let content = toml::to_string(&h)
        .map_err(|e| format!("Could not serialize history: {}", e))?;
    std::fs::write(path, content)
        .map_err(|e| format!("Could not write history: {}", e))
}

pub fn _resolve_identity_file(host: &Host, network: &Network, defaults: &Defaults) -> Option<String> {
    host.identity_file.clone()
        .or(network.identity_file.clone())
        .or(defaults.identity_file.clone())
}

#[derive(Deserialize, Serialize, Clone)]
pub struct AiConfig {
    pub provider: Option<String>,
    pub api_url: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<String>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Config {
    pub defaults: Defaults,
    pub ai: Option<AiConfig>,
}
