use tokio::net::TcpStream;
use std::time::{Duration, Instant};
use ipnetwork::IpNetwork;
use std::str::FromStr;
use std::net::{IpAddr, SocketAddr};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use tokio::sync::Semaphore;
use hickory_resolver::TokioAsyncResolver;
use hickory_resolver::config::{ResolverConfig, ResolverOpts, NameServerConfig, Protocol};
use hickory_resolver::error::ResolveErrorKind;
use tauri::Emitter;
use serde::Serialize;
use crate::host::{Host, Network, Defaults};

// ===== core: begin =====
/// How many connections a scan (or a check) keeps open at the same time.
pub const MAX_CONCURRENT: usize = 200;
/// Progress reaches the interface at most this often, however fast the scan goes.
const PROGRESS_EVERY: Duration = Duration::from_millis(150);
/// Name lookups: how many at once, how long to wait for each one, and how many silent answers in a row
/// make us stop asking (a missing record is an answer; only "no reply at all" counts).
const NAME_LOOKUPS_AT_ONCE: usize = 16;
const NAME_WAIT_SECS: u64 = 2;
const NAME_GIVE_UP_AFTER: u32 = 4;

#[derive(Serialize, Clone, Debug)]
pub struct ScanProgress {
    pub total: usize,
    pub current: usize,
    pub ip: Option<String>,
    pub hostname: Option<String>,
    pub found: bool,
    /// "scan" while probing addresses, "names" while looking up the names of the hosts found.
    pub phase: &'static str,
}

pub struct RangeScan {
    pub found: Vec<String>,
    pub scanned: usize,
    pub total: usize,
    pub cancelled: bool,
}

fn scan_event(total: usize, current: usize, found_ip: Option<IpAddr>) -> ScanProgress {
    ScanProgress {
        total,
        current,
        ip: found_ip.map(|i| i.to_string()),
        hostname: None,
        found: found_ip.is_some(),
        phase: "scan",
    }
}

/// Probes every address of `cidr` with at most MAX_CONCURRENT connections open at once.
/// Memory does not grow with the size of the range, progress is reported at most every PROGRESS_EVERY
/// (carrying the latest host found), and setting `cancel` stops it within a fraction of a second.
pub async fn scan_range<F>(
    cidr: IpNetwork,
    port: u16,
    timeout_secs: u64,
    cancel: Arc<AtomicBool>,
    progress: F,
) -> RangeScan
where
    F: Fn(ScanProgress) + Send + Sync + 'static,
{
    let total = match cidr {
        IpNetwork::V4(n) => (1u64 << (32 - u32::from(n.prefix()))) as usize,
        IpNetwork::V6(_) => return RangeScan { found: vec![], scanned: 0, total: 0, cancelled: false },
    };

    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT));
    let (tx, mut rx) = tokio::sync::mpsc::channel::<(IpAddr, bool)>(MAX_CONCURRENT * 2);

    let stop = cancel.clone();
    let producer = tokio::spawn(async move {
        for ip in cidr.iter() {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            // Wait for a free slot BEFORE creating the task: at most MAX_CONCURRENT tasks exist at any time.
            let permit = match semaphore.clone().acquire_owned().await {
                Ok(p) => p,
                Err(_) => break,
            };
            let tx = tx.clone();
            tokio::spawn(async move {
                let reachable = matches!(
                    tokio::time::timeout(
                        Duration::from_secs(timeout_secs),
                        TcpStream::connect(SocketAddr::new(ip, port)),
                    )
                    .await,
                    Ok(Ok(_))
                );
                let _ = tx.send((ip, reachable)).await;
                drop(permit);
            });
        }
    });

    let mut found: Vec<IpAddr> = Vec::new();
    let mut pending_found: Option<IpAddr> = None;
    let mut scanned = 0usize;
    let mut cancelled = false;
    let mut last = Instant::now();
    progress(scan_event(total, 0, None));
    loop {
        match tokio::time::timeout(Duration::from_millis(100), rx.recv()).await {
            Ok(Some((ip, reachable))) => {
                scanned += 1;
                if reachable {
                    found.push(ip);
                    pending_found = Some(ip);
                }
                if last.elapsed() >= PROGRESS_EVERY {
                    progress(scan_event(total, scanned, pending_found.take()));
                    last = Instant::now();
                }
            }
            Ok(None) => break,
            Err(_) => {}
        }
        if cancel.load(Ordering::Relaxed) {
            cancelled = true;
            break;
        }
    }
    // Connections still in flight finish by themselves (at most `timeout_secs`); nobody waits for them.
    producer.abort();
    found.sort();
    progress(scan_event(total, scanned, pending_found.take()));
    // Anything short of the whole range is a partial scan, whatever made it stop: the caller must never
    // conclude from it that a known host is gone.
    let cancelled = cancelled || scanned < total;

    RangeScan {
        found: found.iter().map(|ip| ip.to_string()).collect(),
        scanned,
        total,
        cancelled,
    }
}

fn names_event(total: usize, current: usize, latest: Option<&(String, String)>) -> ScanProgress {
    ScanProgress {
        total,
        current,
        ip: latest.map(|l| l.0.clone()),
        hostname: latest.map(|l| l.1.clone()),
        found: latest.is_some(),
        phase: "names",
    }
}

fn build_resolver(dns_server: Option<&str>) -> Option<TokioAsyncResolver> {
    let mut opts = ResolverOpts::default();
    opts.timeout = Duration::from_secs(NAME_WAIT_SECS);
    opts.attempts = 1;
    let config = match dns_server {
        Some(server) => {
            let server_addr: SocketAddr = format!("{}:53", server).parse().ok()?;
            let mut config = ResolverConfig::new();
            config.add_name_server(NameServerConfig {
                socket_addr: server_addr,
                protocol: Protocol::Udp,
                tls_dns_name: None,
                trust_negative_responses: false,
                bind_addr: None,
            });
            config
        }
        // No DNS server configured: use the system resolver (/etc/resolv.conf).
        // Never fall back to a public resolver, which would receive the internal IPs being scanned.
        None => hickory_resolver::system_conf::read_system_conf().ok()?.0,
    };
    Some(TokioAsyncResolver::tokio(config, opts))
}

/// Looks up the names of the hosts found. One resolver for the whole scan, a few lookups at a time, a short
/// wait for each, and it stops asking when the DNS server does not answer (second value: it gave up).
/// A host without a name is normal and never counts as the server being silent.
pub async fn resolve_names<F>(
    ips: Vec<String>,
    dns_server: Option<String>,
    cancel: Arc<AtomicBool>,
    progress: F,
) -> (HashMap<String, String>, bool)
where
    F: Fn(ScanProgress) + Send + Sync + 'static,
{
    let total = ips.len();
    if total == 0 {
        return (HashMap::new(), false);
    }
    let resolver = match build_resolver(dns_server.as_deref()) {
        Some(r) => Arc::new(r),
        None => return (HashMap::new(), false),
    };

    let permits = Arc::new(Semaphore::new(NAME_LOOKUPS_AT_ONCE));
    let silent = Arc::new(AtomicU32::new(0));
    let gave_up = Arc::new(AtomicBool::new(false));
    let done = Arc::new(AtomicUsize::new(0));

    let mut tasks = Vec::with_capacity(total);
    for ip in ips {
        let resolver = resolver.clone();
        let permits = permits.clone();
        let silent = silent.clone();
        let gave_up = gave_up.clone();
        let done = done.clone();
        let cancel = cancel.clone();
        tasks.push(tokio::spawn(async move {
            let _permit = permits.acquire().await.ok()?;
            if gave_up.load(Ordering::Relaxed) || cancel.load(Ordering::Relaxed) {
                return None;
            }
            let addr = IpAddr::from_str(&ip).ok()?;
            let answer = resolver.reverse_lookup(addr).await;
            done.fetch_add(1, Ordering::Relaxed);
            match answer {
                Ok(lookup) => {
                    silent.store(0, Ordering::Relaxed);
                    lookup
                        .iter()
                        .next()
                        .map(|n| (ip, n.to_string().trim_end_matches('.').to_string()))
                }
                Err(e) => {
                    if matches!(e.kind(), ResolveErrorKind::NoRecordsFound { .. }) {
                        silent.store(0, Ordering::Relaxed);
                    } else if silent.fetch_add(1, Ordering::Relaxed) + 1 >= NAME_GIVE_UP_AFTER {
                        gave_up.store(true, Ordering::Relaxed);
                    }
                    None
                }
            }
        }));
    }

    let mut names = HashMap::new();
    let mut latest: Option<(String, String)> = None;
    let mut last = Instant::now();
    progress(names_event(total, 0, None));
    for task in tasks {
        if let Ok(Some((ip, name))) = task.await {
            latest = Some((ip.clone(), name.clone()));
            names.insert(ip, name);
        }
        if last.elapsed() >= PROGRESS_EVERY {
            progress(names_event(total, done.load(Ordering::Relaxed), latest.as_ref()));
            last = Instant::now();
        }
    }
    progress(names_event(total, total, latest.as_ref()));
    (names, gave_up.load(Ordering::Relaxed))
}
// ===== core: end =====

pub struct ScanOutcome {
    pub hosts: Vec<Host>,
    pub scanned: usize,
    pub total: usize,
    pub cancelled: bool,
    pub dns_unresponsive: bool,
}

pub async fn scan_network_with_progress(
    network: &Network,
    defaults: &Defaults,
    window: tauri::Window,
    timeout_secs: u64,
    cancel: Arc<AtomicBool>,
) -> ScanOutcome {
    let port = network.port.unwrap_or(defaults.port);
    let cidr = match IpNetwork::from_str(&network.network) {
        Ok(c) => c,
        Err(_) => {
            return ScanOutcome { hosts: vec![], scanned: 0, total: 0, cancelled: false, dns_unresponsive: false };
        }
    };

    let w = window.clone();
    let range = scan_range(cidr, port, timeout_secs, cancel.clone(), move |p| {
        let _ = w.emit("scan-progress", p);
    })
    .await;

    // A cancelled scan stops there: the hosts found are kept, but nobody waits for their names.
    let (names, dns_unresponsive) = if range.cancelled {
        (HashMap::new(), false)
    } else {
        let w = window.clone();
        resolve_names(range.found.clone(), defaults.dns_server.clone(), cancel.clone(), move |p| {
            let _ = w.emit("scan-progress", p);
        })
        .await
    };

    let hosts = range
        .found
        .into_iter()
        .map(|ip| Host {
            hostname: names.get(&ip).cloned(),
            ssh_banner: None,  // this host was just discovered; it gets its banner on the next Check
            ip,
            username: None,
            port: None,
            tag: None,
            notes: None,
            identity_file: None,
            post_connect: None,
            health_status: None,
            health_report: None,
            active: true,
        })
        .collect();

    ScanOutcome { hosts, scanned: range.scanned, total: range.total, cancelled: range.cancelled, dns_unresponsive }
}


/// Reads the SSH banner line from a TCP stream that has JUST connected, without sending anything: the
/// server sends it unprompted, as the very first thing, before any authentication (RFC 4253). A few
/// servers send a plain warning line or two first; up to 2 of those are skipped looking for the real
/// "SSH-..." line. Reads at most 1024 bytes total (far more than the 255-byte line the RFC allows), so
/// a server that floods data without a newline can't make this hang on to memory. Any failure (timeout,
/// connection closed, a non-SSH service answering on that port) returns None: the host's stored banner,
/// if it already had one, is left as it was.
pub async fn read_ssh_banner(stream: &mut TcpStream, timeout_secs: u64) -> Option<String> {
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
    let mut reader = BufReader::new(stream.take(1024));
    let mut line = String::new();
    for _ in 0..3 {
        line.clear();
        let n = tokio::time::timeout(Duration::from_secs(timeout_secs), reader.read_line(&mut line)).await.ok()?.ok()?;
        if n == 0 {
            return None;
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.starts_with("SSH-") {
            return Some(trimmed.to_string());
        }
    }
    None
}
