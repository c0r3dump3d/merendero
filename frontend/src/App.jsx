import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import ReactMarkdown from "react-markdown";
import { open, save } from "@tauri-apps/plugin-dialog";
import "./App.css";

function validateCidr(value) {
  if (!value) return "Network is required (example: 192.168.122.0/24)";
  if (value !== value.trim()) return "Remove leading/trailing spaces";
  const m = value.match(/^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})(?:\/(\d{1,2}))?$/);
  if (!m) return "Invalid format. Use IPv4 CIDR, for example 192.168.122.0/24";
  if (m[5] === undefined) return `Missing prefix length: "${value}" would be scanned as a single host. Use for example ${value}/24`;
  if ([m[1], m[2], m[3], m[4]].some(o => parseInt(o, 10) > 255)) return "Each octet must be between 0 and 255";
  if (parseInt(m[5], 10) > 32) return "Prefix length must be between 0 and 32";
  return null;
}

function NetworkModal({ network, index, onSave, onClose, onRecalibrate }) {
  const isEdit = network !== null;
  const [form, setForm] = useState({
    name: network?.name || "",
    network: network?.network || "",
    username: network?.username || "",
    port: network?.port?.toString() || "",
    scan_timeout: network?.scan_timeout?.toString() || "",
    identity_file: network?.identity_file || "",
  });
  const [calibrating, setCalibrating] = useState(false);
  const [netError, setNetError] = useState(null);
  const [netWarn, setNetWarn] = useState(null);

  function handleChange(e) { setForm({ ...form, [e.target.name]: e.target.value }); setNetError(null); setNetWarn(null); }

  async function handleRecalibrate() {
    if (!onRecalibrate) return;
    setCalibrating(true);
    try {
      const timeout = await onRecalibrate(index);
      setForm(f => ({ ...f, scan_timeout: timeout.toString() }));
    } catch(e) {
      alert("Calibration failed: " + e);
    } finally {
      setCalibrating(false);
    }
  }

  async function handleSubmit() {
    const cidrError = validateCidr(form.network);
    if (cidrError) { setNetError(cidrError); setNetWarn(null); return; }
    const prefix = parseInt(form.network.split("/")[1], 10);
    const warn = prefix === 32
      ? "A /32 network is a single host, so the scan will only check that one IP."
      : prefix < 16
        ? `/${prefix} has ${Math.pow(2, 32 - prefix).toLocaleString()} addresses. The scan opens one connection per address and will take very long.`
        : null;
    if (warn && netWarn !== warn) { setNetError(null); setNetWarn(warn); return; }
    setNetError(null);
    setNetWarn(null);
    if (!form.name || !form.network) { alert("Name and CIDR are required"); return; }
    await onSave({
      name: form.name, network: form.network,
      username: form.username || null,
      port: form.port ? parseInt(form.port) : null,
      scan_timeout: form.scan_timeout ? parseInt(form.scan_timeout) : null,
      identity_file: form.identity_file || null,
      hosts: network?.hosts || []
    }, index);
    onClose();
  }

  return (
    <div className="modal-overlay">
      <div className="modal">
        <h3>{isEdit ? "Edit Network" : "Add Network"}</h3>
        <div className="form-group"><label>Name</label><input name="name" value={form.name} onChange={handleChange} placeholder="PDC-PRO" /></div>
        <div className="form-group"><label>Network CIDR</label><input name="network" value={form.network} onChange={handleChange} placeholder="192.168.1.0/24" /></div>
        <div className="form-group"><label>Username (leave empty for default)</label><input name="username" value={form.username} onChange={handleChange} placeholder="root" /></div>
        <div className="form-group"><label>Port (leave empty for default)</label><input name="port" value={form.port} onChange={handleChange} placeholder="22" type="number" /></div>
        <div className="form-group">
          <label>Identity file (SSH key)</label>
          <input name="identity_file" value={form.identity_file} onChange={handleChange} placeholder="~/.ssh/id_rsa" />
        </div>
        <div className="form-group">
          <label>Scan timeout (seconds) {form.scan_timeout ? "" : "— not calibrated"}</label>
          <div className="form-row">
            <input name="scan_timeout" value={form.scan_timeout} onChange={handleChange} placeholder="auto" type="number" min="1" max="30" />
            {isEdit && (
              <button className="btn-calibrate" onClick={handleRecalibrate} disabled={calibrating}>
                {calibrating ? "Calibrating..." : "⟳ Calibrate"}
              </button>
            )}
          </div>
          {form.scan_timeout && <span className="timeout-info">RTT-based · editable</span>}
        </div>
        {netError && <div className="scp-result error">{netError}</div>}
        {netWarn && <div className="health-issue warning">⚠ {netWarn} Press {isEdit ? "Save" : "Add"} again to confirm.</div>}
        <div className="modal-actions">
          <button className="btn-cancel" onClick={onClose}>Cancel</button>
          <button className="btn-primary" onClick={handleSubmit}>{isEdit ? "Save" : "Add"}</button>
        </div>
      </div>
    </div>
  );
}

function HostModal({ host, networks, defaultNetworkIndex, onSave, onClose, onHealthCheck }) {
  const isEdit = !!host;
  const [form, setForm] = useState({
    ip: host?.ip || "", hostname: host?.hostname || "", tag: host?.tag || "",
    username: host?.username || "", port: host?.port?.toString() || "",
    notes: host?.notes || "",
    identity_file: host?.identity_file || "",
    post_connect: host?.post_connect || "",
    active: host?.active ?? true, networkIndex: defaultNetworkIndex ?? 0,
  });
  function handleChange(e) {
    const val = e.target.type === "checkbox" ? e.target.checked : e.target.value;
    setForm({ ...form, [e.target.name]: val });
  }
  async function handleSubmit() {
    if (!form.ip) { alert("IP is required"); return; }
    await onSave({
      hostData: { ip: form.ip, hostname: form.hostname || null, tag: form.tag || null, username: form.username || null, port: form.port ? parseInt(form.port) : null, notes: form.notes || null, identity_file: form.identity_file || null, post_connect: form.post_connect || null, active: form.active },
      networkIndex: parseInt(form.networkIndex),
    });
    onClose();
  }
  return (
    <div className="modal-overlay">
      <div className="modal">
        <h3>{isEdit ? "Edit Host" : "Add Host"}</h3>
        {!isEdit && (
          <div className="form-group">
            <label>Network</label>
            <select name="networkIndex" value={form.networkIndex} onChange={handleChange} className="form-select">
              {networks.map((net, i) => <option key={i} value={i}>{net.name} — {net.network}</option>)}
            </select>
          </div>
        )}
        <div className="form-group"><label>IP Address</label><input name="ip" value={form.ip} onChange={handleChange} placeholder="192.168.1.10" disabled={isEdit} /></div>
        <div className="form-group"><label>Hostname / FQDN</label><input name="hostname" value={form.hostname} onChange={handleChange} placeholder="server01.example.com" /></div>
        <div className="form-group"><label>Tag</label><input name="tag" value={form.tag} onChange={handleChange} placeholder="web, db, backup..." /></div>
        <div className="form-group"><label>Username (leave empty for network/default)</label><input name="username" value={form.username} onChange={handleChange} placeholder="root" /></div>
        <div className="form-group"><label>Port (leave empty for network/default)</label><input name="port" value={form.port} onChange={handleChange} placeholder="22" type="number" /></div>
        <div className="form-group">
          <label>Identity file (SSH key)</label>
          <input name="identity_file" value={form.identity_file} onChange={handleChange} placeholder="~/.ssh/id_rsa" />
        </div>
        <div className="form-group">
          <label>Post-connect command</label>
          <input name="post_connect" value={form.post_connect} onChange={handleChange} placeholder="sudo su -" />
        </div>
        <div className="form-group">
          <label>Notes</label>
          <textarea name="notes" value={form.notes} onChange={handleChange} placeholder="Anything worth remembering about this host..." className="form-textarea" rows={3} />
        </div>
        <div className="form-group form-group-inline"><label>Active</label><input name="active" type="checkbox" checked={form.active} onChange={handleChange} /></div>
        <div className="modal-actions">
          <button className="btn-cancel" onClick={onClose}>Cancel</button>
          {isEdit && onHealthCheck && <button className="btn-check" onClick={() => onHealthCheck(host)}>⚕ Health</button>}
          <button className="btn-primary" onClick={handleSubmit}>{isEdit ? "Save" : "Add"}</button>
        </div>
      </div>
    </div>
  );
}

function TagModal({ count, onApply, onClose }) {
  const [tag, setTag] = useState("");
  async function handleSubmit() { await onApply(tag); onClose(); }
  return (
    <div className="modal-overlay">
      <div className="modal modal-sm">
        <h3>Tag {count} host{count !== 1 ? "s" : ""}</h3>
        <div className="form-group">
          <label>Tag (leave empty to remove)</label>
          <input value={tag} onChange={e => setTag(e.target.value)} placeholder="web, db, backup..." autoFocus onKeyDown={e => e.key === "Enter" && handleSubmit()} />
        </div>
        <div className="modal-actions">
          <button className="btn-cancel" onClick={onClose}>Cancel</button>
          <button className="btn-primary" onClick={handleSubmit}>Apply</button>
        </div>
      </div>
    </div>
  );
}

function ProgressBar({ progress, label }) {
  if (!progress) return null;
  const pct = progress.total > 0 ? Math.round((progress.current / progress.total) * 100) : 0;
  return (
    <div className="scan-progress">
      <div className="scan-progress-bar"><div className="scan-progress-fill" style={{ width: `${pct}%` }} /></div>
      <div className="scan-progress-info">
        <span>{label} {progress.current}/{progress.total}</span>
        {progress.found && progress.ip && <span className="scan-found">✓ {progress.ip} {progress.hostname ? `(${progress.hostname})` : ""}</span>}
      </div>
    </div>
  );
}

function SettingsModal({ config, onSave, onClose }) {
  const [form, setForm] = useState({
    username: config?.defaults?.username || "root",
    port: config?.defaults?.port?.toString() || "22",
    dns_server: config?.defaults?.dns_server || "",
    terminal: config?.defaults?.terminal || "",
    identity_file: config?.defaults?.identity_file || "",
    ai_provider: config?.ai?.provider || "claude",
    ai_api_url: config?.ai?.api_url || "https://api.anthropic.com/v1/messages",
    ai_api_key: config?.ai?.api_key || "",
    ai_model: config?.ai?.model || "claude-haiku-4-5",
  });

  const [backupMsg, setBackupMsg] = useState(null);

  function handleChange(e) { setForm({ ...form, [e.target.name]: e.target.value }); }

  async function handleExport() {
    try {
      const backup = await invoke("export_backup");
      const blob = new Blob([JSON.stringify(backup, null, 2)], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      const date = new Date().toISOString().slice(0, 10);
      a.href = url;
      a.download = `merendero-backup-${date}.json`;
      a.click();
      URL.revokeObjectURL(url);
      setBackupMsg({ type: "success", text: `Backup saved as merendero-backup-${date}.json in your Downloads folder` });
    } catch(e) {
      setBackupMsg({ type: "error", text: `Export failed: ${e}` });
    }
  }

  async function handleImport(e) {
    const file = e.target.files[0];
    if (!file) return;
    try {
      const text = await file.text();
      const data = JSON.parse(text);
      if (!data.version || !data.config) {
        setBackupMsg({ type: "error", text: "Invalid backup file" });
        return;
      }
      await invoke("import_backup", { data });
      setBackupMsg({ type: "success", text: "Backup imported. Restart to apply changes." });
    } catch(e) {
      setBackupMsg({ type: "error", text: `Import failed: ${e}` });
    }
  }

  function handleChange(e) { setForm({ ...form, [e.target.name]: e.target.value }); }

  function handleProviderChange(e) {
    const provider = e.target.value;
    const urls = {
      claude: "https://api.anthropic.com/v1/messages",
      openai: "https://api.openai.com/v1/chat/completions",
      ollama: "http://localhost:11434/api/chat",
      custom: "",
    };
    const models = {
      claude: "claude-haiku-4-5",
      openai: "gpt-4o-mini",
      ollama: "llama3",
      custom: "",
    };
    setForm({ ...form, ai_provider: provider, ai_api_url: urls[provider] || "", ai_model: models[provider] || "" });
  }

  async function handleSubmit() {
    await onSave({
      defaults: {
        username: form.username,
        port: parseInt(form.port) || 22,
        dns_server: form.dns_server || null,
        terminal: form.terminal || null,
        identity_file: form.identity_file || null,
      },
      ai: form.ai_api_key ? {
        provider: form.ai_provider || null,
        api_url: form.ai_api_url || null,
        api_key: form.ai_api_key || null,
        model: form.ai_model || null,
      } : null,
    });
    onClose();
  }

  return (
    <div className="modal-overlay">
      <div className="modal settings-modal">
        <h3>⚙ Settings</h3>

        <div className="settings-section">SSH defaults</div>
        <div className="form-group"><label>Default username</label><input name="username" value={form.username} onChange={handleChange} placeholder="root" /></div>
        <div className="form-group"><label>Default port</label><input name="port" value={form.port} onChange={handleChange} placeholder="22" type="number" /></div>
        <div className="form-group"><label>DNS server (for reverse lookup)</label><input name="dns_server" value={form.dns_server} onChange={handleChange} placeholder="10.0.0.1" /></div>
        <div className="form-group"><label>Terminal emulator</label><input name="terminal" value={form.terminal} onChange={handleChange} placeholder="terminator" /></div>
        <div className="form-group"><label>Default identity file (SSH key)</label><input name="identity_file" value={form.identity_file} onChange={handleChange} placeholder="~/.ssh/id_rsa" /></div>

        <div className="settings-section">AI analysis (optional)</div>
        <div className="form-group">
          <label>Provider</label>
          <select name="ai_provider" value={form.ai_provider} onChange={handleProviderChange} className="form-select">
            <option value="claude">Claude (Anthropic)</option>
            <option value="openai">OpenAI</option>
            <option value="ollama">Ollama (local)</option>
            <option value="custom">Custom</option>
          </select>
        </div>
        <div className="form-group"><label>API URL</label><input name="ai_api_url" value={form.ai_api_url} onChange={handleChange} placeholder="https://api.anthropic.com/v1/messages" /></div>
        <div className="form-group"><label>API Key</label><input name="ai_api_key" value={form.ai_api_key} onChange={handleChange} placeholder="sk-ant-..." type="password" /></div>
        <div className="form-group"><label>Model</label><input name="ai_model" value={form.ai_model} onChange={handleChange} placeholder="claude-haiku-4-5" /></div>

        <div className="settings-section">Backup &amp; Restore</div>
        <div className="backup-actions">
          <button className="btn-backup" onClick={handleExport}>⬇ Export backup</button>
          <label className="btn-backup btn-import">
            ⬆ Import backup
            <input type="file" accept=".json" style={{display:"none"}} onChange={handleImport} />
          </label>
        </div>
        {backupMsg && <div className={`backup-msg ${backupMsg.type}`}>{backupMsg.text}</div>}
        <div className="modal-actions">
          <button className="btn-cancel" onClick={onClose}>Cancel</button>
          <button className="btn-primary" onClick={handleSubmit}>Save</button>
        </div>
      </div>
    </div>
  );
}

const READ_CMDS = new Set([
  "ls", "cat", "head", "tail", "wc", "sort", "uniq", "cut", "grep", "egrep", "fgrep", "awk",
  "stat", "file", "find", "df", "du", "free", "uptime", "uname", "hostname", "whoami", "id",
  "date", "who", "w", "last", "lsblk", "lscpu", "lsmod", "lspci", "vmstat", "iostat", "mpstat",
  "pidstat", "sar", "ps", "pgrep", "lsof", "ss", "ip", "getent", "systemctl", "journalctl",
  "dmesg", "rpm", "dpkg", "dnf", "yum",
]);
const SAFE_SYSTEMCTL = new Set(["status", "is-active", "is-enabled", "is-failed", "list-units", "list-unit-files", "list-timers", "list-sockets", "list-dependencies", "show", "cat", "get-default"]);
const SAFE_DPKG = new Set(["-l", "-L", "-s", "-S", "-p", "--list", "--listfiles", "--status", "--search", "--print-avail", "--get-selections"]);
const SAFE_PKG_SUB = new Set(["list", "info", "repolist", "check-update", "provides", "search"]);
const IP_DENY = new Set(["add", "del", "delete", "set", "flush", "replace", "change", "append", "exec", "monitor", "up", "down", "-b", "-batch", "-force"]);
const FIND_DENY = new Set(["-exec", "-execdir", "-ok", "-okdir", "-delete", "-fprint", "-fprint0", "-fprintf", "-fls"]);

// Splits a command line into pipeline segments of words. Returns { error } when it finds
// shell syntax that can chain, redirect or substitute commands outside quotes.
function parseShell(cmd) {
  const segments = [[]];
  let word = "";
  let inWord = false;
  let quote = null;
  const flush = () => {
    if (inWord) { segments[segments.length - 1].push(word); word = ""; inWord = false; }
  };
  for (let i = 0; i < cmd.length; i++) {
    const ch = cmd[i];
    const next = cmd[i + 1];
    if (quote === "'") {
      if (ch === "'") quote = null; else word += ch;
      continue;
    }
    if (ch === "\\") {
      if (i + 1 >= cmd.length) return { error: "trailing backslash" };
      word += cmd[++i];
      inWord = true;
      continue;
    }
    if (ch === "`") return { error: "uses backticks" };
    if (ch === "$" && next !== undefined && /[A-Za-z_{('"]/.test(next)) return { error: "uses $ expansion or substitution" };
    if (quote === '"') {
      if (ch === '"') quote = null; else word += ch;
      continue;
    }
    if (ch === "'" || ch === '"') { quote = ch; inWord = true; continue; }
    if (ch === "\n" || ch === "\r") return { error: "multi-line command" };
    if (";&<>".includes(ch)) return { error: `uses '${ch}' (chaining, background or redirection)` };
    if (ch === "|") { flush(); segments.push([]); continue; }
    if (/\s/.test(ch)) { flush(); continue; }
    word += ch;
    inWord = true;
  }
  if (quote) return { error: "unbalanced quotes" };
  flush();
  return { segments };
}

function checkSegment(words) {
  if (words.length === 0) return "empty command in a pipeline";
  const name = words[0];
  if (!READ_CMDS.has(name)) return `'${name}' is not in the read-only list`;
  const args = words.slice(1);
  const positional = args.filter(a => !a.startsWith("-"));
  const short = re => args.some(a => /^-[A-Za-z]/.test(a) && re.test(a.slice(1)));
  const long = (...names) => args.some(a => names.some(n => a === n || a.startsWith(n + "=")));
  switch (name) {
    case "tail":
      if (long("--follow") || short(/[fF]/)) return "tail -f never terminates";
      break;
    case "journalctl":
      if (long("--follow", "--vacuum-size", "--vacuum-time", "--vacuum-files", "--rotate", "--flush", "--sync", "--relinquish-var", "--smart-relinquish-var", "--setup-keys", "--update-catalog") || short(/f/)) return "journalctl option that follows or changes the journal";
      break;
    case "systemctl":
      if (!positional[0] || !SAFE_SYSTEMCTL.has(positional[0])) return `systemctl ${positional[0] || ""} may change a service`;
      break;
    case "ss":
      if (long("--kill") || short(/K/)) return "ss -K kills sockets";
      break;
    case "ip":
      if (args.some(a => IP_DENY.has(a))) return "ip subcommand that changes network configuration";
      break;
    case "dmesg":
      if (long("--clear", "--read-clear", "--console-off", "--console-on", "--console-level", "--follow", "--follow-new") || short(/[cCDEnwW]/)) return "dmesg option that clears the buffer, changes the console level or follows";
      break;
    case "rpm":
      if (!(args[0] && (/^-(q|V)/.test(args[0]) || args[0] === "--query" || args[0] === "--verify"))) return "rpm without -q or -V may install or remove packages";
      break;
    case "dpkg":
      if (!args[0] || !SAFE_DPKG.has(args[0])) return "dpkg option that may change packages";
      break;
    case "dnf":
    case "yum":
      if (!positional[0] || !SAFE_PKG_SUB.has(positional[0])) return `${name} ${positional[0] || ""} may change packages`;
      break;
    case "find":
      if (args.some(a => FIND_DENY.has(a))) return "find with an action that runs commands or writes files";
      break;
    case "sort":
      if (long("--output", "--compress-program") || short(/o/)) return "sort can write to a file here";
      break;
    case "uniq":
      if (positional.length > 1) return "uniq with two file arguments writes an output file";
      break;
    case "date":
      if (!args.every(a => a.startsWith("+") || ["-u", "--utc", "--universal", "-R", "-I"].includes(a))) return "date with arguments can set the system clock";
      break;
    case "hostname":
      if (!args.every(a => a.startsWith("-"))) return "hostname with a name sets the hostname";
      break;
    case "free":
      if (long("--seconds") || short(/s/)) return "free -s repeats forever";
      break;
    case "lsof":
      if (short(/r/)) return "lsof -r repeats forever";
      break;
    case "file":
      if (short(/C/) || long("--compile")) return "file -C writes a compiled magic file";
      break;
    case "awk": {
      const badFlag = args.find(a => a.startsWith("-") && !/^-F/.test(a) && a !== "-v");
      if (badFlag) return `awk option ${badFlag} is not accepted`;
      if (/system\s*\(|getline|[|>]/.test(args.join(" "))) return "awk program may run commands or write files";
      break;
    }
    case "vmstat":
    case "iostat":
    case "mpstat":
    case "pidstat":
    case "sar": {
      if (name === "sar" && short(/o/)) return "sar -o writes a file";
      const nums = positional.filter(a => /^\d+$/.test(a));
      if (nums.length === 1) return `${name} with an interval but no count never terminates`;
      break;
    }
    default:
      break;
  }
  return null;
}

// Local, conservative classification. Anything it cannot prove read-only needs confirmation.
// It reduces accidents; it is not a security boundary.
function classifyCommand(raw) {
  const cmd = (raw || "").trim();
  if (!cmd) return null;
  const cleaned = cmd.replace(/\s2>\s*\/dev\/null(?=\s|$)/g, "").replace(/\s2>&1(?=\s|$)/g, "");
  const parsed = parseShell(cleaned);
  if (parsed.error) return { level: "write", reason: parsed.error };
  for (const words of parsed.segments) {
    const problem = checkSegment(words);
    if (problem) return { level: "write", reason: problem };
  }
  return { level: "read", reason: "" };
}

function RunCommandModal({ hosts, network, config, onClose }) {
  const [command, setCommand] = useState("");
  const [running, setRunning] = useState(false);
  const [results, setResults] = useState([]);
  const [selectedResult, setSelectedResult] = useState(null);
  const [request, setRequest] = useState("");
  const [suggesting, setSuggesting] = useState(false);
  const [suggestion, setSuggestion] = useState(null);
  const [confirming, setConfirming] = useState(false);
  const [confirmText, setConfirmText] = useState("");

  const hasAi = !!(config?.ai?.api_key);
  const cls = classifyCommand(command);
  const needed = String(hosts.length);

  async function execute() {
    setConfirming(false);
    setConfirmText("");
    setRunning(true);
    setResults([]);
    try {
      const res = await invoke("run_command", {
        hosts,
        network,
        defaults: config.defaults,
        command,
      });
      setResults(res);
    } catch(e) {
      alert("Failed: " + e);
    } finally {
      setRunning(false);
    }
  }

  function handleRun() {
    if (!command.trim() || running) return;
    if (cls && cls.level === "write") { setConfirming(true); setConfirmText(""); return; }
    execute();
  }

  async function handleSuggest() {
    if (!request.trim() || suggesting) return;
    const ai = config.ai;
    setSuggesting(true);
    setSuggestion(null);
    setConfirming(false);
    let ok = true;
    let proposed = "";
    try {
      const out = await invoke("ai_suggest_command", {
        request, apiUrl: ai.api_url, apiKey: ai.api_key,
        model: ai.model, provider: ai.provider || "claude",
      });
      proposed = out.command || "";
      setSuggestion({ explanation: out.explanation || "", empty: !proposed });
      if (proposed) setCommand(proposed);
    } catch (e) {
      ok = false;
      setSuggestion({ explanation: `AI error: ${e}`, error: true });
    } finally {
      setSuggesting(false);
    }
    invoke("audit_event", {
      action: "ai_command_suggest", network: network.name, hosts: [],
      detail: JSON.stringify({ request, command: proposed }), success: ok,
    }).catch(() => {});
  }

  return (
    <div className="modal-overlay">
      <div className="modal modal-wide">
        <h3>⚡ Run command on {hosts.length} host{hosts.length !== 1 ? "s" : ""}</h3>

        {hasAi && (
          <div className="cmd-input-row">
            <input
              className="cmd-input"
              value={request}
              onChange={e => setRequest(e.target.value)}
              onKeyDown={e => e.key === "Enter" && handleSuggest()}
              placeholder="🤖 Describe what you want, e.g. which processes use the most memory"
            />
            <button className="btn-ai-analyze" onClick={handleSuggest} disabled={suggesting || !request.trim()}>
              {suggesting ? "Thinking…" : "🤖 Suggest"}
            </button>
          </div>
        )}
        {suggestion && (suggestion.error
          ? <div className="scp-result error">{suggestion.explanation}</div>
          : (
            <div className="health-ai">
              <div className="health-ai-label">🤖 Suggestion · review it before running</div>
              <div className="health-ai-text">{suggestion.empty ? "No command proposed. " : ""}{suggestion.explanation}</div>
            </div>
          ))}

        <div className="cmd-input-row">
          <input
            className="cmd-input"
            value={command}
            onChange={e => { setCommand(e.target.value); setConfirming(false); setSuggestion(null); }}
            onKeyDown={e => e.key === "Enter" && !running && handleRun()}
            placeholder="df -h / uptime / systemctl status nginx..."
            autoFocus
          />
          <button className="btn-primary" onClick={handleRun} disabled={running}>
            {running ? "Running..." : "▶ Run"}
          </button>
        </div>
        {cls && (
          <div className={`cmd-class ${cls.level}`}>
            {cls.level === "read" ? "✓ Read-only command" : `⚠ Needs confirmation: ${cls.reason}`}
          </div>
        )}

        {confirming && (
          <div className="cmd-confirm">
            <div className="cmd-confirm-title">This command may change the system or not terminate. It will run on {hosts.length} host{hosts.length !== 1 ? "s" : ""}:</div>
            <pre className="cmd-output">{command}</pre>
            <div className="cmd-confirm-hosts">
              {hosts.slice(0, 40).map(h => h.hostname || h.ip).join(", ")}
              {hosts.length > 40 ? `, … (${hosts.length - 40} more)` : ""}
            </div>
            <div className="cmd-input-row">
              <input
                className="cmd-input"
                value={confirmText}
                onChange={e => setConfirmText(e.target.value)}
                onKeyDown={e => e.key === "Enter" && confirmText.trim() === needed && execute()}
                placeholder={`Type ${needed} to confirm`}
                autoFocus
              />
              <button className="btn-primary" disabled={confirmText.trim() !== needed} onClick={execute}>Run on {needed}</button>
              <button className="btn-cancel" onClick={() => setConfirming(false)}>Cancel</button>
            </div>
          </div>
        )}

        {results.length > 0 && (
          <div className="cmd-results">
            {results.map((r, i) => (
              <div key={i}
                className={`cmd-result ${r.success ? "success" : "error"}`}
                onClick={() => setSelectedResult(selectedResult === i ? null : i)}
              >
                <div className="cmd-result-header">
                  <span className={`cmd-status-dot ${r.success ? "up" : "down"}`} />
                  <span className="cmd-result-host">{r.hostname || r.ip}</span>
                  <span className="cmd-result-ip">{r.ip}</span>
                  <span className="cmd-result-preview">
                    {selectedResult === i ? "▲" : "▼"} {(r.stdout || r.stderr || "").split(String.fromCharCode(10))[0].trim().substring(0, 50)}
                  </span>
                </div>
                {selectedResult === i && (
                  <div>
                    {r.stdout && <pre className="cmd-output">{r.stdout}</pre>}
                    {r.stderr && <pre className="cmd-output cmd-stderr">{r.stderr}</pre>}
                  </div>
                )}
              </div>
            ))}
          </div>
        )}

        <div className="modal-actions">
          <button className="btn-cancel" onClick={onClose}>Close</button>
        </div>
      </div>
    </div>
  );
}

function ScpBroadcastModal({ hosts, network, config, onClose }) {
  const [direction, setDirection] = useState("upload");
  const [localPath, setLocalPath] = useState("");
  const [remotePath, setRemotePath] = useState("");
  const [running, setRunning] = useState(false);
  const [results, setResults] = useState([]);

  async function browseLocal() {
    const selected = await open({ multiple: false, title: "Select file" });
    if (selected) setLocalPath(selected);
  }

  async function handleRun() {
    if (!localPath.trim() || !remotePath.trim()) {
      alert("Both paths are required");
      return;
    }
    setRunning(true);
    setResults([]);
    try {
      const res = await invoke("run_scp_broadcast", {
        hosts,
        network,
        defaults: config.defaults,
        localPath,
        remotePath,
        direction,
      });
      setResults(res);
    } catch(e) {
      alert("Failed: " + e);
    } finally {
      setRunning(false);
    }
  }

  return (
    <div className="modal-overlay">
      <div className="modal modal-wide">
        <h3>📁 SCP Broadcast — {hosts.length} hosts</h3>

        <div className="scp-direction">
          <button className={`scp-dir-btn ${direction === "upload" ? "active" : ""}`} onClick={() => setDirection("upload")}>↑ Upload (local → remote)</button>
          <button className={`scp-dir-btn ${direction === "download" ? "active" : ""}`} onClick={() => setDirection("download")}>↓ Download (remote → local)</button>
        </div>

        <div className="form-group">
          <label>Local path</label>
          <div className="form-row">
            <input value={localPath} onChange={e => setLocalPath(e.target.value)} placeholder="/home/user/script.sh" />
            <button className="btn-browse" onClick={browseLocal}>Browse</button>
          </div>
        </div>

        <div className="form-group">
          <label>Remote path</label>
          <input value={remotePath} onChange={e => setRemotePath(e.target.value)} placeholder="/tmp/script.sh" />
        </div>

        {results.length > 0 && (
          <div className="cmd-results">
            {results.map((r, i) => (
              <div key={i} className={`cmd-result ${r.success ? "success" : "error"}`}>
                <div className="cmd-result-header">
                  <span className={`cmd-status-dot ${r.success ? "up" : "down"}`} />
                  <span className="cmd-result-host">{r.hostname || r.ip}</span>
                  <span className="cmd-result-ip">{r.ip}</span>
                  <span style={{marginLeft:"auto", fontSize:"0.72rem", color: r.success ? "var(--green)" : "var(--red)"}}>
                    {r.success ? "✓ OK" : "✗ Failed"}
                  </span>
                </div>
                {r.message && <pre className="cmd-output">{r.message}</pre>}
              </div>
            ))}
          </div>
        )}

        <div className="modal-actions">
          <button className="btn-cancel" onClick={onClose}>Close</button>
          <button className="btn-primary" onClick={handleRun} disabled={running}>
            {running ? "Transferring..." : direction === "upload" ? "↑ Upload to all" : "↓ Download from all"}
          </button>
        </div>
      </div>
    </div>
  );
}

function RemoteBrowserModal({ host, network, config, onSelect, onClose }) {
  const [currentPath, setCurrentPath] = useState("/");
  const [entries, setEntries] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);

  const username = host.username || network.username || config?.defaults?.username || "root";
  const port = host.port || network.port || config?.defaults?.port || 22;
  const identityFile = host.identity_file || network.identity_file || config?.defaults?.identity_file || null;

  useEffect(() => { loadDir(currentPath); }, [currentPath]);

  async function loadDir(path) {
    setLoading(true);
    setError(null);
    try {
      const res = await invoke("list_remote_dir", {
        ip: host.ip, username, port, identityFile, path,
      });
      setEntries(res);
    } catch(e) {
      setError(e.toString());
    } finally {
      setLoading(false);
    }
  }

  function navigate(name) {
    if (name === "..") {
      const parts = currentPath.split("/").filter(Boolean);
      parts.pop();
      setCurrentPath("/" + (parts.length ? parts.join("/") : ""));
    } else {
      setCurrentPath(currentPath.endsWith("/") ? currentPath + name : currentPath + "/" + name);
    }
  }

  return (
    <div className="modal-overlay">
      <div className="modal modal-wide">
        <h3>📁 Remote browser — {host.hostname || host.ip}</h3>
        <div className="remote-path-bar">
          <span className="remote-path">{currentPath}</span>
          <button className="btn-cancel" onClick={() => navigate("..")}>↑ Up</button>
        </div>
        {error && <div className="scp-result error">{error}</div>}
        <div className="remote-entries">
          {loading && <div className="remote-loading">Loading...</div>}
          {!loading && entries.map((e, i) => (
            <div key={i} className={`remote-entry ${e.is_dir ? "dir" : "file"}`}
              onClick={() => e.is_dir ? navigate(e.name) : null}>
              <span className="remote-entry-icon">{e.is_dir ? "📁" : "📄"}</span>
              <span className="remote-entry-name">{e.name}</span>
              <span className="remote-entry-size">{e.is_dir ? "" : e.size}</span>
              {!e.is_dir && (
                <button className="btn-select-file" onClick={() => {
                  const fullPath = currentPath.endsWith("/")
                    ? currentPath + e.name
                    : currentPath + "/" + e.name;
                  onSelect(fullPath);
                  onClose();
                }}>Select</button>
              )}
            </div>
          ))}
        </div>
        <div className="modal-actions">
          <button className="btn-cancel" onClick={onClose}>Cancel</button>
          <button className="btn-primary" onClick={() => { onSelect(currentPath); onClose(); }}>
            Use this directory
          </button>
        </div>
      </div>
    </div>
  );
}

function ScpModal({ host, network, config, onClose }) {
  const [direction, setDirection] = useState("upload");
  const [localPath, setLocalPath] = useState("");
  const [remotePath, setRemotePath] = useState("");
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState(null);
  const [showRemoteBrowser, setShowRemoteBrowser] = useState(false);

  const username = host.username || network.username || config?.defaults?.username || "root";
  const port = host.port || network.port || config?.defaults?.port || 22;
  const identityFile = host.identity_file || network.identity_file || config?.defaults?.identity_file || null;

  async function browseLocal() {
    if (direction === "upload") {
      const selected = await open({ multiple: false, title: "Select file to upload" });
      if (selected) setLocalPath(selected);
    } else {
      const selected = await save({ title: "Save downloaded file as" });
      if (selected) setLocalPath(selected);
    }
  }

  async function handleRun() {
    if (!localPath.trim() || !remotePath.trim()) {
      alert("Both paths are required");
      return;
    }
    setRunning(true);
    setResult(null);
    try {
      const out = await invoke("run_scp", {
        ip: host.ip,
        username,
        port,
        identityFile,
        localPath,
        remotePath,
        direction,
      });
      setResult({ success: true, message: out || "Transfer completed successfully" });
    } catch(e) {
      setResult({ success: false, message: e });
    } finally {
      setRunning(false);
    }
  }

  return (
    <div className="modal-overlay">
      <div className="modal">
        <h3>📁 SCP — {host.hostname || host.ip}</h3>

        <div className="scp-direction">
          <button
            className={`scp-dir-btn ${direction === "upload" ? "active" : ""}`}
            onClick={() => setDirection("upload")}
          >↑ Upload (local → remote)</button>
          <button
            className={`scp-dir-btn ${direction === "download" ? "active" : ""}`}
            onClick={() => setDirection("download")}
          >↓ Download (remote → local)</button>
        </div>

        <div className="form-group">
          <label>Local path</label>
          <div className="form-row">
            <input
              value={localPath}
              onChange={e => setLocalPath(e.target.value)}
              placeholder={direction === "upload" ? "/home/user/script.sh" : "/tmp/downloaded_file"}
            />
            <button className="btn-browse" onClick={browseLocal}>Browse</button>
          </div>
        </div>

        {showRemoteBrowser && (
          <RemoteBrowserModal
            host={host}
            network={network}
            config={config}
            onSelect={setRemotePath}
            onClose={() => setShowRemoteBrowser(false)}
          />
        )}
        <div className="form-group">
          <label>Remote path ({username}@{host.ip})</label>
          <div className="form-row">
            <input
              value={remotePath}
              onChange={e => setRemotePath(e.target.value)}
              placeholder={direction === "upload" ? "/tmp/script.sh" : "/var/log/syslog"}
            />
            <button className="btn-browse" onClick={() => setShowRemoteBrowser(true)}>Browse</button>
          </div>
        </div>

        {result && (
          <div className={`scp-result ${result.success ? "success" : "error"}`}>
            {result.success ? "✓" : "✗"} {result.message}
          </div>
        )}

        <div className="modal-actions">
          <button className="btn-cancel" onClick={onClose}>Close</button>
          <button className="btn-primary" onClick={handleRun} disabled={running}>
            {running ? "Transferring..." : direction === "upload" ? "↑ Upload" : "↓ Download"}
          </button>
        </div>
      </div>
    </div>
  );
}

function analyzeHealth(data) {
  const issues = [];
  const metrics = {};

  const loadLine = data.match(/load average[s]?:\s*(.*)/i);
    const loadNums = loadLine ? (loadLine[1].match(/\d+(?:[.,]\d+)?/g) || []).map(n => n.replace(",", ".")) : [];
    const loadMatch = loadNums.length >= 3 ? [loadLine[0], ...loadNums] : null;
  if (loadMatch) {
    metrics.load1 = parseFloat(loadMatch[1]);
    metrics.load5 = parseFloat(loadMatch[2]);
    metrics.load15 = parseFloat(loadMatch[3]);
    if (metrics.load1 > 8) issues.push({ level: "critical", msg: `Very high load: ${metrics.load1}` });
    else if (metrics.load1 > 4) issues.push({ level: "warning", msg: `High load: ${metrics.load1}` });
  }

  const memMatch = data.match(/Mem:\s+(\d+)\s+(\d+)\s+(\d+)/);
  if (memMatch) {
    const total = parseInt(memMatch[1]);
    const used = parseInt(memMatch[2]);
    metrics.memUsedPct = Math.round((used / total) * 100);
    if (metrics.memUsedPct > 95) issues.push({ level: "critical", msg: `Memory critical: ${metrics.memUsedPct}%` });
    else if (metrics.memUsedPct > 85) issues.push({ level: "warning", msg: `Memory high: ${metrics.memUsedPct}%` });
  }

  const swapMatch = data.match(/Swap:\s+(\d+)\s+(\d+)\s+(\d+)/);
  if (swapMatch) {
    const swapTotal = parseInt(swapMatch[1]);
    const swapUsed = parseInt(swapMatch[2]);
    if (swapTotal > 0) {
      const swapPct = Math.round((swapUsed / swapTotal) * 100);
      metrics.swapUsedPct = swapPct;
      if (swapPct > 90) issues.push({ level: "critical", msg: `Swap full: ${swapPct}% (${swapUsed}/${swapTotal} MB)` });
      else if (swapPct > 50) issues.push({ level: "warning", msg: `Swap high: ${swapPct}%` });
    }
  }

  const dfLines = data.split(String.fromCharCode(10)).filter(l => l.match(/\d+%/));
  dfLines.forEach(line => {
    const match = line.match(/(\d+)%\s+(\S+)$/);
    if (match) {
      const pct = parseInt(match[1]);
      const mount = match[2];
      if (pct > 95) issues.push({ level: "critical", msg: `Disk ${mount} at ${pct}%` });
      else if (pct > 85) issues.push({ level: "warning", msg: `Disk ${mount} at ${pct}%` });
    }
  });

  const dmesgSection = (data.split("=== DMESG_TAIL ===")[1] || "").split("=== VMSTAT ===")[0];
    const dmesgPat = /\b(segfault|oom-killer|out of memory|killed process|kernel panic|call trace|blocked for more than|general protection fault|hardware error|i\/o error|errors?)\b/i;
    const dmesgErrors = dmesgSection.split(String.fromCharCode(10)).filter(l => dmesgPat.test(l));
  const dmesgSamples = [...new Set(
      dmesgSection.split(String.fromCharCode(10))
        .filter(l => dmesgPat.test(l))
        .map(l => l.replace(/^\[[^\]]*\]\s*/, "").trim().slice(0, 120))
    )].slice(0, 3);
  if (dmesgErrors.length > 3) issues.push({ level: "warning", msg: `${dmesgErrors.length} suspicious lines in dmesg`, sample: dmesgSamples });

  const health = issues.some(i => i.level === "critical") ? "critical"
    : issues.some(i => i.level === "warning") ? "warning" : "ok";

  const a = { health, issues, metrics };
  return a;
}

function HealthHistory({ networkName, ip, refreshKey }) {
  const [rows, setRows] = useState([]);

  useEffect(() => {
    if (refreshKey === "busy") return;
    invoke("get_health_history", { network: networkName, ip, limit: 30 })
      .then(setRows)
      .catch(() => setRows([]));
  }, [networkName, ip, refreshKey]);

  if (rows.length === 0) return null;

  const icon = { ok: "🟢", warning: "🟡", critical: "🔴" };
  const norm = s => s.replace(/\d+([.,]\d+)?/g, "#");
  const latest = rows[0];
  const prev = rows[1];
  let changeLine = null;
  if (prev) {
    const prevN = new Set((prev.issues || []).map(norm));
    const curN = new Set((latest.issues || []).map(norm));
    const added = (latest.issues || []).filter(m => !prevN.has(norm(m)));
    const resolved = (prev.issues || []).filter(m => !curN.has(norm(m)));
    const parts = [];
    if (prev.status !== latest.status) parts.push(`${icon[prev.status] || ""} ${prev.status} → ${icon[latest.status] || ""} ${latest.status}`);
    if (added.length) parts.push(`new: ${added.join("; ")}`);
    if (resolved.length) parts.push(`resolved: ${resolved.join("; ")}`);
    changeLine = parts.length ? parts.join(" · ") : "no change since the previous check";
  }
  const fmt = ts => new Date(ts * 1000).toLocaleString("en-GB", { day: "2-digit", month: "2-digit", year: "2-digit", hour: "2-digit", minute: "2-digit" });
  const val = (v, suffix = "") => (v === null || v === undefined ? "–" : `${v}${suffix}`);

  return (
    <details className="hh">
      <summary>History · {rows.length} check{rows.length !== 1 ? "s" : ""}{changeLine ? ` · ${changeLine}` : ""}</summary>
      <div className="hh-table">
        <div className="hh-row hh-head"><span>Date</span><span>Status</span><span>Load</span><span>Mem</span><span>Swap</span></div>
        {rows.map((r, i) => (
          <div key={i} className="hh-row">
            <span>{fmt(r.ts)}</span>
            <span>{icon[r.status] || ""} {r.status}</span>
            <span>{val(r.load1)}</span>
            <span>{val(r.mem, "%")}</span>
            <span>{val(r.swap, "%")}</span>
          </div>
        ))}
      </div>
    </details>
  );
}

function HealthCheckModal({ host, network, config, networks, setNetworks, selectedNetworkIndex, onClose }) {
  const [loading, setLoading] = useState(false);
  const [raw, setRaw] = useState("");
  const [analysis, setAnalysis] = useState(null);
  const [aiLoading, setAiLoading] = useState(false);
  const [error, setError] = useState(null);

  const username = host.username || network.username || config?.defaults?.username || "root";
  const port = host.port || network.port || config?.defaults?.port || 22;
  const identityFile = host.identity_file || network.identity_file || config?.defaults?.identity_file || null;
  const hasAi = !!(config?.ai?.api_key);

  useEffect(() => {
    if (host.health_status && host.health_report) {
      try { setAnalysis(JSON.parse(host.health_report)); } catch(e) { runHealthCheck(); }
    } else {
      runHealthCheck();
    }
  }, []);

  async function runHealthCheck() {
    setLoading(true);
    setError(null);
    try {
      const result = await invoke("health_check", { ip: host.ip, username, port, identityFile });
      setRaw(result.raw);
      const a = analyzeLocal(result.raw);
      await saveStatus(a.health, JSON.stringify(a));
    } catch(e) { setError(e.toString()); }
    finally { setLoading(false); }
  }

  function analyzeLocal(data) {
    const a = analyzeHealth(data);
    setAnalysis(a);
    return a;
  }

  async function saveStatus(health, report) {
    if (selectedNetworkIndex === undefined) return;
    try {
      await invoke("health_history_record", { network: network.name, entries: [{ ip: host.ip, status: health, report }] }).catch(() => {});
      const updated = await invoke("save_health_status", {
        networksFile: { networks }, networkIndex: selectedNetworkIndex,
        hostIp: host.ip, status: health, report,
      });
      setNetworks(updated.networks);
    } catch(e) { console.error(e); }
  }

  async function runAiAnalysis() {
    if (!hasAi) return;
    setAiLoading(true);
    const ai = config.ai;
    try {
      let data = raw;
      let base = analysis;
      if (!data) {
        const result = await invoke("health_check", { ip: host.ip, username, port, identityFile });
        if (!result.raw || !result.raw.includes("=== END ===")) {
          throw new Error((result.stderr || "incomplete output").split(String.fromCharCode(10))[0]);
        }
        data = result.raw;
        setRaw(data);
        base = analyzeLocal(data);
      }
      const insight = await invoke("ai_analyze", {
        rawData: data, localSummary: (base?.issues || []).map(i => `${i.level}: ${i.msg}`).join("; "), hostname: host.hostname || host.ip,
        apiUrl: ai.api_url, apiKey: ai.api_key,
        model: ai.model, provider: ai.provider || "claude",
      });
      const updated = { ...base, ai_insight: insight };
      setAnalysis(updated);
      await saveStatus(updated.health, JSON.stringify(updated));
    } catch(e) {
      setAnalysis(a => ({ ...a, ai_insight: `AI error: ${e}` }));
    } finally { setAiLoading(false); }
  }

  const healthColor = { ok: "var(--green)", warning: "var(--yellow)", critical: "var(--red)" };
  const healthIcon = { ok: "🟢", warning: "🟡", critical: "🔴" };

  return (
    <div className="modal-overlay">
      <div className="modal modal-wide">
        <h3>⚕ Health Check — {host.hostname || host.ip}</h3>
        {loading && <div className="health-loading">Running Brendan Gregg 60s checklist...</div>}
        {error && <div className="scp-result error">{error}</div>}
        {analysis && (
          <div className="health-summary">
            <div style={{display:"flex", alignItems:"center", justifyContent:"space-between"}}>
              <div className="health-status" style={{color: healthColor[analysis.health]}}>
                {healthIcon[analysis.health]} {analysis.health.toUpperCase()}
              </div>
              {host.health_status && !loading && (
                <span style={{fontSize:"0.7rem", color:"var(--text3)"}}>
                  Cached · <button className="btn-link" onClick={runHealthCheck}>Refresh</button>
                </span>
              )}
            </div>
            <div className="health-metrics">
              {analysis.metrics.load1 !== undefined && (
                <div className="health-metric">
                  <span className="health-metric-label">Load avg</span>
                  <span className="health-metric-value">{analysis.metrics.load1} / {analysis.metrics.load5} / {analysis.metrics.load15}</span>
                </div>
              )}
              {analysis.metrics.memUsedPct !== undefined && (
                <div className="health-metric">
                  <span className="health-metric-label">Memory</span>
                  <span className="health-metric-value" style={{color: analysis.metrics.memUsedPct > 85 ? "var(--red)" : "var(--text)"}}>
                    {analysis.metrics.memUsedPct}% used
                  </span>
                </div>
              )}
              {analysis.metrics.swapUsedPct !== undefined && (
                <div className="health-metric">
                  <span className="health-metric-label">Swap</span>
                  <span className="health-metric-value" style={{color: analysis.metrics.swapUsedPct > 90 ? "var(--red)" : analysis.metrics.swapUsedPct > 50 ? "var(--yellow)" : "var(--text)"}}>
                    {analysis.metrics.swapUsedPct}% used
                  </span>
                </div>
              )}
            </div>
            {analysis.issues.length > 0 && (
              <div className="health-issues">
                {analysis.issues.map((issue, i) => (
                  <div key={i} className={`health-issue ${issue.level}`}>
                    {issue.level === "critical" ? "🔴" : "🟡"} {issue.msg}
                  </div>
                ))}
              </div>
            )}
            {analysis.ai_insight ? (
              <div className="health-ai">
                <div className="health-ai-label">🤖 AI insights <button className="btn-link" onClick={runAiAnalysis} disabled={aiLoading}>↻ Regenerate</button></div>
                <div className="health-ai-text"><ReactMarkdown>{analysis.ai_insight}</ReactMarkdown></div>
              </div>
            ) : hasAi && !aiLoading && (
              <button className="btn-ai-analyze" onClick={runAiAnalysis}>
                {raw ? "🤖 Analyze with AI" : "🤖 Analyze with AI · runs a fresh check"}
              </button>
            )}
            {aiLoading && <div className="health-loading">AI analyzing...</div>}
          </div>
        )}
        <HealthHistory networkName={network.name} ip={host.ip} refreshKey={(loading || aiLoading) ? "busy" : (analysis ? `${analysis.health}|${JSON.stringify(analysis.metrics)}` : "none")} />
        {raw && (
          <details className="health-raw">
            <summary>Raw output</summary>
            <pre className="cmd-output-full">{raw}</pre>
          </details>
        )}
        <div className="modal-actions">
          <button className="btn-cancel" onClick={onClose}>Close</button>
          {!loading && <button className="btn-cancel" onClick={runHealthCheck}>↺ Rerun</button>}
        </div>
      </div>
    </div>
  );
}
function HealthBatchModal({ hosts, network, networkIndex, config, networks, setNetworks, onOpenHost, onClose }) {
  const POOL = 8;
  const [targets] = useState(hosts);
  const [items, setItems] = useState({});
  const [running, setRunning] = useState(true);
  const [cancelRequested, setCancelRequested] = useState(false);
  const [saveError, setSaveError] = useState(null);
  const [fleetSummary, setFleetSummary] = useState(null);
  const [fleetLoading, setFleetLoading] = useState(false);
  const cancelRef = useRef(false);
  const startedRef = useRef(false);

  useEffect(() => {
    if (startedRef.current) return;
    startedRef.current = true;
    runBatch();
  }, []);

  function setItem(ip, value) { setItems(prev => ({ ...prev, [ip]: value })); }

  async function runBatch() {
    const queue = targets.slice();
    const collected = [];

    async function worker() {
      while (queue.length > 0 && !cancelRef.current) {
        const h = queue.shift();
        setItem(h.ip, { state: "running" });
        const username = h.username || network.username || config?.defaults?.username || "root";
        const port = h.port || network.port || config?.defaults?.port || 22;
        const identityFile = h.identity_file || network.identity_file || config?.defaults?.identity_file || null;
        try {
          const result = await invoke("health_check", { ip: h.ip, username, port, identityFile });
          if (!result.raw || !result.raw.includes("=== END ===")) {
            throw new Error((result.stderr || "incomplete output").split(String.fromCharCode(10))[0]);
          }
          const a = analyzeHealth(result.raw);
          collected.push({ ip: h.ip, status: a.health, report: JSON.stringify(a) });
          setItem(h.ip, { state: "done", health: a.health, issues: a.issues.length, list: a.issues.map(i => `${i.level}: ${i.msg}${i.sample && i.sample.length ? ` [e.g. ${i.sample.join(" | ")}]` : ""}`) });
        } catch (e) {
          const msg = String(e).split(String.fromCharCode(10))[0].slice(0, 100);
          setItem(h.ip, { state: "failed", error: msg });
        }
      }
    }

    await Promise.all(Array.from({ length: Math.min(POOL, targets.length) }, () => worker()));

    if (collected.length > 0) {
      try {
        await invoke("health_history_record", { network: network.name, entries: collected }).catch(() => {});
        const updated = await invoke("save_health_batch", {
          networksFile: { networks }, networkIndex, entries: collected,
        });
        setNetworks(updated.networks);
      } catch (e) { setSaveError(String(e)); }
    }
    setRunning(false);
  }

  async function runFleetSummary() {
    const ai = config?.ai;
    if (!ai?.api_key || fleetLoading) return;
    const done = targets.filter(h => items[h.ip]?.state === "done");
    const flagged = done.filter(h => items[h.ip].health !== "ok");
    const okCount = done.length - flagged.length;
    const failedCount = targets.filter(h => items[h.ip]?.state === "failed").length;
    if (flagged.length === 0) {
      setFleetSummary(`All ${done.length} checked hosts are OK. Nothing to summarize.`);
      return;
    }
    const MAX = 150;
    const shown = flagged.slice(0, MAX);
    const lines = shown.map(h => `${h.hostname || h.ip} (${h.ip}) -> ${items[h.ip].health.toUpperCase()}: ${(items[h.ip].list || []).join("; ")}`);
    if (flagged.length > shown.length) lines.push(`(${flagged.length - shown.length} more flagged hosts omitted)`);
    const text = `Network: ${network.name}\nHosts checked: ${done.length}, OK: ${okCount}, flagged: ${flagged.length}, unreachable or failed: ${failedCount}\n\n` + lines.join("\n");
    setFleetLoading(true);
    setFleetSummary(null);
    let ok = true;
    try {
      const out = await invoke("ai_fleet_summary", {
        summaryText: text, apiUrl: ai.api_url, apiKey: ai.api_key,
        model: ai.model, provider: ai.provider || "claude",
      });
      setFleetSummary(out);
    } catch (e) {
      ok = false;
      setFleetSummary(`AI error: ${e}`);
    } finally {
      setFleetLoading(false);
    }
    invoke("audit_event", {
      action: "ai_fleet_summary", network: network.name, hosts: shown.map(h => h.ip),
      detail: `sent ${shown.length} host summaries (names, IPs and finding texts) to ${ai.provider || "claude"}`, success: ok,
    }).catch(() => {});
  }

  const total = targets.length;
  const finished = targets.filter(h => { const s = items[h.ip]?.state; return s === "done" || s === "failed"; }).length;
  const counts = { critical: 0, warning: 0, ok: 0, failed: 0, cancelled: 0 };
  targets.forEach(h => {
    const it = items[h.ip];
    if (it?.state === "done") counts[it.health] += 1;
    else if (it?.state === "failed") counts.failed += 1;
    else if (!running) counts.cancelled += 1;
  });
  const icon = { ok: "🟢", warning: "🟡", critical: "🔴" };

  return (
    <div className="modal-overlay">
      <div className="modal modal-wide">
        <h3>⚕ Health check — {total} host{total !== 1 ? "s" : ""}</h3>

        <div className="hb-progress"><div className="hb-progress-bar" style={{ width: `${total ? (finished / total) * 100 : 0}%` }} /></div>
        <div className="hb-summary">
          <span>{finished}/{total} done</span>
          <span style={{ color: "var(--red)" }}>{counts.critical} critical</span>
          <span style={{ color: "var(--yellow)" }}>{counts.warning} warning</span>
          <span style={{ color: "var(--green)" }}>{counts.ok} ok</span>
          <span style={{ color: "var(--text3)" }}>{counts.failed} failed</span>
          {counts.cancelled > 0 && <span style={{ color: "var(--text3)" }}>{counts.cancelled} cancelled</span>}
        </div>

        <div className="hb-list">
          {targets.map(h => {
            const it = items[h.ip] || { state: "pending" };
            const clickable = it.state === "done";
            let label = running ? "waiting" : "cancelled";
            let cls = "muted";
            if (it.state === "running") { label = "checking…"; }
            if (it.state === "done") { label = `${icon[it.health]} ${it.health.toUpperCase()}${it.issues ? ` · ${it.issues} issue${it.issues !== 1 ? "s" : ""}` : ""}`; cls = it.health; }
            if (it.state === "failed") { label = `✗ ${it.error}`; cls = "failed"; }
            return (
              <div key={h.ip} className={`hb-row ${clickable ? "clickable" : ""}`} onClick={() => clickable && onOpenHost(h)}>
                {it.state === "running" ? <span className="status-dot checking" /> : <span className="status-dot" style={{ opacity: 0 }} />}
                <span className="hb-host">{h.hostname || h.ip}</span>
                <span className="hb-ip">{h.ip}</span>
                <span className={`hb-state ${cls}`}>{label}</span>
              </div>
            );
          })}
        </div>

        {saveError && <div className="scp-result error">Could not save results: {saveError}</div>}
        {!running && config?.ai?.api_key && !fleetSummary && (
          <>
            <button className="btn-ai-analyze" onClick={runFleetSummary} disabled={fleetLoading}>
              {fleetLoading ? "AI summarizing…" : "🤖 Summarize results with AI"}
            </button>
            <div style={{ fontSize: "0.7rem", color: "var(--text3)" }}>Sends host names, IPs and the local findings (not the raw output) to the configured AI provider.</div>
          </>
        )}
        {fleetSummary && (
          <div className="health-ai">
            <div className="health-ai-label">🤖 Fleet summary <button className="btn-link" onClick={runFleetSummary} disabled={fleetLoading}>↻ Regenerate</button></div>
            <div className="health-ai-text"><ReactMarkdown>{fleetSummary}</ReactMarkdown></div>
          </div>
        )}
        {!running && <div style={{ fontSize: "0.72rem", color: "var(--text3)" }}>Unreachable hosts keep their previous health status. Click a finished host to see its analysis.</div>}

        <div className="modal-actions">
          {running && <button className="btn-cancel" disabled={cancelRequested} onClick={() => { cancelRef.current = true; setCancelRequested(true); }}>{cancelRequested ? "Cancelling…" : "Cancel"}</button>}
          <button className="btn-primary" onClick={onClose} disabled={running}>Close</button>
        </div>
      </div>
    </div>
  );
}

function CommandOutputModal({ result, onClose }) {
  return (
    <div className="modal-overlay">
      <div className="modal modal-wide">
        <h3>⚡ {result.hostname || result.ip} <span style={{color:"var(--text3)", fontSize:"0.75rem"}}>{result.ip}</span></h3>
        <div className={`cmd-result-badge ${result.success ? "success" : "error"}`}>
          {result.success ? "✓ Exit OK" : "✗ Exit error"}
        </div>
        {result.stdout && (
          <div className="form-group">
            <label>stdout</label>
            <pre className="cmd-output-full">{result.stdout}</pre>
          </div>
        )}
        {result.stderr && (
          <div className="form-group">
            <label>stderr</label>
            <pre className="cmd-output-full cmd-stderr">{result.stderr}</pre>
          </div>
        )}
        <div className="modal-actions">
          <button className="btn-primary" onClick={onClose}>Close</button>
        </div>
      </div>
    </div>
  );
}

function BroadcastConfirmModal({ hosts, network, config, onConfirm, onClose }) {
  const username = network?.username || config?.defaults?.username || "root";
  const port = network?.port || config?.defaults?.port || 22;
  const [checking, setChecking] = useState(true);
  const [hostStatus, setHostStatus] = useState({});

  useEffect(() => {
    async function preflight() {
      setChecking(true);
      try {
        const results = await invoke("check_hosts_reachable", {
          hosts,
          network,
          defaults: config.defaults,
        });
        const status = {};
        results.forEach(r => { status[r.ip] = r.reachable; });
        setHostStatus(status);
      } catch(e) { console.error(e); }
      finally { setChecking(false); }
    }
    preflight();
  }, []);

  const reachableHosts = hosts.filter(h => hostStatus[h.ip] === true);
  const unreachableHosts = hosts.filter(h => hostStatus[h.ip] === false);
  const pendingHosts = hosts.filter(h => hostStatus[h.ip] === undefined);

  return (
    <div className="modal-overlay">
      <div className="modal">
        <h3>⚡ Broadcast to {hosts.length} host{hosts.length !== 1 ? "s" : ""}</h3>
        {checking && <div className="health-loading">Checking host reachability...</div>}
        <div className="broadcast-confirm-list">
          {hosts.map((h, i) => {
            const status = hostStatus[h.ip];
            return (
              <div key={i} className="broadcast-confirm-item">
                {checking || status === undefined
                  ? <span className="status-dot checking" />
                  : <span className={`status-dot ${status ? "up" : "down"}`} />
                }
                <span className="broadcast-confirm-host">{h.hostname || h.ip}</span>
                <span className="broadcast-confirm-ip">{h.ip}</span>
                {!checking && status !== undefined && (
                  <span style={{fontSize:"0.7rem", color: status ? "var(--green)" : "var(--red)"}}>
                    {status ? "✓" : "✗"}
                  </span>
                )}
              </div>
            );
          })}
        </div>
        {!checking && unreachableHosts.length > 0 && (
          <div className="health-issue warning" style={{fontSize:"0.78rem"}}>
            ⚠ {unreachableHosts.length} host{unreachableHosts.length !== 1 ? "s" : ""} unreachable — broadcast will skip them
          </div>
        )}
        <div className="broadcast-confirm-meta">
          User: <strong>{username}</strong> · Port: <strong>{port}</strong>
          {!checking && <span> · <strong style={{color: reachableHosts.length > 0 ? "var(--green)" : "var(--red)"}}>{reachableHosts.length}/{hosts.length}</strong> reachable</span>}
        </div>
        <div className="modal-actions">
          <button className="btn-cancel" onClick={onClose}>Cancel</button>
          <button
            className="btn-broadcast"
            onClick={() => onConfirm(reachableHosts)}
            disabled={checking || reachableHosts.length === 0}
          >
            ⚡ Broadcast{!checking ? ` (${reachableHosts.length})` : ""}
          </button>
        </div>
      </div>
    </div>
  );
}

function App() {
  const [networks, setNetworks] = useState([]);
  const [config, setConfig] = useState(null);
  const [selectedNetwork, setSelectedNetwork] = useState(null);
  const [networkScoped, setNetworkScoped] = useState(false);
  const [filter, setFilter] = useState("");
  const [loading, setLoading] = useState(true);
  const [networkModal, setNetworkModal] = useState(null);
  const [scanning, setScanning] = useState(false);
  const [checking, setChecking] = useState(false);
  const [scanProgress, setScanProgress] = useState(null);
  const [checkProgress, setCheckProgress] = useState(null);
  const [selectedHosts, setSelectedHosts] = useState(new Set());
  const [broadcasting, setBroadcasting] = useState(false);
  const [hostModal, setHostModal] = useState(null);
  const [showTagModal, setShowTagModal] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [showRunCommand, setShowRunCommand] = useState(false);
  const [showScpBroadcast, setShowScpBroadcast] = useState(false);
  const [showBroadcastConfirm, setShowBroadcastConfirm] = useState(false);
  const [showHealthBatch, setShowHealthBatch] = useState(false);
  const [scpModal, setScpModal] = useState(null);
  const [healthModal, setHealthModal] = useState(null);
  const [history, setHistory] = useState([]);
  const [showHistory, setShowHistory] = useState(false);

  useEffect(() => { loadData(); loadHistory(); }, []);
  useEffect(() => { setSelectedHosts(new Set()); }, [selectedNetwork]);

  async function loadData() {
    try {
      const [netsResult, cfgResult] = await Promise.all([invoke("get_networks"), invoke("get_config")]);
      setNetworks(netsResult.networks);
      setConfig(cfgResult);
      if (netsResult.networks.length > 0) setSelectedNetwork(0);
    } catch (e) { console.error(e); }
    finally { setLoading(false); }
  }

  async function loadHistory() {
    try {
      const h = await invoke("get_history");
      setHistory(h.connections.slice().reverse());
    } catch(e) { console.error(e); }
  }

  function resolveUsername(host, network) { return host.username || network.username || config?.defaults?.username || "root"; }
  function resolvePort(host, network) { return host.port || network.port || config?.defaults?.port || 22; }

  function lastSeen(ip) {
    const rec = history.find(h => h.ip === ip);
    if (!rec) return null;
    const ts = parseInt(rec.connected_at) * 1000;
    const d = new Date(ts);
    return d.toLocaleString("en-GB", { day: "2-digit", month: "2-digit", year: "2-digit", hour: "2-digit", minute: "2-digit" });
  }

  async function connectSSH(host, netIdx = selectedNetwork) {
    const network = networks[netIdx];
    try { await invoke("launch_ssh", { ip: host.ip, username: resolveUsername(host, network), port: resolvePort(host, network), terminal: config?.defaults?.terminal || null, hostname: host.hostname || null, networkName: network.name, identityFile: host.identity_file || network.identity_file || config?.defaults?.identity_file || null, postConnect: host.post_connect || null }); loadHistory(); }
    catch (e) { alert("Failed to launch SSH: " + e); }
  }

  async function handleSaveNetwork(networkData, index) {
    try {
      let updated;
      if (index !== undefined) {
        updated = await invoke("update_network", { networksFile: { networks }, index, network: networkData });
      } else {
        updated = await invoke("add_network", { networksFile: { networks }, network: networkData });
        setSelectedNetwork(updated.networks.length - 1);
      }
      setNetworks(updated.networks);
    } catch (e) { alert("Failed to save network: " + e); }
  }

  async function handleSaveSettings(newConfig) {
    try {
      await invoke("save_config", { config: newConfig });
      setConfig(newConfig);
    } catch(e) { alert("Failed to save settings: " + e); }
  }

  async function handleRecalibrate(index) {
    try {
      const updated = await invoke("recalibrate_network", {
        networksFile: { networks },
        networkIndex: index,
        defaults: config.defaults,
      });
      setNetworks(updated.networks);
      return updated.networks[index].scan_timeout;
    } catch(e) {
      throw e;
    }
  }

  async function handleDeleteNetwork(index) {
    if (!confirm(`Delete network [${networks[index].name}]?`)) return;
    try {
      const updated = await invoke("delete_network", { networksFile: { networks }, index });
      setNetworks(updated.networks);
      setSelectedNetwork(updated.networks.length > 0 ? 0 : null);
    } catch (e) { alert("Failed to delete network: " + e); }
  }

  async function handleSaveHost({ hostData, networkIndex }) {
    try {
      let updated;
      if (hostModal.index !== undefined) {
        updated = await invoke("update_host", { networksFile: { networks }, networkIndex: selectedNetwork, hostIndex: hostModal.index, host: hostData });
      } else {
        updated = await invoke("add_host", { networksFile: { networks }, networkIndex, host: hostData });
        setSelectedNetwork(networkIndex);
      }
      setNetworks(updated.networks);
    } catch (e) { alert("Failed to save host: " + e); }
  }

  async function handleDeleteHost(hostIndex) {
    const host = networks[selectedNetwork].hosts[hostIndex];
    if (!confirm(`Delete host ${host.ip}?`)) return;
    try {
      const updated = await invoke("delete_host", { networksFile: { networks }, networkIndex: selectedNetwork, hostIndex });
      setNetworks(updated.networks);
    } catch (e) { alert("Failed to delete host: " + e); }
  }

  async function handleScan() {
    if (selectedNetwork === null || scanning) return;
    setScanning(true);
    setScanProgress({ total: 0, current: 0 });
    const unlisten = await listen("scan-progress", e => setScanProgress(e.payload));
    try {
      const updated = await invoke("scan_network", { networksFile: { networks }, networkIndex: selectedNetwork, defaults: config.defaults });
      setNetworks(updated.networks);
    } catch (e) { alert("Scan failed: " + e); }
    finally { unlisten(); setScanning(false); setScanProgress(null); }
  }

  async function handleCheck() {
    if (selectedNetwork === null || checking) return;
    setChecking(true);
    setCheckProgress({ total: 0, current: 0 });
    const unlisten = await listen("check-progress", e => setCheckProgress(e.payload));
    try {
      const updated = await invoke("check_hosts", { networksFile: { networks }, networkIndex: selectedNetwork, defaults: config.defaults });
      setNetworks(updated.networks);
    } catch (e) { alert("Check failed: " + e); }
    finally { unlisten(); setChecking(false); setCheckProgress(null); }
  }

  async function handleBulkTag(tag) {
    try {
      const updated = await invoke("bulk_tag", { networksFile: { networks }, networkIndex: selectedNetwork, hostIps: Array.from(selectedHosts), tag });
      setNetworks(updated.networks);
      setSelectedHosts(new Set());
    } catch (e) { alert("Failed to tag: " + e); }
  }

  function toggleHost(ip) {
    setSelectedHosts(prev => { const next = new Set(prev); next.has(ip) ? next.delete(ip) : next.add(ip); return next; });
  }

  function toggleAll() {
    setSelectedHosts(selectedHosts.size === filteredHosts.length ? new Set() : new Set(filteredHosts.map(h => h.ip)));
  }

  async function handleBroadcast(reachableHosts) {
    if (selectedHosts.size === 0 || broadcasting) return;
    const network = networks[selectedNetwork];
    const targets = reachableHosts && reachableHosts.length > 0
      ? reachableHosts
      : network.hosts.filter(h => selectedHosts.has(h.ip));
    setBroadcasting(true);
    let ok = true;
    let detail = "tmux session launched (delivery to each host is not confirmed)";
    try {
      await invoke("launch_broadcast", {
        hosts: targets,
        network, defaults: config.defaults,
        sessionName: `merendero-${network.name.toLowerCase().replace(/\s+/g, '-')}`,
      });
    } catch (e) { ok = false; detail = String(e); alert("Broadcast failed: " + e); }
    finally { setBroadcasting(false); }
    invoke("audit_event", { action: "broadcast", network: network.name, hosts: targets.map(h => h.ip), detail, success: ok }).catch(() => {});
  }

  function matchesFilter(host, filter) {
    if (!filter) return true;
    const f = filter.toLowerCase().trim();
    if (f === ":critical") return host.health_status === "critical";
    if (f === ":warning") return host.health_status === "warning";
    if (f === ":issues") return host.health_status === "critical" || host.health_status === "warning";
    if (f === ":healthy") return host.health_status === "ok";
    if (f === ":down") return !host.active;
    if (f === ":up") return host.active;
    return host.ip.toLowerCase().includes(f) ||
      (host.hostname || "").toLowerCase().includes(f) ||
      (host.tag || "").toLowerCase().includes(f);
  }

  const filteredHosts = selectedNetwork !== null
    ? networks[selectedNetwork]?.hosts.filter(host => matchesFilter(host, filter)) : [];

  const filteredHostsWithIndex = selectedNetwork !== null
    ? networks[selectedNetwork]?.hosts.map((host, index) => ({ host, index })).filter(({ host }) => matchesFilter(host, filter)) : [];

  useEffect(() => { if (filter.trim() === "") setNetworkScoped(false); }, [filter]);
  const globalMode = filter.trim() !== "" && !networkScoped;
  const globalResults = globalMode
    ? networks
        .map((net, netIdx) => ({ net, netIdx, hosts: net.hosts.filter(h => matchesFilter(h, filter)) }))
        .filter(r => r.hosts.length > 0)
    : [];
  const globalTotal = globalResults.reduce((n, r) => n + r.hosts.length, 0);

  const allSelected = filteredHosts.length > 0 && selectedHosts.size === filteredHosts.length;
  const currentNet = selectedNetwork !== null ? networks[selectedNetwork] : null;
  const activeCount = currentNet ? currentNet.hosts.filter(h => h.active).length : 0;

  if (loading) return <div className="loading">Loading...</div>;

  return (
    <div className="app">
      {networkModal && <NetworkModal network={networkModal.network} index={networkModal.index} onSave={handleSaveNetwork} onClose={() => setNetworkModal(null)} onRecalibrate={handleRecalibrate} />}
      {showHealthBatch && <HealthBatchModal hosts={networks[selectedNetwork]?.hosts.filter(h => selectedHosts.has(h.ip)) || []} network={networks[selectedNetwork]} networkIndex={selectedNetwork} config={config} networks={networks} setNetworks={setNetworks} onOpenHost={(h) => { const fresh = networks[selectedNetwork]?.hosts.find(x => x.ip === h.ip) || h; setHealthModal(fresh); }} onClose={() => setShowHealthBatch(false)} />}
      {healthModal && <HealthCheckModal host={healthModal} network={networks[selectedNetwork]} config={config} networks={networks} setNetworks={setNetworks} selectedNetworkIndex={selectedNetwork} onClose={() => setHealthModal(null)} />}
      {scpModal && <ScpModal host={scpModal} network={networks[selectedNetwork]} config={config} onClose={() => setScpModal(null)} />}
      {showBroadcastConfirm && <BroadcastConfirmModal hosts={networks[selectedNetwork]?.hosts.filter(h => selectedHosts.has(h.ip)) || []} network={networks[selectedNetwork]} config={config} onConfirm={(reachableHosts) => { setShowBroadcastConfirm(false); handleBroadcast(reachableHosts); }} onClose={() => setShowBroadcastConfirm(false)} />}
      {showScpBroadcast && <ScpBroadcastModal hosts={networks[selectedNetwork]?.hosts.filter(h => selectedHosts.has(h.ip)) || []} network={networks[selectedNetwork]} config={config} onClose={() => setShowScpBroadcast(false)} />}
      {showRunCommand && <RunCommandModal hosts={networks[selectedNetwork]?.hosts.filter(h => selectedHosts.has(h.ip)) || []} network={networks[selectedNetwork]} config={config} onClose={() => setShowRunCommand(false)} />}
      {showSettings && <SettingsModal config={config} onSave={handleSaveSettings} onClose={() => setShowSettings(false)} />}
      {hostModal && <HostModal host={hostModal.host} networks={networks} defaultNetworkIndex={selectedNetwork ?? 0} onSave={handleSaveHost} onClose={() => setHostModal(null)} onHealthCheck={(h) => { setHostModal(null); setHealthModal(h); }} />}
      {showTagModal && <TagModal count={selectedHosts.size} onApply={handleBulkTag} onClose={() => setShowTagModal(false)} />}

      <div className="sidebar">
        <div className="sidebar-header">
          <h2>Merendero</h2>
          <span className="sidebar-tagline">// manage your systems</span>
          
        </div>
        <div className="network-list">
          {networks.map((net, i) => {
            const active = net.hosts.filter(h => h.active).length;
            const total = net.hosts.length;
            return (
              <div key={i} className={`network-item ${selectedNetwork === i ? "active" : ""}`} onClick={() => { setSelectedNetwork(i); setFilter(""); }}>
                <div className="network-item-content">
                  <div className="network-name">{net.name}</div>
                  <div className="network-meta">{net.network}</div>
                  {total > 0 && (
                    <div className="network-badges">
                      <span className="badge-active">▲ {active}</span>
                      <span className="badge-inactive">▼ {total - active}</span>
                    </div>
                  )}
                </div>
                <div className="network-item-actions">
                  <button className="btn-edit-network" onClick={e => { e.stopPropagation(); setNetworkModal({ network: net, index: i }); }} title="Edit">✎</button>
                  <button className="btn-delete-network" onClick={e => { e.stopPropagation(); handleDeleteNetwork(i); }} title="Delete">✕</button>
                </div>
              </div>
            );
          })}
        </div>
      </div>

      <div className="main">
        {currentNet && (
          <>
            <div className="main-header">
              <div className="main-title">
                <h2>{globalMode ? `Search: ${filter}` : currentNet.name}</h2>
                <span className="network-cidr">{globalMode ? `${globalTotal} ${globalTotal === 1 ? "match" : "matches"} in ${globalResults.length} ${globalResults.length === 1 ? "network" : "networks"}` : `${currentNet.network} · ${activeCount}/${currentNet.hosts.length} up`}</span>
              </div>
              <div className="main-actions">
                <div className="filter-wrap"><input onKeyDown={e => { if (e.key === "Escape") setFilter(""); }} className="filter-input" type="text" placeholder="Filter: ip, fqdn, tag, :critical, :warning, :issues, :down, :up" value={filter} onChange={e => setFilter(e.target.value)} disabled={scanning || checking} />{filter && <button className="filter-clear" onClick={() => setFilter("")} title="Clear filter (Esc)">✕</button>}</div>
                <button className="btn-settings" onClick={() => setShowSettings(true)}>⚙</button>
                <button className="btn-add-network-main" onClick={() => setNetworkModal({ network: null, index: undefined })}>+ Network</button>
                {globalMode && <button className="btn-clear" onClick={() => setFilter("")}>✕ Clear search</button>}
                {!globalMode && <button className="btn-add-host" onClick={() => setHostModal({ host: null })}>+ Host</button>}
                {!globalMode && <button className="btn-check" onClick={handleCheck} disabled={checking || scanning}>{checking ? "Checking..." : "✓ Check"}</button>}
                {!globalMode && <button className={`btn-scan ${scanning ? "scanning" : ""}`} onClick={handleScan} disabled={scanning || checking}>{scanning ? "Scanning..." : "⟳ Scan"}</button>}
              </div>
            </div>

            {scanning && <ProgressBar progress={scanProgress} label="Scanning..." />}
            {checking && <ProgressBar progress={checkProgress} label="Checking..." />}

            {globalMode ? (
<>
  <div className="global-results">
    {globalResults.length === 0 && <div className="global-empty">No matches in any network</div>}
    {globalResults.map(({ net, netIdx, hosts }) => (
      <div key={netIdx} className="global-block">
        <div className="global-block-header">
          <span className="global-block-name">{net.name}</span>
          <span className="global-block-meta">{net.network} · {hosts.length} {hosts.length === 1 ? "match" : "matches"}</span>
          <button className="btn-edit" onClick={() => { setNetworkScoped(true); setSelectedNetwork(netIdx); }} title="Open this network keeping the filter">→ Open network</button>
        </div>
        {hosts.map(host => (
          <div key={host.ip} className={`global-row ${host.active ? "" : "inactive"}`}>
            <span style={{display:"flex", alignItems:"center", gap:"3px"}}>
              <span className={`status-dot ${host.active ? "up" : "down"}`} />
              {host.health_status === "critical" && <span title="Health: CRITICAL" style={{color:"var(--red)", fontSize:"0.65rem"}}>⚠</span>}
              {host.health_status === "warning" && <span title="Health: WARNING" style={{color:"var(--yellow)", fontSize:"0.65rem"}}>⚠</span>}
            </span>
            <span className="ip">{host.ip}</span>
            <span className="hostname" title={host.hostname || ""}>{host.hostname || ""}</span>
            <span>{host.tag ? <span className="tag-badge">{host.tag}</span> : ""}</span>
            <span className="last-seen">{lastSeen(host.ip) || ""}</span>
            <span className="actions">
              <button className="btn-connect" onClick={() => connectSSH(host, netIdx)}>SSH</button>
            </span>
          </div>
        ))}
      </div>
    ))}
  </div>
  <div className="main-footer">
    <span>{globalTotal} {globalTotal === 1 ? "match" : "matches"} in {globalResults.length} {globalResults.length === 1 ? "network" : "networks"}</span>
  </div>
</>
) : (
<>
{selectedHosts.size > 0 && (
              <div className="broadcast-toolbar">
                <span>{selectedHosts.size} hosts selected</span>
                <div className="broadcast-actions">
                  <button className="btn-clear" onClick={() => setSelectedHosts(new Set())}>Clear</button>
                  <button className="btn-tag" onClick={() => setShowTagModal(true)}>🏷 Tag</button>
                  <button className="btn-run-cmd" onClick={() => setShowRunCommand(true)}>▶ Run cmd</button>
                  <button className="btn-run-cmd" onClick={() => setShowHealthBatch(true)}>⚕ Health</button>
                  <button className="btn-run-cmd" onClick={() => setShowScpBroadcast(true)}>📁 SCP</button>
                  <button className={`btn-broadcast ${broadcasting ? "disabled" : ""}`} onClick={() => setShowBroadcastConfirm(true)} disabled={broadcasting}>⚡ Broadcast ({selectedHosts.size})</button>
                </div>
              </div>
            )}

            <div className="host-table">
              <div className="host-table-header">
                <span><input type="checkbox" checked={allSelected} onChange={toggleAll} /></span>
                <span>Up</span>
                <span>IP</span>
                <span>FQDN</span>
                <span>Tag</span>
                <span>Last connection</span>
                <span>Actions</span>
              </div>
              {filteredHostsWithIndex.map(({ host, index }) => (
                <div key={index} className={`host-row ${host.active ? "active" : "inactive"} ${selectedHosts.has(host.ip) ? "selected" : ""}`}>
                  <span><input type="checkbox" checked={selectedHosts.has(host.ip)} onChange={() => toggleHost(host.ip)} /></span>
                  <span style={{display:"flex", alignItems:"center", gap:"3px"}}>
                    <span className={`status-dot ${host.active ? "up" : "down"}`} />
                    {host.health_status === "critical" && <span title="Health: CRITICAL" style={{color:"var(--red)", fontSize:"0.65rem"}}>⚠</span>}
                    {host.health_status === "warning" && <span title="Health: WARNING" style={{color:"var(--yellow)", fontSize:"0.65rem"}}>⚠</span>}
                  </span>
                  <span className="ip">{host.ip}</span>
                  <span className="hostname" title={host.hostname || ""}>{host.hostname || ""}</span>
                  <span>{host.tag ? <span className="tag-badge">{host.tag}</span> : ""}</span>
                  <span className="last-seen">{lastSeen(host.ip) || ""}</span>
                  <span className="actions">
                    {host.notes && <span className="notes-icon" title={host.notes}>📝</span>}
                    <button className="btn-connect" onClick={() => connectSSH(host)}>SSH</button>
                    <button className="btn-health" onClick={() => setHealthModal(host)} title="Health check">⚕</button>
                    <button className="btn-scp" onClick={() => setScpModal(host)} title="SCP">📁</button>
                    <button className="btn-edit" onClick={() => setHostModal({ host, index })}>✎</button>
                    <button className="btn-delete-host" onClick={() => handleDeleteHost(index)}>✕</button>
                  </span>
                </div>
              ))}
            </div>

            <div className="main-footer">
              <span>{filteredHosts.length} hosts{filter ? ` (filtered)` : ""}</span>
              {selectedHosts.size > 0 && <span className="footer-selected">{selectedHosts.size} selected</span>}
            </div>
</>
)}
          </>
        )}
        {selectedNetwork === null && (
          <div className="empty-state">
            <p>No networks configured.</p>
            <button className="btn-primary" onClick={() => setNetworkModal({ network: null, index: undefined })}>Add your first network</button>
          </div>
        )}
      </div>
    </div>
  );
}

export default App;
