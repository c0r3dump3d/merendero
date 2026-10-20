# Changelog

All notable changes to Merendero are documented here.

## [Unreleased]

### Added
- Local audit log (`~/.config/merendero/audit.jsonl`): SSH sessions, Run cmd, SCP, SCP broadcast and broadcast, with start and end records
- AI fleet summary after a batch health check: groups flagged hosts by shared problem using only the local findings (no raw output); each request is recorded in the audit log
- Health history (`health_history.jsonl`, 90 days): each check is recorded and the host modal shows what changed since the previous check
- Run cmd: describe what you want and the AI proposes a command (only your description is sent). A local check classifies every command; anything not provably read-only needs you to type the host count to confirm
- Health check on a selection of hosts, with limited concurrency (8 at a time) and a single batched save
- AI health report now includes diagnostic and fix commands for each issue, adapted to the host's distribution, with a Regenerate button
- AI analysis can be requested from a saved report (runs a fresh check first)
- CIDR validation in the network modal: prefix required, warnings for /32 and very large ranges
- `scan_network` and `recalibrate_network` return a visible error for an invalid CIDR

### Fixed
- Broadcast now really launches only on hosts that passed the pre-flight check (v0.1.2 showed the check but launched on all selected hosts)
- Load average parsing on hosts with comma decimal locale (e.g. `0,11`)
- dmesg error count no longer matches unrelated text such as process names
- AI prompt truncated the health output at 4000 bytes, dropping the disk section and risking a panic on multibyte characters
- Rescanning a network no longer erases a host's hostname when the reverse DNS lookup fails
- Calibration errors are surfaced instead of being swallowed
- Error dialogs were never displayed because the `dialog:allow-message` permission was missing

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
