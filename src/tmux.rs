use std::process::Command;
use crate::host::{Host, Network, Defaults, resolve_username, resolve_port};

pub fn launch_broadcast(hosts: Vec<(&Host, &Network, &Defaults)>, session_name: &str) {
    if hosts.is_empty() {
        println!("No hosts selected");
        return;
    }

    // mata la sesión si ya existe
    Command::new("tmux")
        .args(["kill-session", "-t", session_name])
        .output()
        .ok();

    let first_cmd = ssh_command(hosts[0].0, hosts[0].1, hosts[0].2);

    // crea la sesión lanzando directamente el primer ssh
    Command::new("tmux")
        .args([
            "new-session", "-d",
            "-s", session_name,
            "-x", "220",
            "-y", "50",
            first_cmd.as_str(),
        ])
        .status()
        .expect("Failed to create tmux session");

    // añade el resto de hosts como nuevos panes
    for (host, network, defaults) in hosts.iter().skip(1) {
        let cmd = ssh_command(host, network, defaults);

        Command::new("tmux")
            .args(["split-window", "-t", session_name, "-v", cmd.as_str()])
            .status()
            .ok();

        Command::new("tmux")
            .args(["select-layout", "-t", session_name, "tiled"])
            .status()
            .ok();
    }

    // activa synchronize-panes
    Command::new("tmux")
        .args(["set-window-option", "-t", session_name, "synchronize-panes", "on"])
        .status()
        .ok();

    println!("Launched {} hosts in tmux session '{}'", hosts.len(), session_name);
    println!("Attaching... (Ctrl+b d to detach)");
    println!("Tip: Ctrl+b : set synchronize-panes off  → unsync panes");

    // attach
    Command::new("tmux")
        .args(["attach-session", "-t", session_name])
        .status()
        .expect("Failed to attach to tmux session");
}

fn ssh_command(host: &Host, network: &Network, defaults: &Defaults) -> String {
    let username = resolve_username(host, network, defaults);
    let port = resolve_port(host, network, defaults);
    format!("ssh -p {} {}@{}", port, username, host.ip)
}
