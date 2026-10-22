# Changelog

All notable changes to Merendero are documented here.

## [Unreleased]

## [0.1.4] - 2026-10-22

### Added
- Automatic availability check: every 15 minutes by default, only while Merendero is open (Settings → Monitoring; `0` turns it off). Each run opens a TCP connection to the SSH port of every known host. It never runs while a dialog is open or another operation is in progress, it shows its state in the footer, and a Pause button stops it until you close Merendero
- In an automatic check, if every host of a network stops answering at once, the previous state is kept (the likely cause is that this machine lost its connection). The ✓ Check button still applies what it finds
- Health history retention is configurable in Settings (90 days by default, `0` keeps everything)
- The running version is shown in the sidebar

### Fixed
- Checking hosts no longer overwrites edits made while the check is running, and no longer overwrites a `networks.toml` that cannot be read: the results are applied to the file as it is on disk
- After a successful broadcast the selected hosts are deselected, as they already were when you changed network. If the broadcast fails, the selection is kept so you can retry

## [0.1.3] - 2026-10-22

### Added
- Local audit log (`~/.config/merendero/audit.jsonl`): SSH sessions, Run cmd, SCP, SCP broadcast and broadcast, with start and end records
- AI fleet summary after a batch health check: groups flagged hosts by shared problem using only the local findings (no raw output); each request is recorded in the audit log
- Health history (`health_history.jsonl`, 90 days): each check is recorded and the host modal shows what changed since the previous check
- Run cmd: describe what you want and the AI proposes a command (only your description is sent). A local check classifies every command; anything not provably read-only needs you to type the host count to confirm
- Run cmd shows an execution receipt (command, hosts OK/failed, how it was approved, time); approved commands are recorded in the audit log
- Export backup now includes the audit log and the health history. On import they are restored only when the local file is empty; otherwise the imported copy is saved next to it
- AI features work with Ollama and custom endpoints without an API key (tested against a simulated server only, not yet against a real local model); `config.toml` is created with owner-only permissions
- Health check on a selection of hosts, with limited concurrency (8 at a time) and a single batched save
- AI health report now includes diagnostic and fix commands for each issue, adapted to the host's distribution, with a Regenerate button
- AI analysis can be requested from a saved report (runs a fresh check first)
- CIDR validation in the network modal: prefix required, warnings for /32 and very large ranges
- `scan_network` and `recalibrate_network` return a visible error for an invalid CIDR

### Fixed
- Broadcast now really launches only on hosts that passed the pre-flight check (v0.1.2 showed the check but launched on all selected hosts)
- On a first run, saving the first network failed because `~/.config/merendero/` did not exist, so a new user could not start. Merendero now creates the directory (owner-only) and starts with default settings (user `root`, port 22). A configuration file that exists but cannot be read now shows an error instead of an empty window. In 0.1.2 and earlier, the workaround is `mkdir -p ~/.config/merendero`
- Reverse DNS lookups during a scan no longer go to Google's public DNS when no DNS server is set in Settings. In 0.1.2 and earlier, the internal IPs found by a scan were sent to 8.8.8.8 and 8.8.4.4 in that case. Merendero now uses the system resolver (`/etc/resolv.conf`), and does no lookups if the system has none
- Load average parsing on hosts with comma decimal locale (e.g. `0,11`)
- dmesg error count no longer matches unrelated text such as process names
- AI prompt truncated the health output at 4000 bytes, dropping the disk section and risking a panic on multibyte characters
- Rescanning a network no longer erases a host's hostname when the reverse DNS lookup fails
- Calibration errors are surfaced instead of being swallowed
- Error dialogs were never displayed because the `dialog:allow-message` permission was missing

### Removed
- The legacy terminal CLI (`merendero-cli`). It rewrote `networks.toml` with only the fields it knew, so using it on a configuration managed by the GUI dropped notes, SSH keys, post-connect commands, health results and scan timeouts

## [0.1.2] - 2026-10-20

### Added
- Global search: the filter now searches across all networks, grouped by network, with an "Open network" button that keeps the filter applied
- Pre-flight reachability check before broadcast; unreachable hosts are skipped
- Clear button (✕) and Esc key to reset the filter

## [0.1.1]

### Added
- Confirmation modal before broadcast, listing the target hosts

### Changed
- Run cmd output expands inline per host

### Fixed
- Crash when creating a new network (missing `handleChange`)

## [0.1.0]

- First public release
