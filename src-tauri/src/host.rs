use serde::{Deserialize, Serialize};
use std::fs;

/// How the tmux session of a Broadcast is set up. Every field defaults to true, so a config.toml written
/// before these settings existed keeps working (and gets them).
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct BroadcastSettings {
    /// Each pane carries the name of its host on its border.
    pub label_panes: bool,
    /// A pane whose ssh fails stays open, with its output, instead of disappearing.
    pub keep_failed: bool,
    /// The panes start synchronized (what a Broadcast is for).
    pub start_synced: bool,
}

impl Default for BroadcastSettings {
    fn default() -> Self {
        BroadcastSettings { label_panes: true, keep_failed: true, start_synced: true }
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Defaults {
    pub username: String,
    pub port: u16,
    pub dns_server: Option<String>,
    pub identity_file: Option<String>,
    pub terminal: Option<String>,
    // A table: it has to come after the plain values for the TOML to be written.
    #[serde(default)]
    pub broadcast: BroadcastSettings,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Host {
    pub ip: String,
    pub hostname: Option<String>,
    pub ssh_banner: Option<String>,
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
            broadcast: BroadcastSettings::default(),
        },
        ai: None,
        monitoring: None,
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

/// Most hosts kept in the history. It is only used to show "Last connection", so what matters is the
/// most recent connection to each host, not how many connections there were.
const MAX_HISTORY_HOSTS: usize = 1000;

pub fn save_history(path: &str, history: &History) -> Result<(), String> {
    // Only the most recent connection of each host (network + ip) is kept, up to MAX_HISTORY_HOSTS.
    // Keeping the last 50 ENTRIES instead made a broadcast to 20 hosts push 20 other hosts out.
    let mut seen = std::collections::HashSet::new();
    let mut kept: Vec<ConnectionRecord> = Vec::new();
    for r in history.connections.iter().rev() {
        if kept.len() >= MAX_HISTORY_HOSTS {
            break;
        }
        if seen.insert((r.network_name.clone(), r.ip.clone())) {
            kept.push(r.clone());
        }
    }
    kept.reverse();
    let h = History { connections: kept };
    let content = toml::to_string(&h)
        .map_err(|e| format!("Could not serialize history: {}", e))?;
    std::fs::write(path, content)
        .map_err(|e| format!("Could not write history: {}", e))
}

/// Adds connection records in ONE load/save of the history (a broadcast records one per host).
pub fn record_connections(path: &str, records: Vec<ConnectionRecord>) -> Result<(), String> {
    let mut history = load_history(path);
    history.connections.extend(records);
    save_history(path, &history)
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
pub struct MonitoringConfig {
    pub check_interval_minutes: Option<u32>,
    pub history_days: Option<u32>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Config {
    pub defaults: Defaults,
    pub ai: Option<AiConfig>,
    pub monitoring: Option<MonitoringConfig>,
}
