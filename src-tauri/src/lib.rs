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

fn launch_ssh_inner(ip: String, username: String, port: u16, terminal: Option<String>, hostname: Option<String>, network_name: Option<String>, identity_file: Option<String>, post_connect: Option<String>) -> Result<(), String> {
    let identity_arg = identity_file
        .map(|f| format!(" -i {}", f))
        .unwrap_or_default();
    let ssh_cmd = match post_connect {
        Some(cmd) => format!("/usr/bin/ssh -t -p {}{} {}@{} '{}'", port, identity_arg, username, ip, cmd),
        None => format!("/usr/bin/ssh -p {}{} {}@{}", port, identity_arg, username, ip),
    };
    let preferred = terminal.as_deref().unwrap_or("terminator");
    let result = match preferred {
        "terminator" => std::process::Command::new("/usr/bin/terminator").env("PATH", "/usr/local/sbin:/usr/local/bin:/usr/bin:/usr/sbin:/bin:/sbin").args(["-e", &ssh_cmd]).spawn(),
        "alacritty" => std::process::Command::new("/usr/bin/alacritty").env("PATH", "/usr/local/sbin:/usr/local/bin:/usr/bin:/usr/sbin:/bin:/sbin").args(["-e", "bash", "-c", &ssh_cmd]).spawn(),
        "kitty" => std::process::Command::new("/usr/bin/kitty").env("PATH", "/usr/local/sbin:/usr/local/bin:/usr/bin:/usr/sbin:/bin:/sbin").args(["bash", "-c", &ssh_cmd]).spawn(),
        _ => std::process::Command::new(preferred).env("PATH", "/usr/local/sbin:/usr/local/bin:/usr/bin:/usr/sbin:/bin:/sbin").args(["-e", &ssh_cmd]).spawn(),
    };
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
    result.map(|_| ()).map_err(|e| format!("Failed to launch terminal: {}", e))
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

#[tauri::command]
async fn scan_network(
    window: tauri::Window,
    mut networks_file: NetworksFile,
    network_index: usize,
    defaults: Defaults,
) -> Result<NetworksFile, String> {
    if network_index >= networks_file.networks.len() {
        return Err(String::from("Invalid network index"));
    }

    validate_cidr(&networks_file.networks[network_index].network)?;

    // calibra si no tiene timeout
    let timeout = if let Some(t) = networks_file.networks[network_index].scan_timeout {
        t
    } else {
        let _ = window.emit("scan-status", "Calibrating network latency...");
        let t = calibrate_timeout(&networks_file.networks[network_index], &defaults).await;
        networks_file.networks[network_index].scan_timeout = Some(t);
        let _ = window.emit("scan-status", format!("Timeout calibrated: {}s", t));
        t
    };

    let network = networks_file.networks[network_index].clone();
    let discovered = scanner::scan_network_with_progress(&network, &defaults, window, timeout).await;

    if !discovered.is_empty() {
        let existing = &mut networks_file.networks[network_index].hosts;
        for host in existing.iter_mut() { host.active = false; }
        for new_host in discovered {
            match existing.iter_mut().find(|h| h.ip == new_host.ip) {
                Some(existing_host) => { existing_host.active = true; if new_host.hostname.is_some() { existing_host.hostname = new_host.hostname; } }
                None => { existing.push(new_host); }
            }
        }
    }

    let path = get_config_dir().join("networks.toml");
    host::save_networks(path.to_str().unwrap(), &networks_file)?;
    Ok(networks_file)
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

#[tauri::command]
async fn check_hosts(
    window: tauri::Window,
    mut networks_file: NetworksFile,
    network_index: usize,
    defaults: Defaults,
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

    let mut tasks = vec![];
    for (ip, p) in &hosts_snapshot {
        let ip = ip.clone();
        let p = *p;
        tasks.push(tokio::spawn(async move {
            let address = format!("{}:{}", ip, p);
            let reachable = match tokio::time::timeout(Duration::from_secs(timeout_secs), TcpStream::connect(&address)).await {
                Ok(Ok(_)) => true,
                _ => false,
            };
            (ip, reachable)
        }));
    }

    let mut current = 0;
    let mut results = std::collections::HashMap::new();
    for task in tasks {
        current += 1;
        if let Ok((ip, status)) = task.await {
            results.insert(ip, status);
        }
        let _ = window.emit("check-progress", serde_json::json!({ "total": total, "current": current }));
    }

    for host in networks_file.networks[network_index].hosts.iter_mut() {
        if let Some(&status) = results.get(&host.ip) {
            host.active = status;
        }
    }

    let path = get_config_dir().join("networks.toml");
    host::save_networks(path.to_str().unwrap(), &networks_file)?;
    Ok(networks_file)
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
    args.push(format!("for f in {}/* {}/.*; do [ -e \"$f\" ] || continue; n=$(basename \"$f\"); [ \"$n\" = \".\" ] || [ \"$n\" = \"..\" ] && continue; if [ -d \"$f\" ]; then echo \"d:$n\"; else echo \"f:$n\"; fi; done 2>/dev/null", path, path));

    let output = Command::new("/usr/bin/ssh")
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

    if direction == "upload" {
        args.push(local_path);
        args.push(remote);
    } else {
        args.push(remote);
        args.push(local_path);
    }

    let output = std::process::Command::new("/usr/bin/scp")
        .env("PATH", "/usr/local/sbin:/usr/local/bin:/usr/bin:/usr/sbin:/bin:/sbin")
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

            if dir == "upload" {
                args.push(local);
                args.push(remote_full);
            } else {
                args.push(remote_full);
                args.push(local);
            }

            let output = Command::new("/usr/bin/scp")
                .env("PATH", "/usr/local/sbin:/usr/local/bin:/usr/bin:/usr/sbin:/bin:/sbin")
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
    let cutoff = now.saturating_sub(HEALTH_RETENTION_DAYS * 86400);
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
            args.push(format!("{}@{}", username, ip));
            args.push(cmd);

            let output = Command::new("/usr/bin/ssh")
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

    let output = Command::new("/usr/bin/ssh")
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
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .map_err(|e| format!("Failed to write {}: {}", path.display(), e))?;
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

#[tauri::command]
fn export_backup() -> Result<serde_json::Value, String> {
    let config_dir = get_config_dir();
    let read = |name: &str| std::fs::read_to_string(config_dir.join(name)).unwrap_or_default();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    Ok(serde_json::json!({
        "version": "1.0",
        "created_at": now,
        "config": read("config.toml"),
        "networks": read("networks.toml"),
        "history": read("history.toml"),
        "audit": read("audit.jsonl"),
        "health_history": read("health_history.jsonl"),
    }))
}

#[tauri::command]
fn import_backup(data: serde_json::Value) -> Result<Vec<String>, String> {
    let config_dir = get_config_dir();
    let mut notes: Vec<String> = Vec::new();

    for (field, file) in [("config", "config.toml"), ("networks", "networks.toml"), ("history", "history.toml")] {
        if let Some(text) = data[field].as_str() {
            if !text.is_empty() {
                write_private(&config_dir.join(file), text)?;
            }
        }
    }
    if let Some(text) = data["audit"].as_str() {
        restore_log(&config_dir, "audit.jsonl", text, &mut notes)?;
    }
    if let Some(text) = data["health_history"].as_str() {
        restore_log(&config_dir, "health_history.jsonl", text, &mut notes)?;
    }
    Ok(notes)
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
    std::process::Command::new("tmux").args(["kill-session", "-t", &session_name]).output().ok();
    let first_ssh = format!("/usr/bin/ssh -p {} {}@{}", host::resolve_port(&hosts[0], &network, &defaults), host::resolve_username(&hosts[0], &network, &defaults), hosts[0].ip);
    std::process::Command::new("tmux").env("PATH", "/usr/local/sbin:/usr/local/bin:/usr/bin:/usr/sbin:/bin:/sbin").args(["new-session", "-d", "-s", &session_name, "-x", "220", "-y", "50", &first_ssh]).status().map_err(|e| format!("Failed to create tmux session: {}", e))?;
    for host in hosts.iter().skip(1) {
        let ssh_cmd = format!("/usr/bin/ssh -p {} {}@{}", host::resolve_port(host, &network, &defaults), host::resolve_username(host, &network, &defaults), host.ip);
        std::process::Command::new("tmux").env("PATH", "/usr/local/sbin:/usr/local/bin:/usr/bin:/usr/sbin:/bin:/sbin").args(["split-window", "-t", &session_name, "-v", &ssh_cmd]).status().ok();
        std::process::Command::new("tmux").args(["select-layout", "-t", &session_name, "tiled"]).status().ok();
    }
    std::process::Command::new("tmux").args(["set-window-option", "-t", &session_name, "synchronize-panes", "on"]).status().ok();
    let attach_cmd = format!("/usr/bin/tmux attach-session -t {}", session_name);
    std::process::Command::new("/usr/bin/terminator").env("PATH", "/usr/local/sbin:/usr/local/bin:/usr/bin:/usr/sbin:/bin:/sbin").args(["-e", &attach_cmd]).spawn().map_err(|e| format!("Failed to launch terminal: {}", e))?;
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
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
            save_config,
            get_history,
            launch_ssh,
            scan_network,
            check_hosts,
            list_remote_dir,
            run_scp,
            run_scp_broadcast,
            run_command,
            health_check,
            export_backup,
            import_backup,
            ai_fleet_summary,
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
