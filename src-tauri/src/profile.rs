use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, PartialEq, Debug)]
pub struct ProfileCommand {
    pub label: String,
    pub command: String,
    /// The output changes on every run (top processes, free memory...): it is not compared with the
    /// previous run. Not written to profiles.toml when false, and a file without it still loads.
    #[serde(default, skip_serializing_if = "is_false")]
    pub volatile: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Deserialize, Serialize, Clone, PartialEq, Debug)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub commands: Vec<ProfileCommand>,
}

#[derive(Deserialize, Serialize, Clone, Default)]
pub struct ProfilesFile {
    pub profiles: Vec<Profile>,
}

fn pc(label: &str, command: &str) -> ProfileCommand {
    ProfileCommand { label: label.to_string(), command: command.to_string(), volatile: false }
}

/// Same, for a command whose output changes on every run.
fn pv(label: &str, command: &str) -> ProfileCommand {
    ProfileCommand { label: label.to_string(), command: command.to_string(), volatile: true }
}

/// Always computed in code, never written to disk: a new stock profile added in a future version
/// reaches everyone on update, with nothing to migrate.
pub fn stock_profiles() -> Vec<Profile> {
    vec![
        Profile {
            id: "stock-hardening".into(),
            name: "Basic hardening".into(),
            commands: vec![
                pc("SSH: effective options", "sshd -T 2>/dev/null | grep -E '^(permitrootlogin|passwordauthentication|permitemptypasswords|x11forwarding|maxauthtries)'"),
                pc("Users with UID 0", "awk -F: '$3 == 0 {print $1}' /etc/passwd"),
                pc("Passwordless sudo rules", "grep -rn NOPASSWD /etc/sudoers /etc/sudoers.d/ 2>/dev/null"),
                pc("Listening ports", "ss -tulnp 2>/dev/null"),
                pc("Firewall status", "systemctl is-active firewalld nftables ufw 2>/dev/null; nft list ruleset 2>/dev/null | head -80; if grep -qx filter /proc/net/ip_tables_names 2>/dev/null; then iptables -S 2>/dev/null | head -20; fi"),
                pc("SUID/SGID binaries", "find / -xdev \\( -perm -4000 -o -perm -2000 \\) -type f 2>/dev/null"),
                pc("Services enabled at boot", "systemctl list-unit-files --state=enabled --type=service 2>/dev/null"),
                pc("Members of wheel and sudo", "getent group wheel sudo 2>/dev/null"),
                pc("Audit subsystem and rules", "systemctl is-active auditd 2>/dev/null; if command -v auditctl >/dev/null 2>&1; then auditctl -s; auditctl -l | head -n 30; else echo 'auditctl not installed'; fi"),
                pc("SELinux or AppArmor", "if command -v getenforce >/dev/null 2>&1; then getenforce; elif [ -d /sys/kernel/security/apparmor ]; then echo apparmor present; cat /sys/module/apparmor/parameters/enabled; else echo 'no SELinux or AppArmor found'; fi"),
                pc("Accounts without a password", "if [ -r /etc/shadow ]; then awk -F: 'length($2) == 0 {print $1}' /etc/shadow | grep . || echo none; else echo 'cannot read /etc/shadow (needs root)'; fi"),
                pc("Network sysctl settings", "sysctl net.ipv4.ip_forward net.ipv4.conf.all.rp_filter net.ipv4.conf.all.accept_redirects net.ipv4.conf.all.send_redirects net.ipv4.conf.all.accept_source_route net.ipv4.tcp_syncookies"),
                pc("Kernel and last boot", "uname -r; who -b"),
            ],
        },
        Profile {
            id: "stock-ioc".into(),
            name: "Indicators of compromise".into(),
            commands: vec![
                pc("Cron: per-user and system jobs", "for u in $(cut -d: -f1 /etc/passwd); do crontab -u \"$u\" -l 2>/dev/null | sed \"s/^/[$u] /\"; done; echo '--- /etc/crontab, cron.d, periodic ---'; cat /etc/crontab 2>/dev/null; cat /etc/cron.d/* 2>/dev/null; ls -la /etc/cron.hourly /etc/cron.daily /etc/cron.weekly /etc/cron.monthly 2>/dev/null"),
                pc("Cron: systemd timers", "systemctl list-timers --all --no-pager --no-legend 2>/dev/null | awk '{print $(NF-1), $NF}' | sort"),
                pc("Zombie processes (state Z)", "ps -eo pid,ppid,stat,comm | awk '$3 ~ /Z/'"),
                pc("Processes running from a deleted binary", "for p in /proc/[0-9]*; do pid=${p#/proc/}; exe=$(readlink \"$p/exe\" 2>/dev/null); case \"$exe\" in *\"(deleted)\") printf 'PID %s: %s  CMD: %s\\n' \"$pid\" \"$exe\" \"$(tr '\\0' ' ' < $p/cmdline 2>/dev/null)\";; esac; done"),
                pc("ld.so.preload", "if [ -e /etc/ld.so.preload ]; then ls -l /etc/ld.so.preload; cat /etc/ld.so.preload; else echo 'no /etc/ld.so.preload'; fi"),
                pc("Kernel taint and modules", "cat /proc/sys/kernel/tainted; lsmod | awk 'NR > 1 {print $1}' | sort"),
                pc("Executables in temp dirs", "echo executables: $(find /tmp /var/tmp /dev/shm -xdev -type f -perm -u+x 2>/dev/null | wc -l); find /tmp /var/tmp /dev/shm -xdev -type f -perm -u+x -exec ls -l {} + | head -n 200"),
                pv("Established connections", "ss -tnp | head -n 100"),
                pc("authorized_keys files", "find /root /home -maxdepth 3 -name authorized_keys -type f -exec ls -l {} + | grep . || echo 'no authorized_keys files found'"),
                pc("Recently modified startup files (7 days)", "find /etc/systemd/system /etc/init.d /etc/cron.d /etc/profile.d /etc/rc.local -xdev -type f -mtime -7 -exec ls -l {} + 2>/dev/null | grep . || echo 'none modified in the last 7 days'"),
            ],
        },
        Profile {
            id: "stock-performance".into(),
            name: "Basic performance".into(),
            commands: vec![
                pv("Load and CPUs", "uptime; nproc 2>/dev/null"),
                pv("Memory and swap", "free -m"),
                pv("Disk space", "timeout 15 df -hP; rc=$?; [ $rc -eq 124 ] && echo 'df timed out after 15 s: a mount may be stale'; exit 0"),
                pv("Inodes", "timeout 15 df -iP; rc=$?; [ $rc -eq 124 ] && echo 'df timed out after 15 s: a mount may be stale'; exit 0"),
                pv("Top processes by CPU", "ps -eo pid,user,pcpu,pmem,stat,etime,comm --sort=-pcpu | head -n 11"),
                pv("Top processes by memory", "ps -eo pid,user,rss,pmem,stat,etime,comm --sort=-rss | head -n 11"),
                pv("Processes in D state", "ps -eo pid,user,stat,wchan:20,comm | awk 'NR == 1 || $3 ~ /^D/'"),
                pc("Kernel errors and OOM kills", "dmesg | grep -iE 'out of memory|oom-kill|killed process|i/o error|blocked for more than|hung task' | tail -n 20"),
                pv("Pressure stall info", "grep . /proc/pressure/cpu /proc/pressure/memory /proc/pressure/io 2>/dev/null || echo 'no /proc/pressure (kernel without PSI)'"),
                pv("Network summary", "ss -s; cat /proc/net/dev"),
                pc("Network and cluster mounts", "grep -E ' (nfs4?|gpfs|cifs|ceph|lustre|fuse([.][a-z0-9_-]+)?) ' /proc/mounts || echo 'no network or cluster mounts'"),
                pc("Failed systemd units", "systemctl --failed --no-pager --plain"),
            ],
        },
    ]
}

pub fn load_custom_profiles(path: &str) -> Result<ProfilesFile, String> {
    if !std::path::Path::new(path).exists() {
        return Ok(ProfilesFile::default());
    }
    let text = std::fs::read_to_string(path).map_err(|e| format!("Could not read {}: {}", path, e))?;
    toml::from_str(&text).map_err(|e| format!("Could not parse {}: {}", path, e))
}

pub fn save_custom_profiles(path: &str, file: &ProfilesFile) -> Result<(), String> {
    let text = toml::to_string_pretty(file).map_err(|e| format!("Could not build profiles.toml: {}", e))?;
    std::fs::write(path, text).map_err(|e| format!("Could not save {}: {}", path, e))
}

/// Stock profiles first, then whatever the user has saved.
pub fn all_profiles(custom: &ProfilesFile) -> Vec<Profile> {
    let mut out = stock_profiles();
    out.extend(custom.profiles.iter().cloned());
    out
}
