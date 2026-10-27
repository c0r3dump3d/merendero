mod host;
mod scanner;

use std::path::PathBuf;
use host::{NetworksFile, Config, Defaults};
use tauri::Emitter;

fn get_config_dir() -> PathBuf {
    let mut path = dirs::home_dir().expect("Cannot find home directory");
    path.push(".config");
    path.push("merendero");
    // Create it on first use. It holds SSH settings, API keys and logs, so owner only.
    {
        use std::os::unix::fs::DirBuilderExt;
        let _ = std::fs::DirBuilder::new().recursive(true).mode(0o700).create(&path);
    }
    path
}

#[tauri::command]
fn get_networks() -> Result<NetworksFile, String> {
    let path = get_config_dir().join("networks.toml");
    host::load_networks(path.to_str().unwrap())
}

#[tauri::command]
fn get_config() -> Result<Config, String> {
    let path = get_config_dir().join("config.toml");
    host::load_config(path.to_str().unwrap())
}

#[tauri::command]
fn save_networks(networks_file: NetworksFile) -> Result<(), String> {
    let path = get_config_dir().join("networks.toml");
    host::save_networks(path.to_str().unwrap(), &networks_file)
}

#[tauri::command]
fn add_network(mut networks_file: NetworksFile, network: host::Network) -> Result<NetworksFile, String> {
    networks_file.networks.push(network);
    let path = get_config_dir().join("networks.toml");
    host::save_networks(path.to_str().unwrap(), &networks_file)?;
    Ok(networks_file)
}

#[tauri::command]
fn update_network(mut networks_file: NetworksFile, index: usize, network: host::Network) -> Result<NetworksFile, String> {
    if index >= networks_file.networks.len() {
        return Err(String::from("Invalid network index"));
    }
    let hosts = networks_file.networks[index].hosts.clone();
    networks_file.networks[index] = network;
    networks_file.networks[index].hosts = hosts;
    let path = get_config_dir().join("networks.toml");
    host::save_networks(path.to_str().unwrap(), &networks_file)?;
    Ok(networks_file)
}

#[tauri::command]
fn delete_network(mut networks_file: NetworksFile, index: usize) -> Result<NetworksFile, String> {
    if index >= networks_file.networks.len() {
        return Err(String::from("Invalid network index"));
    }
    networks_file.networks.remove(index);
    let path = get_config_dir().join("networks.toml");
    host::save_networks(path.to_str().unwrap(), &networks_file)?;
    Ok(networks_file)
}

#[tauri::command]
fn add_host(mut networks_file: NetworksFile, network_index: usize, host: host::Host) -> Result<NetworksFile, String> {
    if network_index >= networks_file.networks.len() {
        return Err(String::from("Invalid network index"));
    }
    networks_file.networks[network_index].hosts.push(host);
    let path = get_config_dir().join("networks.toml");
    host::save_networks(path.to_str().unwrap(), &networks_file)?;
    Ok(networks_file)
}

#[tauri::command]
fn update_host(mut networks_file: NetworksFile, network_index: usize, host_index: usize, host: host::Host) -> Result<NetworksFile, String> {
    if network_index >= networks_file.networks.len() {
        return Err(String::from("Invalid network index"));
    }
    if host_index >= networks_file.networks[network_index].hosts.len() {
        return Err(String::from("Invalid host index"));
    }
    networks_file.networks[network_index].hosts[host_index] = host;
    let path = get_config_dir().join("networks.toml");
    host::save_networks(path.to_str().unwrap(), &networks_file)?;
    Ok(networks_file)
}

#[tauri::command]
fn delete_host(mut networks_file: NetworksFile, network_index: usize, host_index: usize) -> Result<NetworksFile, String> {
    if network_index >= networks_file.networks.len() {
        return Err(String::from("Invalid network index"));
    }
    if host_index >= networks_file.networks[network_index].hosts.len() {
        return Err(String::from("Invalid host index"));
    }
    networks_file.networks[network_index].hosts.remove(host_index);
    let path = get_config_dir().join("networks.toml");
    host::save_networks(path.to_str().unwrap(), &networks_file)?;
    Ok(networks_file)
}

#[tauri::command]
fn bulk_set_username(mut networks_file: NetworksFile, network_index: usize, host_ips: Vec<String>, username: String) -> Result<NetworksFile, String> {
    if network_index >= networks_file.networks.len() {
        return Err(String::from("Invalid network index"));
    }
    // Trimmed, unlike bulk_tag: a username with stray leading/trailing spaces breaks the SSH login,
    // while a tag with spaces is only cosmetic.
    let username_opt = if username.trim().is_empty() { None } else { Some(username.trim().to_string()) };
    for host in networks_file.networks[network_index].hosts.iter_mut() {
        if host_ips.contains(&host.ip) {
            host.username = username_opt.clone();
        }
    }
    let path = get_config_dir().join("networks.toml");
    host::save_networks(path.to_str().unwrap(), &networks_file)?;
    Ok(networks_file)
}

#[tauri::command]
fn bulk_tag(mut networks_file: NetworksFile, network_index: usize, host_ips: Vec<String>, tag: String) -> Result<NetworksFile, String> {
    if network_index >= networks_file.networks.len() {
        return Err(String::from("Invalid network index"));
    }
    let tag_opt = if tag.is_empty() { None } else { Some(tag) };
    for host in networks_file.networks[network_index].hosts.iter_mut() {
        if host_ips.contains(&host.ip) {
            host.tag = tag_opt.clone();
        }
    }
    let path = get_config_dir().join("networks.toml");
    host::save_networks(path.to_str().unwrap(), &networks_file)?;
    Ok(networks_file)
}

#[tauri::command]
fn save_config(config: host::Config) -> Result<(), String> {
    let path = get_config_dir().join("config.toml");
    let content = toml::to_string(&config)
        .map_err(|e| format!("Could not serialize config: {}", e))?;
    write_private(&path, &content)
}

#[tauri::command]
fn get_history() -> host::History {
    let path = get_config_dir().join("history.toml");
    host::load_history(path.to_str().unwrap())
}

// ---------------------------------------------------------------------------------------
// Programs and terminals
// ---------------------------------------------------------------------------------------

fn is_executable_file(p: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.metadata().map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0).unwrap_or(false)
}

// Directories where programs are looked for: the PATH this process inherited (the user's PATH)
// first, then well-known directories that a desktop session often leaves out.
fn user_path() -> String {
    let mut dirs: Vec<String> = std::env::var("PATH").unwrap_or_default()
        .split(':').filter(|d| !d.is_empty()).map(String::from).collect();
    let mut extra: Vec<String> = ["/usr/local/sbin", "/usr/local/bin", "/usr/sbin", "/usr/bin", "/sbin", "/bin", "/run/current-system/sw/bin", "/snap/bin"]
        .iter().map(|s| s.to_string()).collect();
    if let Some(home) = dirs::home_dir() {
        for rel in [".local/bin", ".cargo/bin", ".nix-profile/bin", ".guix-profile/bin"] {
            extra.push(home.join(rel).to_string_lossy().into_owned());
        }
    }
    for d in extra {
        if !dirs.contains(&d) { dirs.push(d); }
    }
    dirs.join(":")
}

// Finds a program by name in user_path(). A name with a '/' (or starting with "~/") is used as given.
fn find_program(name: &str) -> Option<std::path::PathBuf> {
    let name = name.trim();
    if name.is_empty() { return None; }
    if name.contains('/') || name.starts_with('~') {
        let p = match name.strip_prefix("~/") {
            Some(rest) => dirs::home_dir()?.join(rest),
            None => std::path::PathBuf::from(name),
        };
        return if is_executable_file(&p) { Some(p) } else { None };
    }
    user_path().split(':').map(|d| std::path::Path::new(d).join(name)).find(|p| is_executable_file(p))
}

// Path to use when spawning a program. If it cannot be found, the bare name is returned so that
// the error that follows names the missing program.
fn program(name: &str) -> String {
    find_program(name).map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|| name.to_string())
}

// Quotes a string for a POSIX shell. Strings made only of safe characters stay as they are.
fn sh_quote(s: &str) -> String {
    if !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || "_-./:@%+=,".contains(c)) {
        return s.to_string();
    }
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[derive(Clone, Copy)]
enum LaunchStyle {
    ETextString, // terminal -e "<command as one string>"
    EShC,        // terminal -e sh -c <command>
    ShC,         // terminal sh -c <command>
    DashDashShC, // terminal -- sh -c <command>
    XShC,        // terminal -x sh -c <command>
}

struct TerminalRecipe {
    id: &'static str,
    style: LaunchStyle,
    maximize: &'static [&'static str],
}

// How to start each known terminal with a command. Every form below was checked against the real
// terminal (the program receives the command as a single argument, quotes included), and so was
// every maximize option (the window ends up covering the screen).
const TERMINALS: &[TerminalRecipe] = &[
    TerminalRecipe { id: "terminator", style: LaunchStyle::ETextString, maximize: &["--maximise"] },
    TerminalRecipe { id: "alacritty", style: LaunchStyle::EShC, maximize: &["-o", "window.startup_mode=\"Maximized\""] },
    TerminalRecipe { id: "kitty", style: LaunchStyle::ShC, maximize: &["--start-as=maximized"] },
    TerminalRecipe { id: "foot", style: LaunchStyle::ShC, maximize: &[] },
    TerminalRecipe { id: "gnome-terminal", style: LaunchStyle::DashDashShC, maximize: &["--maximize"] },
    TerminalRecipe { id: "mate-terminal", style: LaunchStyle::DashDashShC, maximize: &["--maximize"] },
    TerminalRecipe { id: "xfce4-terminal", style: LaunchStyle::XShC, maximize: &["--maximize"] },
    TerminalRecipe { id: "tilix", style: LaunchStyle::EShC, maximize: &["--maximize"] },
    TerminalRecipe { id: "konsole", style: LaunchStyle::EShC, maximize: &[] },
    TerminalRecipe { id: "qterminal", style: LaunchStyle::EShC, maximize: &[] },
    TerminalRecipe { id: "xterm", style: LaunchStyle::EShC, maximize: &["-maximized"] },
    TerminalRecipe { id: "lxterminal", style: LaunchStyle::EShC, maximize: &[] },
    TerminalRecipe { id: "urxvt", style: LaunchStyle::EShC, maximize: &[] },
    TerminalRecipe { id: "sakura", style: LaunchStyle::ETextString, maximize: &[] },
];

// Builds the command that opens `terminal` (a name or a path; the automatic choice if empty or "auto") running
// `shell_cmd`. A terminal that is not in the table is started with -e "<command>".
fn terminal_command(terminal: Option<&str>, shell_cmd: &str, maximize: bool) -> Result<std::process::Command, String> {
    let name_owned = resolve_terminal_name(terminal, auto_terminal)?;
    let name = name_owned.as_str();
    let base = std::path::Path::new(name).file_name().and_then(|n| n.to_str()).unwrap_or(name);
    let recipe = TERMINALS.iter().find(|r| r.id == base);
    let bin = find_program(name).ok_or_else(|| format!("Terminal '{}' not found. Choose another one in Settings", name))?;
    let mut cmd = std::process::Command::new(bin);
    cmd.env("PATH", user_path());
    if maximize {
        if let Some(r) = recipe { cmd.args(r.maximize); }
    }
    match recipe.map(|r| r.style).unwrap_or(LaunchStyle::ETextString) {
        LaunchStyle::ETextString => { cmd.args(["-e", shell_cmd]); }
        LaunchStyle::EShC => { cmd.args(["-e", "sh", "-c", shell_cmd]); }
        LaunchStyle::ShC => { cmd.args(["sh", "-c", shell_cmd]); }
        LaunchStyle::DashDashShC => { cmd.args(["--", "sh", "-c", shell_cmd]); }
        LaunchStyle::XShC => { cmd.args(["-x", "sh", "-c", shell_cmd]); }
    }
    Ok(cmd)
}

// ---------------------------------------------------------------------------------------
// Terminal choice
// ---------------------------------------------------------------------------------------

// The terminals to try, in order, when none is chosen (or "auto" is). terminator comes first
// because it was always the default; then the one that belongs to the user's desktop; then the rest.
fn auto_order_for(desktop: &str) -> Vec<&'static str> {
    let mut order: Vec<&'static str> = vec!["terminator"];
    for part in desktop.to_lowercase().split(':') {
        let own = match part.trim() {
            "gnome" | "x-cinnamon" | "budgie" | "unity" => Some("gnome-terminal"),
            "kde" => Some("konsole"),
            "xfce" => Some("xfce4-terminal"),
            "mate" => Some("mate-terminal"),
            "lxqt" => Some("qterminal"),
            "lxde" => Some("lxterminal"),
            _ => None,
        };
        if let Some(t) = own {
            if !order.contains(&t) { order.push(t); }
        }
    }
    for t in ["tilix", "alacritty", "kitty", "gnome-terminal", "konsole", "xfce4-terminal", "mate-terminal", "lxterminal", "qterminal", "xterm", "urxvt", "sakura", "foot"] {
        if !order.contains(&t) { order.push(t); }
    }
    order
}

fn pick_auto(desktop: &str, installed: impl Fn(&str) -> bool) -> Option<String> {
    auto_order_for(desktop).into_iter().find(|t| installed(t)).map(String::from)
}

// What "automatic" resolves to on this system right now.
fn auto_terminal() -> Option<String> {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    pick_auto(&desktop, |t| find_program(t).is_some())
}

// The terminal to use: the one chosen, or the automatic choice when none is chosen (or it is "auto").
fn resolve_terminal_name(chosen: Option<&str>, auto: impl FnOnce() -> Option<String>) -> Result<String, String> {
    match chosen.map(str::trim).filter(|t| !t.is_empty() && *t != "auto") {
        Some(name) => Ok(name.to_string()),
        None => auto().ok_or_else(|| String::from("No supported terminal emulator found. Install one (for example xterm or gnome-terminal) or set another in Settings")),
    }
}

#[derive(serde::Serialize)]
struct TerminalInfo {
    id: String,
    path: String,
    maximize: bool,
}

// The known terminals that are installed, for the picker in Settings.
#[tauri::command]
fn list_terminals() -> Vec<TerminalInfo> {
    TERMINALS.iter().filter_map(|r| {
        find_program(r.id).map(|p| TerminalInfo { id: r.id.to_string(), path: p.to_string_lossy().into_owned(), maximize: !r.maximize.is_empty() })
    }).collect()
}

// What "automatic" resolves to, to show it in Settings.
#[tauri::command]
fn default_terminal() -> Option<String> {
    auto_terminal()
}

fn launch_ssh_inner(ip: String, username: String, port: u16, terminal: Option<String>, hostname: Option<String>, network_name: Option<String>, identity_file: Option<String>, post_connect: Option<String>) -> Result<(), String> {
    let ssh = sh_quote(&program("ssh"));
    let identity_arg = identity_file.as_deref().map(str::trim).filter(|f| !f.is_empty())
        .map(|f| format!(" -i {}", sh_quote(f))).unwrap_or_default();
    // '--' before the destination: without it, a username/ip starting with '-' (e.g. from an imported
    // backup) could be read by ssh/scp as an OPTION instead of the destination.
    let target = sh_quote(&format!("{}@{}", username, ip));
    let ssh_cmd = match post_connect.as_deref().filter(|c| !c.trim().is_empty()) {
        Some(cmd) => format!("{} -t -p {}{} -- {} {}", ssh, port, identity_arg, target, sh_quote(cmd)),
        None => format!("{} -p {}{} -- {}", ssh, port, identity_arg, target),
    };
    let result = terminal_command(terminal.as_deref(), &ssh_cmd, false)
        .and_then(|mut c| c.spawn().map_err(|e| format!("Failed to launch terminal: {}", e)));
    // guarda en historial
    if result.is_ok() {
        let path = get_config_dir().join("history.toml");
        let mut history = host::load_history(path.to_str().unwrap());
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        history.connections.push(host::ConnectionRecord {
            ip: ip.clone(),
            hostname,
            username: username.clone(),
            port,
            network_name: network_name.unwrap_or(String::from("unknown")),
            connected_at: now.to_string(),
        });
        let _ = host::save_history(path.to_str().unwrap(), &history);
    }
    result.map(|_| ())
}

async fn calibrate_timeout(network: &host::Network, defaults: &Defaults) -> u64 {
    use tokio::net::TcpStream;
    use std::time::{Duration, Instant};
    use ipnetwork::IpNetwork;
    use std::str::FromStr;

    let port = network.port.unwrap_or(defaults.port);
    let cidr = match IpNetwork::from_str(&network.network) {
        Ok(c) => c,
        Err(_) => return 3,
    };

    // toma hasta 10 IPs aleatorias del rango
    let ips: Vec<String> = cidr.iter()
        .skip(1)
        .take(50)
        .step_by(5)
        .map(|ip| ip.to_string())
        .collect();

    let mut rtts = vec![];

    for ip in &ips {
        let address = format!("{}:{}", ip, port);
        let start = Instant::now();
        if let Ok(Ok(_)) = tokio::time::timeout(
            Duration::from_secs(5),
            TcpStream::connect(&address)
        ).await {
            rtts.push(start.elapsed().as_millis());
        }
        if rtts.len() >= 5 { break; }
    }

    if rtts.is_empty() {
        return 3; // default si no hay respuestas
    }

    let avg_ms = rtts.iter().sum::<u128>() / rtts.len() as u128;

    // timeout = RTT medio × 3 + 500ms margen, mínimo 1s, máximo 10s
    let timeout_ms = (avg_ms * 3) + 500;
    let timeout_secs = (timeout_ms / 1000).max(2).min(10);

    timeout_secs as u64
}

fn validate_cidr(value: &str) -> Result<(), String> {
    if value != value.trim() {
        return Err(format!("Invalid network '{}': remove leading/trailing spaces", value));
    }
    if !value.contains('/') {
        return Err(format!(
            "Invalid network '{}': missing prefix length, it would be scanned as a single host. Use for example {}/24",
            value, value
        ));
    }
    value
        .parse::<ipnetwork::IpNetwork>()
        .map(|_| ())
        .map_err(|e| format!("Invalid network '{}': {}", value, e))
}

// ===== large networks: begin =====
/// A scan of up to this many addresses (a /20) needs no warning.
const SCAN_CONFIRM_ABOVE: u64 = 4_096;
/// Above this many addresses (bigger than a /12) the scan is refused: it would run for hours.
const SCAN_REFUSE_ABOVE: u64 = 1_048_576;

#[derive(serde::Serialize, Debug, PartialEq)]
struct ScanSize {
    addresses: u64,
    /// Worst case: every address drops the connection and waits for the whole timeout.
    estimate_secs: u64,
    /// "ok", "confirm" or "refuse".
    level: &'static str,
}

fn scan_size_info(network: &str, timeout_secs: u64) -> Result<ScanSize, String> {
    let net: ipnetwork::IpNetwork = network
        .trim()
        .parse()
        .map_err(|e| format!("Invalid network '{}': {}", network, e))?;
    let addresses: u64 = match net {
        ipnetwork::IpNetwork::V4(n) => 1u64 << (32 - u32::from(n.prefix())),
        ipnetwork::IpNetwork::V6(_) => return Err(String::from("Only IPv4 networks can be scanned")),
    };
    let estimate_secs = addresses.div_ceil(scanner::MAX_CONCURRENT as u64) * timeout_secs;
    let level = if addresses > SCAN_REFUSE_ABOVE {
        "refuse"
    } else if addresses > SCAN_CONFIRM_ABOVE {
        "confirm"
    } else {
        "ok"
    };
    Ok(ScanSize { addresses, estimate_secs, level })
}

/// Puts the result of a scan onto the networks as they are on disk NOW. A big scan takes minutes and
/// the person may have edited hosts meanwhile, so the copy the window sent when the scan began is not used.
/// Only a scan that covered the whole range (`complete`) can say that a known host is gone.
fn apply_scan_results(
    current: &mut NetworksFile,
    name: &str,
    cidr: &str,
    discovered: Vec<host::Host>,
    complete: bool,
    calibrated: Option<u64>,
) -> Result<(), String> {
    let net = current
        .networks
        .iter_mut()
        .find(|n| n.name == name && n.network == cidr)
        .ok_or_else(|| String::from("The network was changed or removed while it was being scanned. Scan it again."))?;
    if net.scan_timeout.is_none() {
        net.scan_timeout = calibrated;
    }
    if discovered.is_empty() {
        return Ok(());
    }
    if complete {
        for h in net.hosts.iter_mut() {
            h.active = false;
        }
    }
    for new_host in discovered {
        match net.hosts.iter_mut().find(|h| h.ip == new_host.ip) {
            Some(existing) => {
                existing.active = true;
                if new_host.hostname.is_some() {
                    existing.hostname = new_host.hostname;
                }
            }
            None => net.hosts.push(new_host),
        }
    }
    Ok(())
}
// ===== large networks: end =====

#[derive(Default)]
struct ScanControl {
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

#[derive(serde::Serialize)]
struct ScanReply {
    networks: Vec<host::Network>,
    found: usize,
    scanned: usize,
    total: usize,
    cancelled: bool,
    dns_unresponsive: bool,
}

#[tauri::command]
fn cancel_scan(control: tauri::State<'_, ScanControl>) {
    control.cancel.store(true, std::sync::atomic::Ordering::SeqCst);
}

#[tauri::command]
fn network_scan_info(network: String, timeout: Option<u64>) -> Result<ScanSize, String> {
    scan_size_info(&network, timeout.unwrap_or(3))
}

#[tauri::command]
async fn scan_network(
    window: tauri::Window,
    networks_file: NetworksFile,
    network_index: usize,
    defaults: Defaults,
    control: tauri::State<'_, ScanControl>,
) -> Result<ScanReply, String> {
    if network_index >= networks_file.networks.len() {
        return Err(String::from("Invalid network index"));
    }
    let target = networks_file.networks[network_index].clone();

    validate_cidr(&target.network)?;

    // Safety net: the window asks for confirmation, but a hand-edited or imported file can bring any network.
    let size = scan_size_info(&target.network, target.scan_timeout.unwrap_or(3))?;
    if size.level == "refuse" {
        return Err(format!(
            "{} has {} addresses, too many to scan in one go. Add the subnets you actually use instead.",
            target.network, size.addresses
        ));
    }

    control.cancel.store(false, std::sync::atomic::Ordering::SeqCst);

    // calibrates if the network has no timeout yet
    let mut calibrated = None;
    let timeout = match target.scan_timeout {
        Some(t) => t,
        None => {
            let _ = window.emit("scan-status", "Calibrating network latency...");
            let t = calibrate_timeout(&target, &defaults).await;
            calibrated = Some(t);
            let _ = window.emit("scan-status", format!("Timeout calibrated: {}s", t));
            t
        }
    };

    let outcome = scanner::scan_network_with_progress(&target, &defaults, window, timeout, control.cancel.clone()).await;
    let found = outcome.hosts.len();

    let path = get_config_dir().join("networks.toml");
    let mut current = host::load_networks(path.to_str().unwrap())?;
    apply_scan_results(&mut current, &target.name, &target.network, outcome.hosts, !outcome.cancelled, calibrated)?;
    host::save_networks(path.to_str().unwrap(), &current)?;

    Ok(ScanReply {
        networks: current.networks,
        found,
        scanned: outcome.scanned,
        total: outcome.total,
        cancelled: outcome.cancelled,
        dns_unresponsive: outcome.dns_unresponsive,
    })
}

#[tauri::command]
async fn recalibrate_network(
    mut networks_file: NetworksFile,
    network_index: usize,
    defaults: Defaults,
) -> Result<NetworksFile, String> {
    if network_index >= networks_file.networks.len() {
        return Err(String::from("Invalid network index"));
    }
    validate_cidr(&networks_file.networks[network_index].network)?;
    let t = calibrate_timeout(&networks_file.networks[network_index], &defaults).await;
    networks_file.networks[network_index].scan_timeout = Some(t);
    let path = get_config_dir().join("networks.toml");
    host::save_networks(path.to_str().unwrap(), &networks_file)?;
    Ok(networks_file)
}

enum CheckApply {
    Changed,
    Unchanged,
    KeptOffline,
}

// Applies the result of a check to the networks file as it is on disk right now, so that
// edits made while the check was running are not lost. In an automatic check, if every host
// of a network fails at once, this machine most likely lost connectivity (VPN down, no
// network): the last known state is kept instead of marking everything as down.
fn apply_check_results(
    current: &mut NetworksFile,
    name: &str,
    cidr: &str,
    results: &std::collections::HashMap<String, (bool, Option<String>)>,
    auto: bool,
) -> Result<CheckApply, String> {
    let idx = current.networks.iter()
        .position(|n| n.name == name && n.network == cidr)
        .ok_or_else(|| String::from("The network was edited while it was being checked. Run the check again."))?;
    let hosts = &mut current.networks[idx].hosts;
    if auto && !results.is_empty() && !results.values().any(|(up, _)| *up) && hosts.iter().any(|h| h.active) {
        return Ok(CheckApply::KeptOffline);
    }
    let mut changed = false;
    for host in hosts.iter_mut() {
        if let Some((status, banner)) = results.get(&host.ip) {
            if host.active != *status {
                host.active = *status;
                changed = true;
            }
            // A banner that could not be read this time (busy host, brief hiccup) does not erase one
            // already known: the field only moves forward when a real banner is actually read.
            if banner.is_some() && host.ssh_banner != *banner {
                host.ssh_banner = banner.clone();
                changed = true;
            }
        }
    }
    Ok(if changed { CheckApply::Changed } else { CheckApply::Unchanged })
}

#[tauri::command]
async fn check_hosts(
    window: tauri::Window,
    networks_file: NetworksFile,
    network_index: usize,
    defaults: Defaults,
    auto: Option<bool>,
) -> Result<NetworksFile, String> {
    use tokio::net::TcpStream;
    use std::time::Duration;

    if network_index >= networks_file.networks.len() {
        return Err(String::from("Invalid network index"));
    }

    let network = &networks_file.networks[network_index];
    let port = network.port.unwrap_or(defaults.port);
    let timeout_secs = network.scan_timeout.unwrap_or(3);
    let hosts_snapshot: Vec<(String, u16)> = network.hosts.iter()
        .map(|h| (h.ip.clone(), h.port.unwrap_or(port)))
        .collect();

    let total = hosts_snapshot.len();
    let _ = window.emit("check-progress", serde_json::json!({ "total": total, "current": 0 }));

    // At most MAX_CONCURRENT connections at a time: a big network has thousands of known hosts, and this
    // process only has so many file descriptors.
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(scanner::MAX_CONCURRENT));
    let mut tasks = vec![];
    for (ip, p) in &hosts_snapshot {
        let ip = ip.clone();
        let p = *p;
        let sem = semaphore.clone();
        tasks.push(tokio::spawn(async move {
            let _permit = sem.acquire().await.unwrap();
            let address = format!("{}:{}", ip, p);
            // None = could not tell (this process ran out of file descriptors): the host keeps its state.
            let (reachable, banner) = match tokio::time::timeout(Duration::from_secs(timeout_secs), TcpStream::connect(&address)).await {
                Ok(Ok(mut stream)) => {
                    // Reusing the same connection: no extra connect just to read the banner.
                    let banner = scanner::read_ssh_banner(&mut stream, 2).await;
                    (Some(true), banner)
                }
                Ok(Err(e)) if matches!(e.raw_os_error(), Some(23) | Some(24)) => (None, None),
                _ => (Some(false), None),
            };
            (ip, reachable, banner)
        }));
    }

    let mut current = 0;
    let mut results = std::collections::HashMap::new();
    for task in tasks {
        current += 1;
        if let Ok((ip, Some(status), banner)) = task.await {
            results.insert(ip, (status, banner));
        }
        let _ = window.emit("check-progress", serde_json::json!({ "total": total, "current": current }));
    }

    let path = get_config_dir().join("networks.toml");
    let mut current = host::load_networks(path.to_str().unwrap())?;
    let target = &networks_file.networks[network_index];
    match apply_check_results(&mut current, &target.name, &target.network, &results, auto.unwrap_or(false))? {
        CheckApply::Changed => host::save_networks(path.to_str().unwrap(), &current)?,
        CheckApply::Unchanged | CheckApply::KeptOffline => {}
    }
    Ok(current)
}

#[tauri::command]
async fn list_remote_dir(
    ip: String,
    username: String,
    port: u16,
    identity_file: Option<String>,
    path: String,
) -> Result<Vec<serde_json::Value>, String> {
    use tokio::process::Command;

    let identity_arg = identity_file
        .map(|f| vec!["-i".to_string(), f])
        .unwrap_or_default();

    let mut args = vec![
        "-p".to_string(), port.to_string(),
        "-o".to_string(), "StrictHostKeyChecking=no".to_string(),
        "-o".to_string(), "ConnectTimeout=5".to_string(),
        "-o".to_string(), "BatchMode=yes".to_string(),
    ];
    args.extend(identity_arg);
    args.push(format!("{}@{}", username, ip));
    // ls -la con formato parseable
    // The path goes through the remote shell: quote it, or a folder named  x$(command)  would run that command.
    // The * and .* stay outside the quotes so the shell still expands them.
    args.push(format!("for f in {}/* {}/.*; do [ -e \"$f\" ] || continue; n=$(basename \"$f\"); [ \"$n\" = \".\" ] || [ \"$n\" = \"..\" ] && continue; if [ -d \"$f\" ]; then echo \"d:$n\"; else echo \"f:$n\"; fi; done 2>/dev/null", sh_quote(&path), sh_quote(&path)));

    let output = Command::new(program("ssh")).env("PATH", user_path())
        .args(&args)
        .output()
        .await
        .map_err(|e| format!("SSH error: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let mut entries = vec![];

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        if let Some(name) = line.strip_prefix("d:") {
            entries.push(serde_json::json!({
                "name": name,
                "is_dir": true,
                "is_link": false,
                "perms": "",
                "size": "",
            }));
        } else if let Some(name) = line.strip_prefix("f:") {
            entries.push(serde_json::json!({
                "name": name,
                "is_dir": false,
                "is_link": false,
                "perms": "",
                "size": "",
            }));
        }
    }

    // ordena: directorios primero
    entries.sort_by(|a, b| {
        let a_dir = a["is_dir"].as_bool().unwrap_or(false);
        let b_dir = b["is_dir"].as_bool().unwrap_or(false);
        b_dir.cmp(&a_dir).then(
            a["name"].as_str().unwrap_or("").cmp(b["name"].as_str().unwrap_or(""))
        )
    });

    Ok(entries)
}

fn run_scp_inner(
    ip: String,
    username: String,
    port: u16,
    identity_file: Option<String>,
    local_path: String,
    remote_path: String,
    direction: String, // "upload" | "download"
) -> Result<String, String> {
    let identity_arg = identity_file
        .map(|f| vec!["-i".to_string(), f])
        .unwrap_or_default();

    let mut args = vec![
        "-P".to_string(), port.to_string(),
        "-o".to_string(), "StrictHostKeyChecking=no".to_string(),
        "-o".to_string(), "ConnectTimeout=5".to_string(),
    ];
    args.extend(identity_arg);

    let remote = format!("{}@{}:{}", username, ip, remote_path);

    // '--' before the destination: without it, a username/ip starting with '-' (e.g. from an imported
    // backup) could be read by ssh/scp as an OPTION instead of the destination.
    args.push(String::from("--"));
    if direction == "upload" {
        args.push(local_path);
        args.push(remote);
    } else {
        args.push(remote);
        args.push(local_path);
    }

    let output = std::process::Command::new(program("scp"))
        .env("PATH", user_path())
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to run scp: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

async fn run_scp_broadcast_inner(
    hosts: Vec<host::Host>,
    network: host::Network,
    defaults: host::Defaults,
    local_path: String,
    remote_path: String,
    direction: String,
) -> Vec<serde_json::Value> {
    use tokio::process::Command;

    let mut tasks = vec![];

    for host in hosts {
        let username = host::resolve_username(&host, &network, &defaults);
        let port = host::resolve_port(&host, &network, &defaults);
        let identity_arg = host.identity_file.clone()
            .or(network.identity_file.clone())
            .or(defaults.identity_file.clone())
            .map(|f| vec!["-i".to_string(), f])
            .unwrap_or_default();
        let ip = host.ip.clone();
        let hostname = host.hostname.clone();
        let local = local_path.clone();
        let remote = remote_path.clone();
        let dir = direction.clone();

        let task = tokio::spawn(async move {
            let mut args = vec![
                "-P".to_string(), port.to_string(),
                "-o".to_string(), "StrictHostKeyChecking=no".to_string(),
                "-o".to_string(), "ConnectTimeout=5".to_string(),
            ];
            args.extend(identity_arg);

            let remote_full = format!("{}@{}:{}", username, ip, remote);

            args.push(String::from("--"));
            if dir == "upload" {
                args.push(local);
                args.push(remote_full);
            } else {
                args.push(remote_full);
                args.push(local);
            }

            let output = Command::new(program("scp"))
                .env("PATH", user_path())
                .args(&args)
                .output()
                .await;

            match output {
                Ok(out) => serde_json::json!({
                    "ip": ip,
                    "hostname": hostname,
                    "success": out.status.success(),
                    "message": String::from_utf8_lossy(if out.status.success() { &out.stdout } else { &out.stderr }).to_string(),
                }),
                Err(e) => serde_json::json!({
                    "ip": ip,
                    "hostname": hostname,
                    "success": false,
                    "message": format!("Failed: {}", e),
                }),
            }
        });
        tasks.push(task);
    }

    let mut results = vec![];
    for task in tasks {
        if let Ok(r) = task.await { results.push(r); }
    }
    results
}

fn iso_utc(secs: u64) -> String {
    let days = (secs / 86400) as i64;
    let rem = secs % 86400;
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, mo, d, h, m, s)
}

fn audit_write(action: &str, network: &str, hosts: Vec<serde_json::Value>, detail: serde_json::Value) {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let user = std::env::var("USER").unwrap_or_else(|_| String::from("unknown"));
    let entry = serde_json::json!({
        "time": iso_utc(secs),
        "user": user,
        "action": action,
        "network": network,
        "hosts": hosts,
        "detail": detail,
    });
    let path = get_config_dir().join("audit.jsonl");
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(&path)
    {
        let _ = writeln!(file, "{}", entry);
    }
}

async fn ai_call(
    client: &reqwest::Client,
    provider: &str,
    api_url: &str,
    api_key: &str,
    model: &str,
    prompt: &str,
    max_tokens: u32,
) -> Result<String, String> {
    let req = match provider {
        "claude" => client
            .post(api_url)
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&serde_json::json!({
                "model": model,
                "max_tokens": max_tokens,
                "messages": [{ "role": "user", "content": prompt }]
            })),
        "openai" => client
            .post(api_url)
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&serde_json::json!({
                "model": model,
                "max_tokens": max_tokens,
                "messages": [{ "role": "user", "content": prompt }]
            })),
        _ => client
            .post(api_url)
            .json(&serde_json::json!({
                "model": model,
                "messages": [{ "role": "user", "content": prompt }],
                "stream": false
            })),
    };
    let resp = req.send().await.map_err(|e| format!("Request failed: {}", e))?;
    let status = resp.status();
    let raw = resp
        .text()
        .await
        .map_err(|e| format!("Could not read the response: {}", e))?;
    let body: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => {
            let snippet: String = raw.trim().chars().take(200).collect();
            return Err(format!("API returned {} with a non-JSON body: {}", status, snippet));
        }
    };
    if !status.is_success() {
        let msg = body["error"]["message"]
            .as_str()
            .or(body["error"].as_str())
            .unwrap_or("no details");
        return Err(format!("API returned {}: {}", status, msg));
    }
    let text = match provider {
        "claude" => body["content"][0]["text"].as_str(),
        "openai" => body["choices"][0]["message"]["content"].as_str(),
        _ => body["message"]["content"]
            .as_str()
            .or(body["choices"][0]["message"]["content"].as_str()),
    };
    text.map(|t| t.to_string())
        .ok_or_else(|| String::from("The API response had no text"))
}

fn parse_suggestion(text: &str) -> Result<serde_json::Value, String> {
    let start = text
        .find('{')
        .ok_or_else(|| String::from("The AI response had no JSON object"))?;
    let end = text
        .rfind('}')
        .ok_or_else(|| String::from("The AI response had no JSON object"))?;
    if end < start {
        return Err(String::from("The AI response had no JSON object"));
    }
    let v: serde_json::Value = serde_json::from_str(&text[start..=end])
        .map_err(|e| format!("Could not parse the AI response: {}", e))?;
    let command = v["command"].as_str().unwrap_or("").trim().to_string();
    let explanation = v["explanation"].as_str().unwrap_or("").trim().to_string();
    if command.contains('\n') || command.contains('\r') || command.chars().count() > 1000 {
        return Ok(serde_json::json!({
            "command": "",
            "explanation": "The AI proposed a multi-line or very long command, which is not accepted. Rephrase the request."
        }));
    }
    Ok(serde_json::json!({ "command": command, "explanation": explanation }))
}

#[tauri::command]
async fn ai_suggest_command(
    request: String,
    api_url: String,
    api_key: String,
    model: String,
    provider: String,
) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::new();
    let request: String = request.chars().take(500).collect();
    let prompt = format!(
        r#"You translate a sysadmin's request into ONE shell command to run over SSH on Linux servers. Both RHEL-family (AlmaLinux, CentOS) and Debian-family hosts may be present, so avoid package-manager commands unless the request is about packages.

Rules:
- Prefer read-only commands that only inspect state.
- One single line. Pipes are fine. No sudo, no ; or && chains, no redirection, no command substitution, no variables.
- The command must finish on its own: no -f/--follow, no watch or top, no interactive programs. For vmstat, iostat or mpstat always give both interval and count.
- If the request implies changing something (restart, install, delete, edit), still propose the command and say in the explanation that it modifies the system.
- If the request is unclear or cannot be done in one command, return an empty command and explain what is missing.
- Reply with ONLY a JSON object, no Markdown, in the form {{"command": "...", "explanation": "..."}}. The explanation is one or two short sentences.

REQUEST: {request}"#
    );
    let text = ai_call(&client, &provider, &api_url, &api_key, &model, &prompt, 400).await?;
    parse_suggestion(&text)
}

#[tauri::command]
async fn ai_fleet_summary(
    summary_text: String,
    api_url: String,
    api_key: String,
    model: String,
    provider: String,
) -> Result<String, String> {
    let client = reqwest::Client::new();
    let data: String = summary_text.chars().take(24000).collect();
    let prompt = format!(
        r#"You are a senior Linux sysadmin reviewing the results of an automated health check that ran on a group of hosts. Hosts with status OK are only counted, not listed. Each listed host line has the format: name (ip) -> STATUS: findings.

Write a concise report in Markdown:
1. One line with the overall picture (counts).
2. Group the flagged hosts that share the same kind of problem. Use naming patterns (same prefix or service family) only to group hosts. State a common cause only when the findings or the example log lines support it; otherwise say it cannot be inferred.
3. A short prioritised list of which hosts to open first and why.
Findings come from simple regex rules, so a count such as "N suspicious lines in dmesg" is not a measure of severity. Use the example log lines, when present, to judge whether they look benign or serious. When only a count is given, say the evidence is weak and do not rank hosts by that count. Never state or suggest a root cause (including misconfiguration, memory corruption, hardware or environment problems) unless a log line says so explicitly; describe what is observed and what to check instead. Each host has at most 3 example lines while the count may include other kinds of lines, so never claim that all counted lines are of the type shown in the examples. The same message on several hosts only means the message repeats; it can be harmless noise from a common image or tool, so do not present it as evidence of a shared fault. Stay consistent: if you say a cause cannot be inferred, do not claim one later. Only call a problem ongoing or recurring when the data shows it. Do not use Markdown headings; use bold labels and lists. Do not give shell commands: the per-host report provides them. Do not invent data that is not in the input.

DATA:
{data}"#
    );
    ai_call(&client, &provider, &api_url, &api_key, &model, &prompt, 1200).await
}

const HEALTH_RETENTION_DAYS: u64 = 90;

fn health_history_path() -> std::path::PathBuf {
    get_config_dir().join("health_history.jsonl")
}

#[tauri::command]
fn health_history_record(network: String, entries: Vec<HealthEntry>) {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Retention comes from Settings (monitoring.history_days). 0 keeps everything.
    let retention_days: u64 = host::load_config(get_config_dir().join("config.toml").to_str().unwrap())
        .ok()
        .and_then(|c| c.monitoring)
        .and_then(|m| m.history_days)
        .map(|d| d as u64)
        .unwrap_or(HEALTH_RETENTION_DAYS);
    let cutoff = if retention_days == 0 { 0 } else { now.saturating_sub(retention_days * 86400) };
    let path = health_history_path();
    let existing = std::fs::read_to_string(&path).unwrap_or_default();

    let mut kept: Vec<String> = Vec::new();
    let mut last: std::collections::HashMap<String, serde_json::Value> = std::collections::HashMap::new();
    let mut pruned = false;
    for line in existing.lines() {
        if line.trim().is_empty() { continue; }
        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => { pruned = true; continue; }
        };
        if v["ts"].as_u64().unwrap_or(0) < cutoff { pruned = true; continue; }
        if let (Some(n), Some(ip)) = (v["network"].as_str(), v["ip"].as_str()) {
            last.insert(format!("{}|{}", n, ip), v.clone());
        }
        kept.push(line.to_string());
    }

    let mut fresh: Vec<String> = Vec::new();
    for e in entries {
        let report: serde_json::Value = e.report.as_deref()
            .and_then(|r| serde_json::from_str(r).ok())
            .unwrap_or(serde_json::Value::Null);
        let issues: Vec<String> = report["issues"].as_array()
            .map(|a| a.iter().filter_map(|i| i["msg"].as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();
        let rec = serde_json::json!({
            "ts": now,
            "time": iso_utc(now),
            "network": network,
            "ip": e.ip,
            "status": e.status,
            "load1": report["metrics"]["load1"],
            "mem": report["metrics"]["memUsedPct"],
            "swap": report["metrics"]["swapUsedPct"],
            "issues": issues,
        });
        let key = format!("{}|{}", network, e.ip);
        if let Some(prev) = last.get(&key) {
            let same = prev["status"] == rec["status"]
                && prev["load1"] == rec["load1"]
                && prev["mem"] == rec["mem"]
                && prev["swap"] == rec["swap"]
                && prev["issues"] == rec["issues"];
            if same && now.saturating_sub(prev["ts"].as_u64().unwrap_or(0)) < 300 { continue; }
        }
        last.insert(key, rec.clone());
        fresh.push(rec.to_string());
    }
    if fresh.is_empty() && !pruned { return; }

    if pruned {
        let tmp = path.with_extension("jsonl.tmp");
        let _ = (|| -> std::io::Result<()> {
            let mut f = std::fs::OpenOptions::new()
                .create(true).write(true).truncate(true).mode(0o600).open(&tmp)?;
            for l in kept.iter().chain(fresh.iter()) { writeln!(f, "{}", l)?; }
            std::fs::rename(&tmp, &path)
        })();
    } else if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).mode(0o600).open(&path) {
        for l in &fresh { let _ = writeln!(f, "{}", l); }
    }
}

#[tauri::command]
fn get_health_history(network: String, ip: String, limit: usize) -> Vec<serde_json::Value> {
    let text = std::fs::read_to_string(health_history_path()).unwrap_or_default();
    let mut rows: Vec<serde_json::Value> = text
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter(|v| v["network"].as_str() == Some(network.as_str()) && v["ip"].as_str() == Some(ip.as_str()))
        .collect();
    rows.reverse();
    rows.truncate(limit);
    rows
}

// Where the SSH key picker opens: ~/.ssh if it exists, otherwise the home directory.
#[tauri::command]
fn key_picker_start() -> String {
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/"));
    let ssh = home.join(".ssh");
    let start = if ssh.is_dir() { ssh } else { home };
    start.to_string_lossy().into_owned()
}

#[derive(serde::Serialize)]
struct KeyPathChoice {
    path: String,
    note: Option<String>,
}

// Turns the file chosen in the picker into what gets stored. A path inside the home directory is
// written as ~/..., which is shorter and still works if the configuration is moved to another
// machine. A public key (.pub) is replaced by the private key next to it, when that file exists.
#[tauri::command]
fn normalize_key_path(path: String) -> KeyPathChoice {
    let mut p = std::path::PathBuf::from(path.trim());
    let mut note = None;
    if p.extension().map(|e| e == "pub").unwrap_or(false) {
        let private = p.with_extension("");
        if private.is_file() {
            p = private;
            note = Some(String::from("That was the public key (.pub). The private key next to it is used instead."));
        }
    }
    let shown = match dirs::home_dir().and_then(|h| p.strip_prefix(&h).ok().map(|r| r.to_path_buf())) {
        Some(rest) if !rest.as_os_str().is_empty() => format!("~/{}", rest.to_string_lossy()),
        _ => p.to_string_lossy().into_owned(),
    };
    KeyPathChoice { path: shown, note }
}

#[tauri::command]
fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// The version tag of the latest GitHub release, without the leading "v" ("0.1.7"), or None on ANY
/// failure - no internet, the site being blocked, GitHub being down, a malformed response. This is a
/// nice-to-have, never something to bother the person about: a failed check looks exactly like "no
/// update available".
#[tauri::command]
async fn check_latest_release() -> Option<String> {
    let client = reqwest::Client::new();
    let resp = client
        .get("https://api.github.com/repos/c0r3dump3d/merendero/releases/latest")
        .header("User-Agent", "Merendero")
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let json: serde_json::Value = resp.json().await.ok()?;
    let tag = json.get("tag_name")?.as_str()?;
    Some(tag.trim_start_matches('v').to_string())
}

#[tauri::command]
fn audit_event(action: String, network: String, hosts: Vec<String>, detail: String, success: bool) {
    let entries: Vec<serde_json::Value> = hosts
        .into_iter()
        .map(|ip| serde_json::json!({ "ip": ip }))
        .collect();
    let detail_value = serde_json::from_str::<serde_json::Value>(&detail)
        .ok()
        .filter(|v| v.is_object())
        .unwrap_or(serde_json::Value::String(detail));
    audit_write(&action, &network, entries, serde_json::json!({ "detail": detail_value, "success": success }));
}

#[tauri::command]
async fn run_command(
    hosts: Vec<host::Host>,
    network: host::Network,
    defaults: host::Defaults,
    command: String,
) -> Vec<serde_json::Value> {
    let net_name = network.name.clone();
    let targets: Vec<serde_json::Value> = hosts
        .iter()
        .map(|h| serde_json::json!({ "ip": h.ip, "hostname": h.hostname }))
        .collect();
    audit_write("run_command", &net_name, targets, serde_json::json!({ "phase": "start", "command": command }));
    let results = run_command_inner(hosts, network, defaults, command.clone()).await;
    let outcome: Vec<serde_json::Value> = results
        .iter()
        .map(|r| serde_json::json!({ "ip": r["ip"], "success": r["success"] }))
        .collect();
    audit_write("run_command", &net_name, outcome, serde_json::json!({ "phase": "end", "command": command }));
    results
}

#[tauri::command]
async fn run_scp_broadcast(
    hosts: Vec<host::Host>,
    network: host::Network,
    defaults: host::Defaults,
    local_path: String,
    remote_path: String,
    direction: String,
) -> Vec<serde_json::Value> {
    let net_name = network.name.clone();
    let transfer = serde_json::json!({ "direction": direction, "local": local_path, "remote": remote_path });
    let targets: Vec<serde_json::Value> = hosts
        .iter()
        .map(|h| serde_json::json!({ "ip": h.ip, "hostname": h.hostname }))
        .collect();
    audit_write("scp_broadcast", &net_name, targets, serde_json::json!({ "phase": "start", "transfer": transfer }));
    let results = run_scp_broadcast_inner(hosts, network, defaults, local_path, remote_path, direction).await;
    let outcome: Vec<serde_json::Value> = results
        .iter()
        .map(|r| serde_json::json!({ "ip": r["ip"], "success": r["success"] }))
        .collect();
    audit_write("scp_broadcast", &net_name, outcome, serde_json::json!({ "phase": "end", "transfer": transfer }));
    results
}

#[tauri::command]
fn run_scp(
    ip: String,
    username: String,
    port: u16,
    identity_file: Option<String>,
    local_path: String,
    remote_path: String,
    direction: String,
) -> Result<String, String> {
    let target = vec![serde_json::json!({ "ip": ip })];
    let transfer = serde_json::json!({ "direction": direction, "local": local_path, "remote": remote_path, "remote_user": username });
    audit_write("scp", "", target.clone(), serde_json::json!({ "phase": "start", "transfer": transfer }));
    let result = run_scp_inner(ip, username, port, identity_file, local_path, remote_path, direction);
    audit_write("scp", "", target, serde_json::json!({ "phase": "end", "success": result.is_ok(), "transfer": transfer }));
    result
}

#[tauri::command]
fn launch_ssh(
    ip: String,
    username: String,
    port: u16,
    terminal: Option<String>,
    hostname: Option<String>,
    network_name: Option<String>,
    identity_file: Option<String>,
    post_connect: Option<String>,
) -> Result<(), String> {
    let net_name = network_name.clone().unwrap_or_default();
    let target = vec![serde_json::json!({ "ip": ip, "hostname": hostname })];
    let detail = serde_json::json!({ "user": username, "port": port, "post_connect": post_connect });
    let result = launch_ssh_inner(ip, username, port, terminal, hostname, network_name, identity_file, post_connect);
    audit_write("ssh", &net_name, target, serde_json::json!({ "session": detail, "terminal_launched": result.is_ok() }));
    result
}

async fn run_command_inner(
    hosts: Vec<host::Host>,
    network: host::Network,
    defaults: host::Defaults,
    command: String,
) -> Vec<serde_json::Value> {
    use tokio::process::Command;

    let mut tasks = vec![];

    for host in hosts {
        let cmd = command.clone();
        let username = host::resolve_username(&host, &network, &defaults);
        let port = host::resolve_port(&host, &network, &defaults);
        let identity_arg = host.identity_file.clone()
            .or(network.identity_file.clone())
            .or(defaults.identity_file.clone())
            .map(|f| vec!["-i".to_string(), f])
            .unwrap_or_default();
        let ip = host.ip.clone();
        let hostname = host.hostname.clone();

        let task = tokio::spawn(async move {
            let mut args = vec![
                "-p".to_string(), port.to_string(),
                "-o".to_string(), "StrictHostKeyChecking=no".to_string(),
                "-o".to_string(), "ConnectTimeout=5".to_string(),
                "-o".to_string(), "BatchMode=yes".to_string(),
            ];
            args.extend(identity_arg);
            args.push(String::from("--"));
            args.push(format!("{}@{}", username, ip));
            args.push(cmd);

            let output = Command::new(program("ssh")).env("PATH", user_path())
                .args(&args)
                .output()
                .await;

            match output {
                Ok(out) => {
                    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                    serde_json::json!({
                        "ip": ip,
                        "hostname": hostname,
                        "stdout": stdout,
                        "stderr": stderr,
                        "success": out.status.success(),
                    })
                }
                Err(e) => serde_json::json!({
                    "ip": ip,
                    "hostname": hostname,
                    "stdout": "",
                    "stderr": format!("Failed to connect: {}", e),
                    "success": false,
                })
            }
        });
        tasks.push(task);
    }

    let mut results = vec![];
    for task in tasks {
        if let Ok(result) = task.await {
            results.push(result);
        }
    }
    results
}

#[tauri::command]
async fn health_check(
    ip: String,
    username: String,
    port: u16,
    identity_file: Option<String>,
) -> Result<serde_json::Value, String> {
    use tokio::process::Command;

    let identity_arg = identity_file
        .map(|f| vec!["-i".to_string(), f])
        .unwrap_or_default();

    let mut base_args = vec![
        "-p".to_string(), port.to_string(),
        "-o".to_string(), "StrictHostKeyChecking=no".to_string(),
        "-o".to_string(), "ConnectTimeout=5".to_string(),
        "-o".to_string(), "BatchMode=yes".to_string(),
    ];
    base_args.extend(identity_arg);
    // '--' before the destination: without it, a username/ip starting with '-' (e.g. from an imported
    // backup) could be read by ssh/scp as an OPTION instead of the destination.
    base_args.push(String::from("--"));
    base_args.push(format!("{}@{}", username, ip));

    // receta de Brendan Gregg — 60 second performance check
    let script = r#"
echo "=== OS ==="
cat /etc/os-release 2>/dev/null | grep -E '^(PRETTY_NAME|ID|ID_LIKE|VERSION_ID)='
uname -r
echo "=== UPTIME ==="
uptime
echo "=== DMESG_TAIL ==="
dmesg 2>/dev/null | tail -20
echo "=== VMSTAT ==="
vmstat 1 3 2>/dev/null
echo "=== MPSTAT ==="
mpstat -P ALL 1 1 2>/dev/null || echo "mpstat not available"
echo "=== PIDSTAT ==="
pidstat 1 1 2>/dev/null || echo "pidstat not available"
echo "=== IOSTAT ==="
iostat -xz 1 1 2>/dev/null || echo "iostat not available"
echo "=== FREE ==="
free -m
echo "=== NETSTAT ==="
ss -s 2>/dev/null || netstat -s 2>/dev/null | head -20
echo "=== TOP ==="
top -bn1 2>/dev/null | head -30
echo "=== TOP_MEM ==="
ps -eo pid,user,rss,%mem,comm --sort=-%mem 2>/dev/null | head -8
echo "=== DF ==="
df -h
echo "=== END ==="
"#;

    let mut args = base_args.clone();
    args.push(script.to_string());

    let output = Command::new(program("ssh")).env("PATH", user_path())
        .args(&args)
        .output()
        .await
        .map_err(|e| format!("SSH error: {}", e))?;

    let raw = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() && raw.is_empty() {
        return Err(format!("Connection failed: {}", stderr));
    }

    Ok(serde_json::json!({
        "raw": raw,
        "stderr": stderr,
        "ip": ip,
    }))
}

#[tauri::command]
fn save_health_status(
    mut networks_file: NetworksFile,
    network_index: usize,
    host_ip: String,
    status: String,
    report: Option<String>,
) -> Result<NetworksFile, String> {
    if network_index >= networks_file.networks.len() {
        return Err(String::from("Invalid network index"));
    }
    for host in networks_file.networks[network_index].hosts.iter_mut() {
        if host.ip == host_ip {
            host.health_status = Some(status.clone());
            if let Some(r) = report {
                host.health_report = Some(r);
            }
            break;
        }
    }
    let path = get_config_dir().join("networks.toml");
    host::save_networks(path.to_str().unwrap(), &networks_file)?;
    Ok(networks_file)
}

fn write_private(path: &std::path::Path, content: &str) -> Result<(), String> {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .map_err(|e| format!("Failed to write {}: {}", path.display(), e))?;
    // mode() only applies when the file is created: a file that already existed keeps its old permissions.
    f.set_permissions(std::fs::Permissions::from_mode(0o600))
        .map_err(|e| format!("Failed to restrict {}: {}", path.display(), e))?;
    f.write_all(content.as_bytes())
        .map_err(|e| format!("Failed to write {}: {}", path.display(), e))
}

// Restores a JSON-lines log. It never overwrites a log that already has entries:
// in that case the imported copy is saved next to it.
fn restore_log(dir: &std::path::Path, file: &str, imported: &str, notes: &mut Vec<String>) -> Result<(), String> {
    if imported.trim().is_empty() {
        return Ok(());
    }
    let valid = imported
        .lines()
        .filter(|l| !l.trim().is_empty())
        .all(|l| serde_json::from_str::<serde_json::Value>(l).is_ok());
    if !valid {
        notes.push(format!("{} in the backup is not valid JSON lines, so it was not restored.", file));
        return Ok(());
    }
    let local = dir.join(file);
    let local_empty = match std::fs::read_to_string(&local) {
        Ok(t) => t.trim().is_empty(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => true,
        Err(e) => return Err(format!("Could not read {}: {}", file, e)),
    };
    if local_empty {
        write_private(&local, imported)?;
        notes.push(format!("{} restored.", file));
    } else {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let side = format!("{}.imported-{}.jsonl", file.trim_end_matches(".jsonl"), now);
        write_private(&dir.join(&side), imported)?;
        notes.push(format!("{} already has entries, so it was kept. The imported copy is saved as {}.", file, side));
    }
    Ok(())
}

// ===== backup: begin =====
/// The API key of the AI provider, if the config text has a non-empty one.
fn api_key_of(value: &toml::Value) -> Option<&str> {
    value.get("ai")?.get("api_key")?.as_str().map(str::trim).filter(|k| !k.is_empty())
}

fn config_has_api_key(text: &str) -> bool {
    text.parse::<toml::Value>().map(|v| api_key_of(&v).is_some()).unwrap_or(false)
}

/// Takes the API key out of the text of a config.toml. If the text is not valid TOML it fails instead of
/// guessing, and the message does not quote the offending line (it could be the key itself).
fn config_without_api_key(text: &str) -> Result<String, String> {
    let mut value: toml::Value = text.parse().map_err(|e: toml::de::Error| {
        let line = e.span().and_then(|s| text.get(..s.start)).map(|before| before.matches('\n').count() + 1);
        match line {
            Some(l) => format!("config.toml is not valid TOML (near line {}), so the API key cannot be left out of the backup. Fix the file, or export with the key included.", l),
            None => String::from("config.toml is not valid TOML, so the API key cannot be left out of the backup. Fix the file, or export with the key included."),
        }
    })?;
    if let Some(ai) = value.get_mut("ai").and_then(|a| a.as_table_mut()) {
        ai.remove("api_key");
    }
    toml::to_string(&value).map_err(|e| format!("Could not write the config without the API key: {}", e))
}

#[derive(Debug, PartialEq)]
enum KeyRestore {
    /// The backup has no [ai] section, so there is no key to talk about.
    NoAi,
    /// The backup carries its own key.
    Included,
    /// The backup has no key and the config already here has one: it is kept.
    KeptLocal,
    /// The backup has no key and neither has the config here.
    Missing,
}

/// The text to write as config.toml when restoring `incoming`. A backup without the API key never wipes
/// the key that is already saved here.
fn config_for_restore(incoming: &str, local: Option<&str>) -> Result<(String, KeyRestore), String> {
    let mut value: toml::Value = match incoming.parse() {
        Ok(v) => v,
        Err(_) => return Ok((incoming.to_string(), KeyRestore::NoAi)),
    };
    if value.get("ai").and_then(|a| a.as_table()).is_none() {
        return Ok((incoming.to_string(), KeyRestore::NoAi));
    }
    if api_key_of(&value).is_some() {
        return Ok((incoming.to_string(), KeyRestore::Included));
    }
    let local_key = local
        .and_then(|t| t.parse::<toml::Value>().ok())
        .and_then(|v| api_key_of(&v).map(|k| k.to_string()));
    match local_key {
        Some(key) => {
            if let Some(ai) = value.get_mut("ai").and_then(|a| a.as_table_mut()) {
                ai.insert(String::from("api_key"), toml::Value::String(key));
            }
            let text = toml::to_string(&value).map_err(|e| format!("Could not merge the saved API key into the config: {}", e))?;
            Ok((text, KeyRestore::KeptLocal))
        }
        None => Ok((incoming.to_string(), KeyRestore::Missing)),
    }
}

fn build_backup(config_dir: &std::path::Path, include_api_key: bool, now: u64) -> Result<serde_json::Value, String> {
    let read = |name: &str| std::fs::read_to_string(config_dir.join(name)).unwrap_or_default();
    let config = read("config.toml");
    let has_key = config_has_api_key(&config);
    let config = if include_api_key { config } else { config_without_api_key(&config)? };
    Ok(serde_json::json!({
        "version": "1.0",
        "created_at": now,
        "includes_api_key": include_api_key && has_key,
        "config": config,
        "networks": read("networks.toml"),
        "history": read("history.toml"),
        "audit": read("audit.jsonl"),
        "health_history": read("health_history.jsonl"),
    }))
}

/// Calendar date (UTC) of a Unix time, without a date library.
fn civil_date(secs: u64) -> (i64, u32, u32) {
    let z = (secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn backup_dir() -> std::path::PathBuf {
    dirs::download_dir().or_else(dirs::home_dir).unwrap_or_else(|| std::path::PathBuf::from("."))
}

/// Writes `content` as a NEW file that only its owner can read, named "<prefix>-<date>.<ext>" (or
/// "...-2.<ext>", "...-3.<ext>"... if that name is taken). It never overwrites an existing file.
fn write_private_file(dir: &std::path::Path, prefix: &str, date: &str, ext: &str, content: &str) -> Result<std::path::PathBuf, String> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    for n in 1..=1000u32 {
        let name = if n == 1 { format!("{}-{}.{}", prefix, date, ext) } else { format!("{}-{}-{}.{}", prefix, date, n, ext) };
        let path = dir.join(&name);
        match std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&path) {
            Ok(mut f) => {
                if let Err(e) = f.write_all(content.as_bytes()) {
                    let _ = std::fs::remove_file(&path);
                    return Err(format!("Could not write {}: {}", path.display(), e));
                }
                return Ok(path);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("Could not save it in {}: {}", dir.display(), e)),
        }
    }
    Err(format!("There are too many files named {}-{} in {}", prefix, date, dir.display()))
}

fn save_backup_in(config_dir: &std::path::Path, out_dir: &std::path::Path, include_api_key: bool, now: u64) -> Result<serde_json::Value, String> {
    let backup = build_backup(config_dir, include_api_key, now)?;
    let content = serde_json::to_string_pretty(&backup).map_err(|e| format!("Could not build the backup: {}", e))?;
    let (y, m, d) = civil_date(now);
    let path = write_private_file(out_dir, "merendero-backup", &format!("{:04}-{:02}-{:02}", y, m, d), "json", &content)?;
    Ok(serde_json::json!({
        "path": path.to_string_lossy(),
        "includes_api_key": backup["includes_api_key"],
    }))
}

// ===== networks export/import: begin =====
/// Only the fields that describe the network itself, not what this machine has discovered or measured.
fn network_definition_only(n: &host::Network) -> host::Network {
    host::Network {
        name: n.name.clone(),
        network: n.network.clone(),
        username: n.username.clone(),
        port: n.port,
        identity_file: n.identity_file.clone(),
        scan_timeout: None,
        hosts: vec![],
    }
}

fn export_networks_in(networks_file: &NetworksFile, indices: &[usize], out_dir: &std::path::Path, now: u64) -> Result<serde_json::Value, String> {
    if indices.is_empty() {
        return Err(String::from("Select at least one network to export."));
    }
    let chosen: Vec<host::Network> = indices
        .iter()
        .map(|&i| networks_file.networks.get(i).map(network_definition_only).ok_or_else(|| String::from("Invalid network index")))
        .collect::<Result<_, _>>()?;
    let count = chosen.len();
    let out = NetworksFile { networks: chosen };
    let content = toml::to_string_pretty(&out).map_err(|e| format!("Could not build the file: {}", e))?;
    let (y, m, d) = civil_date(now);
    let path = write_private_file(out_dir, "merendero-networks", &format!("{:04}-{:02}-{:02}", y, m, d), "toml", &content)?;
    Ok(serde_json::json!({ "path": path.to_string_lossy(), "count": count }))
}

/// Adds every incoming network whose CIDR ("network" field) does not already exist locally. A network
/// that already exists (by CIDR, trimmed) is left completely untouched — name, hosts, everything.
fn import_networks_into(config_dir: &std::path::Path, content: &str) -> Result<serde_json::Value, String> {
    let incoming: NetworksFile = toml::from_str(content).map_err(|e| format!("That is not a valid networks file: {}", e))?;
    let path = config_dir.join("networks.toml");
    let mut current = host::load_networks(path.to_str().unwrap())?;
    let existing: std::collections::HashSet<String> = current.networks.iter().map(|n| n.network.trim().to_string()).collect();

    let mut added: Vec<String> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    for n in incoming.networks {
        let cidr = n.network.trim().to_string();
        if existing.contains(&cidr) {
            skipped.push(n.name);
        } else {
            added.push(n.name.clone());
            current.networks.push(network_definition_only(&n));
        }
    }
    if !added.is_empty() {
        host::save_networks(path.to_str().unwrap(), &current)?;
    }
    Ok(serde_json::json!({ "networks": current.networks, "added": added, "skipped": skipped }))
}

#[tauri::command]
fn export_networks(networks_file: NetworksFile, indices: Vec<usize>) -> Result<serde_json::Value, String> {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
    export_networks_in(&networks_file, &indices, &backup_dir(), now)
}

#[tauri::command]
fn import_networks(content: String) -> Result<serde_json::Value, String> {
    import_networks_into(&get_config_dir(), &content)
}
// ===== networks export/import: end =====

fn import_backup_into(config_dir: &std::path::Path, data: &serde_json::Value) -> Result<Vec<String>, String> {
    let mut notes: Vec<String> = Vec::new();

    for (field, file) in [("config", "config.toml"), ("networks", "networks.toml"), ("history", "history.toml")] {
        if let Some(text) = data[field].as_str() {
            if !text.is_empty() {
                let path = config_dir.join(file);
                if field == "config" {
                    let local = std::fs::read_to_string(&path).ok();
                    let (merged, key) = config_for_restore(text, local.as_deref())?;
                    write_private(&path, &merged)?;
                    match key {
                        KeyRestore::KeptLocal => notes.push(String::from("The backup does not include the AI API key, so the one already saved here was kept.")),
                        KeyRestore::Missing => notes.push(String::from("The backup does not include the AI API key: enter it again in Settings if you use AI analysis.")),
                        _ => {}
                    }
                } else {
                    write_private(&path, text)?;
                }
            }
        }
    }
    if let Some(text) = data["audit"].as_str() {
        restore_log(config_dir, "audit.jsonl", text, &mut notes)?;
    }
    if let Some(text) = data["health_history"].as_str() {
        restore_log(config_dir, "health_history.jsonl", text, &mut notes)?;
    }
    Ok(notes)
}
// ===== backup: end =====

#[tauri::command]
fn save_backup(include_api_key: bool) -> Result<serde_json::Value, String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    save_backup_in(&get_config_dir(), &backup_dir(), include_api_key, now)
}

#[tauri::command]
fn import_backup(data: serde_json::Value) -> Result<Vec<String>, String> {
    import_backup_into(&get_config_dir(), &data)
}

#[derive(serde::Deserialize)]
struct HealthEntry {
    ip: String,
    status: String,
    report: Option<String>,
}

#[tauri::command]
fn save_health_batch(
    mut networks_file: NetworksFile,
    network_index: usize,
    entries: Vec<HealthEntry>,
) -> Result<NetworksFile, String> {
    if network_index >= networks_file.networks.len() {
        return Err(String::from("Invalid network index"));
    }
    for entry in entries {
        if let Some(h) = networks_file.networks[network_index]
            .hosts
            .iter_mut()
            .find(|h| h.ip == entry.ip)
        {
            h.health_status = Some(entry.status);
            if let Some(r) = entry.report {
                h.health_report = Some(r);
            }
        }
    }
    let path = get_config_dir().join("networks.toml");
    host::save_networks(path.to_str().unwrap(), &networks_file)?;
    Ok(networks_file)
}

#[tauri::command]
async fn ai_analyze(
    raw_data: String,
    hostname: String,
    api_url: String,
    api_key: String,
    model: String,
    provider: String,
    local_summary: Option<String>,
) -> Result<String, String> {
    let client = reqwest::Client::new();
    let data: String = raw_data.chars().take(14000).collect();
    let summary = local_summary.unwrap_or_default();
    let summary = if summary.is_empty() { String::from("none") } else { summary };
    let prompt = format!(
        r#"You are a senior Linux sysadmin. Below is the output of a 60-second performance checklist (Brendan Gregg style) from host {hostname}. The OS section tells you the distribution.

Automated local analysis already flagged: {summary}

Write a concise report in Markdown:
1. First line: overall status in bold (OK, WARNING or CRITICAL).
2. Findings: at most 5 short bullets, based only on the data. If a section is missing or empty, say it could not be assessed instead of guessing.
3. For every WARNING or CRITICAL finding add a Fix subsection with exact CLI commands in fenced bash code blocks, adapted to the detected distribution (apt, dnf/yum, pacman...). Put read-only diagnostic commands first (to identify the cause, for example which process uses the memory), then the remediation. Add a comment starting with CAUTION before any command that is destructive, restarts a service or needs a reboot. Prefer reversible changes and mention how to make them persistent when relevant (fstab, sysctl.d).
4. If everything is healthy, say so in one line and give no commands.
Do not invent data that is not in the output.

HOST OUTPUT:
{data}"#
    );

    let response_text = match provider.as_str() {
        "claude" => {
            let body = serde_json::json!({
                "model": model,
                "max_tokens": 1500,
                "messages": [{ "role": "user", "content": prompt }]
            });
            let resp = client.post(&api_url)
                .header("x-api-key", &api_key)
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json")
                .json(&body)
                .send()
                .await
                .map_err(|e| format!("Request failed: {}", e))?;
            let data: serde_json::Value = resp.json().await
                .map_err(|e| format!("Parse failed: {}", e))?;
            data["content"][0]["text"].as_str().unwrap_or("No response").to_string()
        }
        "openai" => {
            let body = serde_json::json!({
                "model": model,
                "max_tokens": 1500,
                "messages": [{ "role": "user", "content": prompt }]
            });
            let resp = client.post(&api_url)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("content-type", "application/json")
                .json(&body)
                .send()
                .await
                .map_err(|e| format!("Request failed: {}", e))?;
            let data: serde_json::Value = resp.json().await
                .map_err(|e| format!("Parse failed: {}", e))?;
            data["choices"][0]["message"]["content"].as_str().unwrap_or("No response").to_string()
        }
        _ => {
            // ollama y custom
            let body = serde_json::json!({
                "model": model,
                "messages": [{ "role": "user", "content": prompt }],
                "stream": false
            });
            let resp = client.post(&api_url)
                .header("content-type", "application/json")
                .json(&body)
                .send()
                .await
                .map_err(|e| format!("Request failed: {}", e))?;
            let data: serde_json::Value = resp.json().await
                .map_err(|e| format!("Parse failed: {}", e))?;
            data["message"]["content"].as_str()
                .or(data["choices"][0]["message"]["content"].as_str())
                .unwrap_or("No response").to_string()
        }
    };

    Ok(response_text)
}

#[tauri::command]
async fn check_hosts_reachable(
    hosts: Vec<host::Host>,
    network: host::Network,
    defaults: host::Defaults,
) -> Vec<serde_json::Value> {
    use tokio::net::TcpStream;
    use std::time::Duration;

    let mut tasks = vec![];

    for host in hosts {
        let port = host.port.unwrap_or(network.port.unwrap_or(defaults.port));
        let timeout = Duration::from_secs(network.scan_timeout.unwrap_or(3));
        let ip = host.ip.clone();
        let hostname = host.hostname.clone();

        let task = tokio::spawn(async move {
            let address = format!("{}:{}", ip, port);
            let reachable = match tokio::time::timeout(timeout, TcpStream::connect(&address)).await {
                Ok(Ok(_)) => true,
                _ => false,
            };
            serde_json::json!({
                "ip": ip,
                "hostname": hostname,
                "reachable": reachable,
            })
        });
        tasks.push(task);
    }

    let mut results = vec![];
    for task in tasks {
        if let Ok(r) = task.await { results.push(r); }
    }
    results
}

#[tauri::command]
fn launch_broadcast(hosts: Vec<host::Host>, network: host::Network, defaults: Defaults, session_name: String) -> Result<(), String> {
    if hosts.is_empty() { return Err(String::from("No hosts selected")); }
    let tmux = program("tmux");
    let run_tmux = |args: &[&str]| std::process::Command::new(&tmux).env("PATH", user_path()).args(args).output();
    let ssh = sh_quote(&program("ssh"));
    // Same SSH key rules as a single session: the host's key, then the network's, then the global default
    let pane_cmd = |h: &host::Host| -> String {
        let key = h.identity_file.clone().or(network.identity_file.clone()).or(defaults.identity_file.clone())
            .filter(|f| !f.trim().is_empty());
        let ident = key.map(|f| format!(" -i {}", sh_quote(f.trim()))).unwrap_or_default();
        let target = sh_quote(&format!("{}@{}", host::resolve_username(h, &network, &defaults), h.ip));
        format!("{} -p {}{} -- {}", ssh, host::resolve_port(h, &network, &defaults), ident, target)
    };
    let _ = run_tmux(&["kill-session", "-t", &session_name]);
    let out = run_tmux(&["new-session", "-d", "-s", &session_name, "-x", "220", "-y", "50", &pane_cmd(&hosts[0])])
        .map_err(|e| format!("Failed to run tmux: {}", e))?;
    if !out.status.success() {
        return Err(format!("tmux could not create the session: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    for (i, h) in hosts.iter().enumerate().skip(1) {
        let out = run_tmux(&["split-window", "-t", &session_name, "-v", &pane_cmd(h)])
            .map_err(|e| format!("Failed to run tmux: {}", e))?;
        if !out.status.success() {
            let _ = run_tmux(&["kill-session", "-t", &session_name]);
            return Err(format!("tmux has no room for more panes: only {} of {} hosts fit. Select fewer hosts", i, hosts.len()));
        }
        let _ = run_tmux(&["select-layout", "-t", &session_name, "tiled"]);
    }
    let _ = run_tmux(&["set-window-option", "-t", &session_name, "synchronize-panes", "on"]);
    let attach = format!("{} attach-session -t {}", sh_quote(&tmux), sh_quote(&session_name));
    // With several hosts the panes get small, so the terminal opens maximized when it can
    let launched = terminal_command(defaults.terminal.as_deref(), &attach, hosts.len() >= 4)
        .and_then(|mut c| c.spawn().map(|_| ()).map_err(|e| format!("Failed to launch terminal: {}", e)));
    if launched.is_err() {
        let _ = run_tmux(&["kill-session", "-t", &session_name]);
    }
    launched
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(ScanControl::default())
        .invoke_handler(tauri::generate_handler![
            get_networks,
            get_config,
            save_networks,
            add_network,
            update_network,
            delete_network,
            add_host,
            update_host,
            delete_host,
            bulk_tag,
            bulk_set_username,
            save_config,
            get_history,
            launch_ssh,
            scan_network,
            cancel_scan,
            network_scan_info,
            check_hosts,
            list_remote_dir,
            run_scp,
            run_scp_broadcast,
            run_command,
            health_check,
            save_backup,
            export_networks,
            import_networks,
            import_backup,
            ai_fleet_summary,
            app_version,
            check_latest_release,
            key_picker_start,
            list_terminals,
            default_terminal,
            normalize_key_path,
            ai_suggest_command,
            health_history_record,
            get_health_history,
            audit_event,
            save_health_batch,
            save_health_status,
            ai_analyze,
            check_hosts_reachable,
            launch_broadcast,
            recalibrate_network,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
