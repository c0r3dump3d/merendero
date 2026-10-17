use tokio::net::TcpStream;
use std::time::Duration;
use ipnetwork::IpNetwork;
use std::str::FromStr;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use tokio::sync::Semaphore;
use hickory_resolver::TokioAsyncResolver;
use hickory_resolver::config::{ResolverConfig, ResolverOpts, NameServerConfig, Protocol};
use tauri::Emitter;
use serde::Serialize;
use crate::host::{Host, Network, Defaults};

const MAX_CONCURRENT: usize = 200;

#[derive(Serialize, Clone)]
pub struct ScanProgress {
    pub total: usize,
    pub current: usize,
    pub ip: Option<String>,
    pub hostname: Option<String>,
    pub found: bool,
}

async fn resolve_hostname(ip: &str, dns_server: Option<&str>) -> Option<String> {
    let ip_addr = match IpAddr::from_str(ip) {
        Ok(addr) => addr,
        Err(_) => return None,
    };

    let resolver = match dns_server {
        Some(server) => {
            let server_addr: SocketAddr = match format!("{}:53", server).parse() {
                Ok(addr) => addr,
                Err(_) => return None,
            };
            let mut config = ResolverConfig::new();
            config.add_name_server(NameServerConfig {
                socket_addr: server_addr,
                protocol: Protocol::Udp,
                tls_dns_name: None,
                trust_negative_responses: false,
                bind_addr: None,
            });
            TokioAsyncResolver::tokio(config, ResolverOpts::default())
        }
        None => TokioAsyncResolver::tokio(ResolverConfig::default(), ResolverOpts::default()),
    };

    match resolver.reverse_lookup(ip_addr).await {
        Ok(lookup) => {
            lookup.iter().next().map(|name| {
                name.to_string().trim_end_matches('.').to_string()
            })
        }
        Err(_) => None,
    }
}

pub async fn scan_network_with_progress(
    network: &Network,
    defaults: &Defaults,
    window: tauri::Window,
    timeout_secs: u64,
) -> Vec<Host> {
    let port = network.port.unwrap_or(defaults.port);
    let dns_server = defaults.dns_server.clone();

    let cidr = match IpNetwork::from_str(&network.network) {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    let ips: Vec<String> = cidr.iter()
        .map(|ip| ip.to_string())
        .collect();

    let total = ips.len();
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT));

    let _ = window.emit("scan-progress", ScanProgress {
        total, current: 0, ip: None, hostname: None, found: false,
    });

    let mut tasks = vec![];

    for ip in &ips {
        let ip = ip.clone();
        let sem = semaphore.clone();
        let task = tokio::spawn(async move {
            let _permit = sem.acquire().await.unwrap();
            let address = format!("{}:{}", ip, port);
            let reachable = match tokio::time::timeout(
                Duration::from_secs(timeout_secs),
                TcpStream::connect(&address)
            ).await {
                Ok(Ok(_)) => true,
                _ => false,
            };
            (ip, reachable)
        });
        tasks.push(task);
    }

    let mut discovered = vec![];
    let mut current = 0;

    for task in tasks {
        current += 1;
        if let Ok((ip, true)) = task.await {
            let hostname = resolve_hostname(&ip, dns_server.as_deref()).await;
            let _ = window.emit("scan-progress", ScanProgress {
                total, current,
                ip: Some(ip.clone()),
                hostname: hostname.clone(),
                found: true,
            });
            discovered.push(Host {
                ip, hostname, username: None, port: None, tag: None, notes: None, identity_file: None, post_connect: None, health_status: None, health_report: None, active: true,
            });
        } else {
            let _ = window.emit("scan-progress", ScanProgress {
                total, current, ip: None, hostname: None, found: false,
            });
        }
    }

    discovered
}
