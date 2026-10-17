use std::process::Command;
use crate::host::{Host, Network, Defaults, resolve_username, resolve_port};

pub fn launch_ssh(host: &Host, network: &Network, defaults: &Defaults) {
    let username = resolve_username(host, network, defaults);
    let port = resolve_port(host, network, defaults);
    let target = format!("{}@{}", username, host.ip);

    println!("Connecting to {}:{}...", target, port);

    Command::new("ssh")
        .args(["-p", &port.to_string(), &target])
        .status()
        .expect("Failed to launch ssh");
}
