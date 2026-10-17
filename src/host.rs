use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Deserialize, Serialize, Clone)]
pub struct Defaults {
    pub username: String,
    pub port: u16,
    pub dns_server: Option<String>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Config {
    pub defaults: Defaults,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Host {
    pub ip: String,
    pub hostname: Option<String>,
    pub username: Option<String>,
    pub port: Option<u16>,
    pub tag: Option<String>,
    pub active: bool,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Network {
    pub name: String,
    pub network: String,
    pub username: Option<String>,
    pub port: Option<u16>,
    pub hosts: Vec<Host>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct NetworksFile {
    pub networks: Vec<Network>,
}

pub fn load_config(path: &str) -> Result<Config, String> {
    let content = match fs::read_to_string(path) {
        Ok(text) => text,
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
