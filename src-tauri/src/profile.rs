use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, PartialEq, Debug)]
pub struct ProfileCommand {
    pub label: String,
    pub command: String,
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
    ProfileCommand { label: label.to_string(), command: command.to_string() }
}

/// Always computed in code, never written to disk: a new stock profile added in a future version
/// reaches everyone on update, with nothing to migrate.
pub fn stock_profiles() -> Vec<Profile> {
    vec![
        Profile {
            id: "stock-hardening".into(),
            name: "Bastionado básico".into(),
            commands: vec![
                pc("SSH: opciones efectivas", "sshd -T 2>/dev/null | grep -E '^(permitrootlogin|passwordauthentication|permitemptypasswords|x11forwarding|maxauthtries)'"),
                pc("Usuarios con UID 0", "awk -F: '$3 == 0 {print $1}' /etc/passwd"),
                pc("sudoers sin contraseña", "grep -rn NOPASSWD /etc/sudoers /etc/sudoers.d/ 2>/dev/null"),
                pc("Puertos en escucha", "ss -tulnp 2>/dev/null"),
                pc("Estado del firewall", "systemctl is-active firewalld ufw 2>/dev/null; iptables -L -n 2>/dev/null | head -20"),
                pc("Binarios con SUID/SGID", "find / -xdev \\( -perm -4000 -o -perm -2000 \\) -type f 2>/dev/null"),
                pc("Servicios habilitados al arranque", "systemctl list-unit-files --state=enabled --type=service 2>/dev/null"),
            ],
        },
        Profile {
            id: "stock-ioc".into(),
            name: "Indicios de compromiso".into(),
            commands: vec![
                pc("Cron: tareas por usuario y sistema", "for u in $(cut -d: -f1 /etc/passwd); do crontab -u \"$u\" -l 2>/dev/null | sed \"s/^/[$u] /\"; done; echo '--- /etc/crontab, cron.d, periodic ---'; cat /etc/crontab 2>/dev/null; cat /etc/cron.d/* 2>/dev/null; ls -la /etc/cron.hourly /etc/cron.daily /etc/cron.weekly /etc/cron.monthly 2>/dev/null"),
                pc("Cron: timers de systemd", "systemctl list-timers --all --no-pager --no-legend 2>/dev/null | awk '{print $(NF-1), $NF}' | sort"),
                pc("Procesos en estado Z", "ps -eo pid,ppid,stat,comm | awk '$3 ~ /Z/'"),
                pc("Procesos ejecutando desde un binario borrado", "for p in /proc/[0-9]*; do pid=${p#/proc/}; exe=$(readlink \"$p/exe\" 2>/dev/null); case \"$exe\" in *\"(deleted)\") printf 'PID %s: %s  CMD: %s\\n' \"$pid\" \"$exe\" \"$(tr '\\0' ' ' < $p/cmdline 2>/dev/null)\";; esac; done"),
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
