mod host;
mod scanner;
mod ui;
mod ssh;
mod tmux;

use host::{load_config, load_networks, save_networks, resolve_username, resolve_port};
use futures::future::join_all;
use std::path::PathBuf;

fn get_config_dir() -> PathBuf {
    let mut path = dirs::home_dir().expect("Cannot find home directory");
    path.push(".config");
    path.push("merendero");
    path
}

fn select_network(networks_file: &host::NetworksFile) -> Option<usize> {
    println!("\nAvailable networks:");
    for (i, network) in networks_file.networks.iter().enumerate() {
        println!("  {}. {} - {} ({} hosts)", i + 1, network.name, network.network, network.hosts.len());
    }
    let input = ui::read_input("\nSelect network: ");
    match input.parse::<usize>() {
        Ok(n) if n > 0 && n <= networks_file.networks.len() => Some(n - 1),
        _ => { println!("Invalid selection"); None }
    }
}

fn select_host(network: &host::Network) -> Option<usize> {
    let pattern = ui::read_input("Filter (ip/fqdn/tag, leave empty for all): ");
    let filtered = ui::filter_hosts(&network.hosts, &pattern);

    if filtered.is_empty() {
        println!("No hosts match that filter");
        return None;
    }

    println!("\nHosts in [{}]{}:",
        network.name,
        if pattern.is_empty() { String::new() } else { format!(" (filter: {})", pattern) }
    );

    for (display_idx, (_, host)) in filtered.iter().enumerate() {
        let hostname = host.hostname.clone().unwrap_or(String::from(""));
        let tag = host.tag.clone().unwrap_or(String::from(""));
        println!("  {}. {} | {} | {} | tag: {}",
            display_idx + 1,
            if host.active { "✓" } else { "✗" },
            host.ip, hostname, tag
        );
    }

    let input = ui::read_input("\nSelect host: ");
    match input.parse::<usize>() {
        Ok(n) if n > 0 && n <= filtered.len() => Some(filtered[n - 1].0),
        _ => { println!("Invalid selection"); None }
    }
}

#[tokio::main]
async fn main() {
    let config_dir = get_config_dir();

    if !config_dir.exists() {
        std::fs::create_dir_all(&config_dir).expect("Cannot create config directory");
        println!("Created config directory: {}", config_dir.display());
    }

    let config_path = config_dir.join("config.toml");
    let networks_path = config_dir.join("networks.toml");

    if !config_path.exists() {
        std::fs::write(&config_path, "[defaults]\nusername = \"root\"\nport = 22\n")
            .expect("Cannot create default config.toml");
    }

    if !networks_path.exists() {
        std::fs::write(&networks_path, "networks = []\n")
            .expect("Cannot create default networks.toml");
    }

    let config = match load_config(config_path.to_str().unwrap()) {
        Ok(c) => c,
        Err(e) => { println!("Error loading config: {}", e); return; }
    };

    let mut networks_file = match load_networks(networks_path.to_str().unwrap()) {
        Ok(n) => n,
        Err(e) => { println!("Error loading networks: {}", e); return; }
    };

    loop {
        ui::show_menu();
        let option = ui::read_input("\nSelect option: ");

        match option.as_str() {
            "1" => {
                println!("\nConfigured networks:");
                for (i, network) in networks_file.networks.iter().enumerate() {
                    let username = network.username.clone().unwrap_or(config.defaults.username.clone());
                    let port = network.port.unwrap_or(config.defaults.port);
                    println!("  {}. {} | {} | user: {} | port: {} | hosts: {}",
                        i + 1, network.name, network.network, username, port, network.hosts.len());
                }
            }
            "2" => {
                let pattern = ui::read_input("Filter (ip/fqdn/tag, leave empty for all): ");
                for network in &networks_file.networks {
                    let filtered = ui::filter_hosts(&network.hosts, &pattern);
                    if filtered.is_empty() { continue; }
                    println!("\n[{}] - {}", network.name, network.network);
                    for (_, host) in &filtered {
                        let username = resolve_username(host, network, &config.defaults);
                        let port = resolve_port(host, network, &config.defaults);
                        let hostname = host.hostname.clone().unwrap_or(String::from(""));
                        let tag = host.tag.clone().unwrap_or(String::from(""));
                        println!("  {} | {} | {}@{}:{} | {} | tag: {}",
                            if host.active { "✓" } else { "✗" },
                            host.ip, username, host.ip, port, hostname, tag);
                    }
                }
            }
            "3" => {
                for network in &networks_file.networks {
                    println!("\nChecking [{}]...", network.name);
                    let mut tasks = vec![];
                    for host in &network.hosts {
                        tasks.push(scanner::check_host(host.clone(), network, &config.defaults));
                    }
                    let results = join_all(tasks).await;
                    for (host, status) in results {
                        let username = resolve_username(&host, network, &config.defaults);
                        let port = resolve_port(&host, network, &config.defaults);
                        println!("  {} {}@{}:{}",
                            if status { "✓" } else { "✗" }, username, host.ip, port);
                    }
                }
            }
            "4" => {
                let network = ui::prompt_new_network();
                println!("\nAdding network [{}] - {}", network.name, network.network);
                networks_file.networks.push(network);
                match save_networks(networks_path.to_str().unwrap(), &networks_file) {
                    Ok(_) => println!("Network saved successfully"),
                    Err(e) => println!("Error saving network: {}", e),
                }
            }
            "5" => {
                let net_index = match select_network(&networks_file) {
                    Some(i) => i,
                    None => continue,
                };
                let host = ui::prompt_new_host();
                println!("\nAdding host {} to [{}]", host.ip, networks_file.networks[net_index].name);
                networks_file.networks[net_index].hosts.push(host);
                match save_networks(networks_path.to_str().unwrap(), &networks_file) {
                    Ok(_) => println!("Host saved successfully"),
                    Err(e) => println!("Error saving: {}", e),
                }
            }
            "6" => {
                let index = match select_network(&networks_file) {
                    Some(i) => i,
                    None => continue,
                };
                let discovered = scanner::scan_network(&networks_file.networks[index], &config.defaults).await;
                if discovered.is_empty() {
                    println!("No hosts found");
                } else {
                    println!("\nFound {} hosts, merging...", discovered.len());
                    let existing = &mut networks_file.networks[index].hosts;
                    for host in existing.iter_mut() { host.active = false; }
                    for new_host in discovered {
                        match existing.iter_mut().find(|h| h.ip == new_host.ip) {
                            Some(existing_host) => {
                                existing_host.active = true;
                                existing_host.hostname = new_host.hostname;
                            }
                            None => { existing.push(new_host); }
                        }
                    }
                    match save_networks(networks_path.to_str().unwrap(), &networks_file) {
                        Ok(_) => println!("Network saved successfully"),
                        Err(e) => println!("Error saving: {}", e),
                    }
                }
            }
            "7" => {
                let net_index = match select_network(&networks_file) {
                    Some(i) => i,
                    None => continue,
                };
                let host_index = match select_host(&networks_file.networks[net_index]) {
                    Some(i) => i,
                    None => continue,
                };
                let network = &networks_file.networks[net_index];
                ssh::launch_ssh(&network.hosts[host_index], network, &config.defaults);
            }
            "8" => {
                let net_index = match select_network(&networks_file) {
                    Some(i) => i,
                    None => continue,
                };
                let name = networks_file.networks[net_index].name.clone();
                let confirm = ui::read_input(&format!("Delete network [{}]? (yes/no): ", name));
                if confirm == "yes" {
                    networks_file.networks.remove(net_index);
                    match save_networks(networks_path.to_str().unwrap(), &networks_file) {
                        Ok(_) => println!("Network [{}] deleted", name),
                        Err(e) => println!("Error saving: {}", e),
                    }
                } else {
                    println!("Cancelled");
                }
            }
            "9" => {
                let net_index = match select_network(&networks_file) {
                    Some(i) => i,
                    None => continue,
                };
                let host_index = match select_host(&networks_file.networks[net_index]) {
                    Some(i) => i,
                    None => continue,
                };
                let ip = networks_file.networks[net_index].hosts[host_index].ip.clone();
                let confirm = ui::read_input(&format!("Delete host {}? (yes/no): ", ip));
                if confirm == "yes" {
                    networks_file.networks[net_index].hosts.remove(host_index);
                    match save_networks(networks_path.to_str().unwrap(), &networks_file) {
                        Ok(_) => println!("Host {} deleted", ip),
                        Err(e) => println!("Error saving: {}", e),
                    }
                } else {
                    println!("Cancelled");
                }
            }
            "10" => {
                let net_index = match select_network(&networks_file) {
                    Some(i) => i,
                    None => continue,
                };
                let host_index = match select_host(&networks_file.networks[net_index]) {
                    Some(i) => i,
                    None => continue,
                };
                let current = networks_file.networks[net_index].hosts[host_index].clone();
                let updated = ui::prompt_edit_host(&current);
                networks_file.networks[net_index].hosts[host_index] = updated;
                match save_networks(networks_path.to_str().unwrap(), &networks_file) {
                    Ok(_) => println!("Host updated successfully"),
                    Err(e) => println!("Error saving: {}", e),
                }
            }
            "11" => {
                let net_index = match select_network(&networks_file) {
                    Some(i) => i,
                    None => continue,
                };
                let network = &networks_file.networks[net_index];
                let pattern = ui::read_input("Filter hosts (ip/fqdn/tag, leave empty for all): ");
                let filtered = ui::filter_hosts(&network.hosts, &pattern);

                if filtered.is_empty() {
                    println!("No hosts match that filter");
                    continue;
                }

                println!("\nHosts to broadcast{}:",
                    if pattern.is_empty() { String::new() } else { format!(" (filter: {})", pattern) }
                );
                for (_, host) in &filtered {
                    let hostname = host.hostname.clone().unwrap_or(String::from(""));
                    println!("  {} | {}", host.ip, hostname);
                }

                let confirm = ui::read_input(&format!("\nOpen {} hosts in tmux with sync? (yes/no): ", filtered.len()));
                if confirm != "yes" {
                    println!("Cancelled");
                    continue;
                }

                let session_name = format!("merendero-{}", network.name.to_lowercase().replace(' ', "-"));
                let hosts_data: Vec<(&host::Host, &host::Network, &host::Defaults)> = filtered
                    .iter()
                    .map(|(_, host)| (*host, network, &config.defaults))
                    .collect();

                tmux::launch_broadcast(hosts_data, &session_name);
            }
            "12" => {
                println!("Bye!");
                break;
            }
            _ => {
                println!("Invalid option");
            }
        }
    }
}
