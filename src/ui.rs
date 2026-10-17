use std::io;
use std::io::Write;
use crate::host::{Network, Host};

pub fn read_input(prompt: &str) -> String {
    print!("{}", prompt);
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    input.trim().to_string()
}

pub fn show_menu() {
    println!("\nMerendero v0.1.0");
    println!("================");
    println!("1.  List networks");
    println!("2.  List hosts");
    println!("3.  Check hosts");
    println!("4.  Add network");
    println!("5.  Add host");
    println!("6.  Scan network");
    println!("7.  Connect to host");
    println!("8.  Delete network");
    println!("9.  Delete host");
    println!("10. Edit host");
    println!("11. Broadcast (tmux)");
    println!("12. Exit");
}

pub fn prompt_new_network() -> Network {
    let name = read_input("Network name: ");
    let network = read_input("Network CIDR (e.g. 192.168.1.0/24): ");

    let username_input = read_input("Username (leave empty for default): ");
    let username = if username_input.is_empty() { None } else { Some(username_input) };

    let port_input = read_input("Port (leave empty for default): ");
    let port = if port_input.is_empty() {
        None
    } else {
        match port_input.parse::<u16>() {
            Ok(p) => Some(p),
            Err(_) => { println!("Invalid port, using default"); None }
        }
    };

    Network { name, network, username, port, hosts: vec![] }
}

pub fn prompt_new_host() -> Host {
    let ip = read_input("IP address: ");

    let hostname_input = read_input("Hostname (leave empty if none): ");
    let hostname = if hostname_input.is_empty() { None } else { Some(hostname_input) };

    let tag_input = read_input("Tag (leave empty if none): ");
    let tag = if tag_input.is_empty() { None } else { Some(tag_input) };

    let username_input = read_input("Username (leave empty for network/default): ");
    let username = if username_input.is_empty() { None } else { Some(username_input) };

    let port_input = read_input("Port (leave empty for network/default): ");
    let port = if port_input.is_empty() {
        None
    } else {
        match port_input.parse::<u16>() {
            Ok(p) => Some(p),
            Err(_) => { println!("Invalid port, using default"); None }
        }
    };

    Host { ip, hostname, tag, username, port, active: true }
}

pub fn prompt_edit_host(host: &Host) -> Host {
    println!("Leave empty to keep current value");

    let hostname_input = read_input(&format!("Hostname [{}]: ",
        host.hostname.clone().unwrap_or(String::from(""))));
    let hostname = if hostname_input.is_empty() {
        host.hostname.clone()
    } else {
        Some(hostname_input)
    };

    let tag_input = read_input(&format!("Tag [{}]: ",
        host.tag.clone().unwrap_or(String::from(""))));
    let tag = if tag_input.is_empty() {
        host.tag.clone()
    } else {
        Some(tag_input)
    };

    let username_input = read_input(&format!("Username [{}]: ",
        host.username.clone().unwrap_or(String::from("network/default"))));
    let username = if username_input.is_empty() {
        host.username.clone()
    } else {
        Some(username_input)
    };

    let port_input = read_input(&format!("Port [{}]: ",
        host.port.map(|p| p.to_string()).unwrap_or(String::from("network/default"))));
    let port = if port_input.is_empty() {
        host.port
    } else {
        match port_input.parse::<u16>() {
            Ok(p) => Some(p),
            Err(_) => { println!("Invalid port, keeping current"); host.port }
        }
    };

    Host {
        ip: host.ip.clone(),
        hostname,
        tag,
        username,
        port,
        active: host.active,
    }
}

pub fn filter_hosts<'a>(hosts: &'a [Host], pattern: &str) -> Vec<(usize, &'a Host)> {
    let pattern = pattern.to_lowercase();
    hosts.iter().enumerate().filter(|(_, host)| {
        if pattern.is_empty() {
            return true;
        }
        let hostname = host.hostname.clone().unwrap_or_default().to_lowercase();
        let tag = host.tag.clone().unwrap_or_default().to_lowercase();
        let ip = host.ip.to_lowercase();

        hostname.contains(&pattern) || ip.contains(&pattern) || tag.contains(&pattern)
    }).collect()
}
