# Changelog

All notable changes to Merendero are documented here.

## [Unreleased]

## [0.1.5] - 2026-10-23

Thanks to [@jvir](https://github.com/jvir) and [@jabanos](https://github.com/jabanos), who took the time to test Merendero and found the problems fixed in this release.

### Added
- Terminal choice: Merendero detects the terminals installed on your system and lists them in Settings, with Automatic (the default: `terminator` first, then your desktop's own terminal) and Other… for a name or a full path. If you have not chosen one and have two or more installed, it asks once, the first time. If none is installed, the error says so. It knows how to start `terminator`, `alacritty`, `kitty`, `foot`, `gnome-terminal`, `mate-terminal`, `xfce4-terminal`, `tilix`, `konsole`, `qterminal`, `xterm`, `lxterminal`, `urxvt` and `sakura`; any other one is started as `<terminal> -e "<command>"`
- Browse button for the SSH key in the network, host and Settings forms. It opens in `~/.ssh`, stores the path as `~/…`, and if you pick a `.pub` file it uses the private key next to it
- Big networks: a scan of up to a `/20` runs as before; from a `/19` to a `/12` it shows how long it can take in the worst case and asks first; a range larger than a `/12` is refused (in the network form, when you press Scan, and in the backend). While a scan runs there is a **✕ Cancel** button that keeps what was found, and the footer sums up the result
- The selection toolbar and the footer show how many selected hosts a filter is hiding, for example `3 hosts selected (2 hidden)`
- Backup export has an *Include the AI API key* checkbox, off by default

### Changed
- Broadcast opens in the terminal you configured (it always used `terminator` before) and starts maximized from 4 hosts up in the terminals that can do it. If tmux can't fit that many panes you get an error, instead of a silent failure
- Merendero looks for `ssh`, `scp`, `tmux` and the terminals in your `PATH`, plus `~/.local/bin`, `~/.cargo/bin` and the Nix, Guix and snap directories, instead of fixed `/usr/bin` paths
- Merendero itself writes the backup, not the browser: the file goes to your Downloads folder (or your home), is created readable only by you, never overwrites an earlier backup (`-2`, `-3`…), and the message shows the full path. The AI API key is left out by default, and restoring a backup that has no key keeps the key you already have saved
- Network form: `/15` and larger no longer ask for a second click. A line under the field says how many addresses the range has and how long a scan can take

### Fixed
- Broadcast ignored your SSH keys: it now uses the host's key, then the network's, then the global one
- SSH keys and post-connect commands with spaces or quotes in them were not quoted correctly
- The broadcast window was too small with many hosts
- Scanning a big network could freeze the window or leave it blank. The scan built the whole list of addresses in memory (about 700 bytes each: 47 MB for a `/16`, 700 MB for a `/12`, an estimated 11 GB for a `/8`) and sent one progress message per address (about 11,000 in 100 milliseconds for a `/16`). It now uses constant memory, sends about 7 messages a second and can be cancelled
- A scan looked frozen when the DNS server did not answer: each host found waited up to 15 seconds for its name, one after another. Names are now looked up 16 at a time with a 2 second wait, and Merendero stops asking, and tells you, when the server does not answer
- A scan overwrote what you edited while it ran. The results are now applied to the file as it is on disk
- A cancelled or partial scan no longer marks the known hosts it did not reach as down
- The availability check opened a connection to every known host at once. On a network with thousands of hosts it could run out of file descriptors, and a host it could not even try was recorded as down. It now uses at most 200 connections at a time and leaves a host's state alone when it can't tell
- "Select all" compared the number of selected hosts with the number of visible ones, so with hosts hidden by a filter it could show as checked and clear the whole selection. It now looks at which hosts are selected
- Files that already existed with wider permissions, such as a `config.toml` created as `644`, are now restricted to the owner when Merendero writes them
- The network name field no longer suggests an internal name as its placeholder

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
