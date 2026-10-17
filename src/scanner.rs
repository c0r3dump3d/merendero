use tokio::net::TcpStream;
use std::time::Duration;
use ipnetwork::IpNetwork;
use std::str::FromStr;
use std::net::{IpAddr, SocketAddr};
use indicatif::{ProgressBar, ProgressStyle};
use hickory_resolver::TokioAsyncResolver;
use hickory_resolver::config::{ResolverConfig, ResolverOpts, NameServerConfig, Protocol};
use crate::host::{Host, Network, Defaults, resolve_port};

pub async fn check_host(host: Host, network: &Network, defaults: &Defaults) -> (Host, bool) {
    let port = resolve_port(&host, network, defaults);
    let address = format!("{}:{}", host.ip, port);
    let timeout = Duration::from_secs(3);

    let reachable = match tokio::time::timeout(timeout, TcpStream::connect(&address)).await {
        Ok(Ok(_)) => true,
        _ => false,
    };

    (host, reachable)
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

pub async fn scan_network(network: &Network, defaults: &Defaults) -> Vec<Host> {
    let port = network.port.unwrap_or(defaults.port);
    let dns_server = defaults.dns_server.clone();

    let cidr = match IpNetwork::from_str(&network.network) {
        Ok(c) => c,
        Err(_) => {
            println!("Invalid network CIDR: {}", network.network);
            return vec![];
        }
    };

    let ips: Vec<String> = cidr.iter()
        .map(|ip| ip.to_string())
        .collect();

    println!("Scanning {} hosts on port {}...", ips.len(), port);
    if let Some(ref dns) = dns_server {
        println!("Using DNS server: {}", dns);
    }

    let pb = ProgressBar::new(ips.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("#>-")
    );

    let mut tasks = vec![];

    for ip in &ips {
        let ip = ip.clone();
        let task = tokio::spawn(async move {
            let address = format!("{}:{}", ip, port);
            let timeout = Duration::from_secs(3);

            let reachable = match tokio::time::timeout(timeout, TcpStream::connect(&address)).await {
                Ok(Ok(_)) => true,
                _ => false,
            };

            (ip, reachable)
        });
        tasks.push(task);
    }

    let mut discovered = vec![];

    for task in tasks {
        pb.inc(1);
        if let Ok((ip, true)) = task.await {
            let hostname = resolve_hostname(&ip, dns_server.as_deref()).await;
            pb.println(format!("  ✓ {} | fqdn: {}",
                ip,
                hostname.clone().unwrap_or(String::from("(none)"))
            ));
            discovered.push(Host {
                ip,
                hostname,
                username: None,
                port: None,
                tag: None,
                active: true,
            });
        }
    }

    pb.finish_with_message("Scan complete");
    discovered
}
