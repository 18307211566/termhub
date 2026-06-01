import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type React from "react";
import type { FormEvent } from "react";
import { useCallback, useEffect, useMemo, useState } from "react";

import {
  createSession,
  deleteSession,
  getAutostart,
  getServerInfo,
  kickClient,
  listClients,
  listSessions,
  restartSession,
  setAutostart,
  stopSession,
  updateSession,
} from "./api";
import type {
  ClientView,
  ServerInfo,
  SessionConfig,
  SessionStatus,
  SessionView,
  StatusEvent,
  UpstreamSpec,
} from "./types";

type Lang = "zh" | "en";
type UpKind = "loopback" | "ssh" | "telnet" | "raw_tcp" | "serial" | "local_shell" | "http_proxy";

const labels = {
  zh: {
    appName: "Termhub",
    copyFingerprint: "复制指纹",
    copyFailed: "复制失败",
    autostart: "自启",
    refresh: "刷新",
    noSessions: "暂无会话",
    newSession: "新建会话",
    chooseSession: "选择左侧会话查看详情",
    idle: "空闲",
    starting: "启动中",
    running: "运行",
    reconnecting: "重连",
    failed: "失败",
    stopped: "已停止",
    start: "启动",
    restart: "重启",
    stop: "停止",
    edit: "编辑",
    delete: "删除",
    sshAccess: "SSH 接入",
    localIp: "本机IP",
    user: "用户",
    passwordMasked: "密码: ***",
    downstream: "下联",
    noConnections: "暂无连接",
    remote: "远程",
    time: "时间",
    traffic: "入/出",
    kick: "踢",
    createTitle: "新建会话",
    editTitle: "编辑会话",
    sessionName: "会话名称",
    upstreamType: "上游类型",
    upstreamParams: "上游参数",
    downstreamSsh: "下联（SSH 终端接入）",
    sshUser: "SSH 用户名",
    sshListen: "SSH 监听地址",
    sshPassword: "SSH 密码",
    passwordKeep: "密码（留空则不修改）",
    newPassword: "新密码",
    random: "随机",
    autoReconnect: "自动重连",
    ptyOverride: "PTY 覆盖",
    cancel: "取消",
    createAndStart: "创建并启动",
    save: "保存",
    host: "主机",
    port: "端口",
    upstreamUser: "用户",
    upstreamPassword: "密码",
    serialPort: "串口",
    noSerialPorts: "暂无串口",
    baud: "波特率",
    dataBits: "数据位",
    parity: "校验",
    stopBits: "停止位",
    flow: "流控",
    command: "命令",
    listen: "监听地址",
    target: "目标地址",
    protocol: "协议",
    sshClient: "SSH 客户端",
    telnet: "Telnet",
    rawTcp: "原始 TCP",
    serial: "串口",
    localShell: "本地 Shell",
    httpProxy: "端口转发",
  },
  en: {
    appName: "Termhub",
    copyFingerprint: "Copy fingerprint",
    copyFailed: "Copy failed",
    autostart: "Autostart",
    refresh: "Refresh",
    noSessions: "No sessions",
    newSession: "New session",
    chooseSession: "Select a session on the left to view details",
    idle: "Idle",
    starting: "Starting",
    running: "Running",
    reconnecting: "Reconnecting",
    failed: "Failed",
    stopped: "Stopped",
    start: "Start",
    restart: "Restart",
    stop: "Stop",
    edit: "Edit",
    delete: "Delete",
    sshAccess: "SSH access",
    localIp: "local-ip",
    user: "User",
    passwordMasked: "Password: ***",
    downstream: "Clients",
    noConnections: "No connections",
    remote: "Remote",
    time: "Time",
    traffic: "In / Out",
    kick: "Kick",
    createTitle: "New session",
    editTitle: "Edit session",
    sessionName: "Session name",
    upstreamType: "Upstream type",
    upstreamParams: "Upstream settings",
    downstreamSsh: "Downstream SSH access",
    sshUser: "SSH username",
    sshListen: "SSH listen address",
    sshPassword: "SSH password",
    passwordKeep: "Password (leave blank to keep)",
    newPassword: "New password",
    random: "Random",
    autoReconnect: "Auto reconnect",
    ptyOverride: "PTY override",
    cancel: "Cancel",
    createAndStart: "Create and start",
    save: "Save",
    host: "Host",
    port: "Port",
    upstreamUser: "User",
    upstreamPassword: "Password",
    serialPort: "Serial port",
    noSerialPorts: "No serial ports",
    baud: "Baud",
    dataBits: "Data bits",
    parity: "Parity",
    stopBits: "Stop bits",
    flow: "Flow control",
    command: "Command",
    listen: "Listen address",
    target: "Target address",
    protocol: "Protocol",
    sshClient: "SSH client",
    telnet: "Telnet",
    rawTcp: "Raw TCP",
    serial: "Serial",
    localShell: "Local shell",
    httpProxy: "Port forwarding",
  },
} as const;

function getInitialLang(): Lang {
  const saved = localStorage.getItem("termhub-lang");
  if (saved === "zh" || saved === "en") return saved;
  return navigator.language.toLowerCase().startsWith("zh") ? "zh" : "en";
}

function formatStatus(s: SessionStatus, lang: Lang): string {
  const t = labels[lang];
  switch (s.state) {
    case "idle":
      return t.idle;
    case "starting":
      return t.starting;
    case "running":
      return `${t.running} ${s.uptime_secs}s`;
    case "reconnecting":
      return `${t.reconnecting} #${s.attempt}`;
    case "failed":
      return `${t.failed}: ${s.reason}`;
    case "stopped":
      return t.stopped;
    default:
      return "?";
  }
}

function statusDot(s: SessionStatus): string {
  switch (s.state) {
    case "running":
      return "text-emerald-400";
    case "starting":
    case "reconnecting":
      return "text-amber-400";
    case "failed":
    case "stopped":
      return "text-slate-500";
    default:
      return "text-slate-500";
  }
}

function randomPassword(len = 12): string {
  const chars = "abcdefghijkmnpqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789";
  const values = new Uint32Array(len);
  if (globalThis.crypto?.getRandomValues) {
    globalThis.crypto.getRandomValues(values);
    return Array.from(values, (n) => chars[n % chars.length]).join("");
  }
  return Array.from({ length: len }, () => chars[Math.floor(Math.random() * chars.length)]).join("");
}

function defaultUpstream(kind: UpKind): UpstreamSpec {
  switch (kind) {
    case "loopback":
      return { type: "loopback" };
    case "ssh":
      return { type: "ssh", host: "127.0.0.1", port: 22, user: "user", password: "" };
    case "telnet":
      return { type: "telnet", host: "127.0.0.1", port: 23 };
    case "raw_tcp":
      return { type: "raw_tcp", host: "127.0.0.1", port: 4000 };
    case "serial":
      return {
        type: "serial",
        port: "COM3",
        baud: 115200,
        data_bits: 8,
        parity: "none",
        stop_bits: 1,
        flow: "none",
        input_eol: "as_is",
        output_eol: "as_is",
      };
    case "local_shell":
      return { type: "local_shell", command: "cmd.exe", args: [] };
    case "http_proxy":
      return { type: "http_proxy", listen: "0.0.0.0:8080", target: "127.0.0.1:80", protocol: "tcp" };
  }
}

function upstreamLabel(spec: UpstreamSpec): string {
  switch (spec.type) {
    case "loopback":
      return "loopback";
    case "serial":
      return `${spec.port} @ ${spec.baud}`;
    case "ssh":
      return `${spec.user}@${spec.host}:${spec.port}`;
    case "telnet":
      return `${spec.host}:${spec.port}`;
    case "raw_tcp":
      return `${spec.host}:${spec.port}`;
    case "local_shell":
      return spec.command;
    case "http_proxy":
      return `${spec.listen} -> ${spec.target} (${spec.protocol.toUpperCase()})`;
  }
}

function kindToUpKind(kind: string): UpKind {
  if (kind === "loopback" || kind === "ssh" || kind === "telnet" || kind === "raw_tcp" || kind === "serial" || kind === "local_shell" || kind === "http_proxy") {
    return kind;
  }
  return "loopback";
}

function upstreamTypeLabel(kind: UpKind, lang: Lang): string {
  const t = labels[lang];
  return {
    loopback: "Loopback",
    ssh: t.sshClient,
    telnet: t.telnet,
    raw_tcp: t.rawTcp,
    serial: t.serial,
    local_shell: t.localShell,
    http_proxy: t.httpProxy,
  }[kind];
}

function formatConnectedAt(secs: number): string {
  const d = new Date(secs * 1000);
  return d.toLocaleTimeString();
}

function demoSessions(): SessionView[] {
  return [
    {
      name: "serial-lab",
      ssh_user: "admin",
      listen: "0.0.0.0:2222",
      status: { state: "running", uptime_secs: 428 },
      upstream_kind: "serial",
      upstream_summary: "COM4 @ 115200",
      upstream: {
        type: "serial",
        port: "COM4",
        baud: 115200,
        data_bits: 8,
        parity: "none",
        stop_bits: 1,
        flow: "none",
        input_eol: "as_is",
        output_eol: "as_is",
      },
      auto_reconnect: true,
      client_count: 2,
    },
    {
      name: "ap-ssh",
      ssh_user: "ops",
      listen: "127.0.0.1:2223",
      status: { state: "reconnecting", attempt: 2 },
      upstream_kind: "ssh",
      upstream_summary: "ops@192.168.1.20:22",
      upstream: { type: "ssh", host: "192.168.1.20", port: 22, user: "ops", password: "" },
      auto_reconnect: true,
      client_count: 0,
    },
  ];
}

function demoClients(): ClientView[] {
  const now = Math.floor(Date.now() / 1000);
  return [
    { id: 1, remote: "192.168.1.42:52018", connected_at_secs: now - 310, bytes_in: 1842, bytes_out: 92334 },
    { id: 2, remote: "192.168.1.43:52044", connected_at_secs: now - 84, bytes_in: 730, bytes_out: 25190 },
  ];
}

function UpstreamFields({
  upstream,
  setUpstream,
  serialPorts,
  lang,
}: {
  upstream: UpstreamSpec;
  setUpstream: React.Dispatch<React.SetStateAction<UpstreamSpec>>;
  serialPorts: string[];
  lang: Lang;
}) {
  const t = labels[lang];
  if (upstream.type === "ssh") {
    return (
      <div className="grid grid-cols-2 gap-2">
        <Field label={t.host}><input className="input w-full" value={upstream.host} onChange={(e) => setUpstream({ ...upstream, host: e.target.value })} /></Field>
        <Field label={t.port}><input className="input w-full" type="number" value={upstream.port} onChange={(e) => setUpstream({ ...upstream, port: Number(e.target.value) || 22 })} /></Field>
        <Field label={t.upstreamUser}><input className="input w-full" value={upstream.user} onChange={(e) => setUpstream({ ...upstream, user: e.target.value })} /></Field>
        <Field label={t.upstreamPassword}><input className="input w-full" type="password" value={upstream.password} onChange={(e) => setUpstream({ ...upstream, password: e.target.value })} /></Field>
      </div>
    );
  }
  if (upstream.type === "telnet" || upstream.type === "raw_tcp") {
    return (
      <div className="grid grid-cols-2 gap-2">
        <Field label={t.host}><input className="input w-full" value={upstream.host} onChange={(e) => setUpstream({ ...upstream, host: e.target.value })} /></Field>
        <Field label={t.port}><input className="input w-full" type="number" value={upstream.port} onChange={(e) => setUpstream({ ...upstream, port: Number(e.target.value) || 0 })} /></Field>
      </div>
    );
  }
  if (upstream.type === "serial") {
    return (
      <div className="grid grid-cols-3 gap-2">
        <Field label={t.serialPort}>
          <select className="input w-full" value={upstream.port} onChange={(e) => setUpstream({ ...upstream, port: e.target.value })}>
            {serialPorts.length === 0 && <option value="">{t.noSerialPorts}</option>}
            {serialPorts.map((p) => <option key={p} value={p}>{p}</option>)}
          </select>
        </Field>
        <Field label={t.baud}>
          <select className="input w-full" value={upstream.baud} onChange={(e) => setUpstream({ ...upstream, baud: Number(e.target.value) })}>
            {[9600, 19200, 38400, 57600, 115200, 230400, 460800, 921600].map((b) => <option key={b} value={b}>{b}</option>)}
          </select>
        </Field>
        <Field label={t.dataBits}>
          <select className="input w-full" value={upstream.data_bits} onChange={(e) => setUpstream({ ...upstream, data_bits: Number(e.target.value) as 5|6|7|8 })}>
            {[5, 6, 7, 8].map((b) => <option key={b} value={b}>{b}</option>)}
          </select>
        </Field>
        <Field label={t.parity}><select className="input w-full" value={upstream.parity} onChange={(e) => setUpstream({ ...upstream, parity: e.target.value })}><option value="none">None</option><option value="even">Even</option><option value="odd">Odd</option></select></Field>
        <Field label={t.stopBits}><select className="input w-full" value={upstream.stop_bits} onChange={(e) => setUpstream({ ...upstream, stop_bits: Number(e.target.value) as 1|2 })}><option value="1">1</option><option value="2">2</option></select></Field>
        <Field label={t.flow}><select className="input w-full" value={upstream.flow} onChange={(e) => setUpstream({ ...upstream, flow: e.target.value })}><option value="none">None</option><option value="hardware">Hardware</option><option value="software">Software</option></select></Field>
      </div>
    );
  }
  if (upstream.type === "local_shell") {
    return <Field label={t.command}><input className="input w-full" value={upstream.command} onChange={(e) => setUpstream({ ...upstream, command: e.target.value, args: [] })} /></Field>;
  }
  if (upstream.type === "http_proxy") {
    return (
      <div className="grid grid-cols-3 gap-2">
        <Field label={t.listen}><input className="input w-full" value={upstream.listen} onChange={(e) => setUpstream({ ...upstream, listen: e.target.value })} /></Field>
        <Field label={t.target}><input className="input w-full" value={upstream.target} onChange={(e) => setUpstream({ ...upstream, target: e.target.value })} /></Field>
        <Field label={t.protocol}><select className="input w-full" value={upstream.protocol} onChange={(e) => setUpstream({ ...upstream, protocol: e.target.value })}><option value="tcp">TCP</option><option value="udp">UDP</option></select></Field>
      </div>
    );
  }
  return null;
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div>
      <label className="mb-1 block text-xs text-slate-500">{label}</label>
      {children}
    </div>
  );
}

export default function App() {
  const isDemo = new URLSearchParams(location.search).has("demo");
  const [lang, setLang] = useState<Lang>(getInitialLang);
  const t = labels[lang];
  const [sessions, setSessions] = useState<SessionView[]>(isDemo ? demoSessions() : []);
  const [server, setServer] = useState<ServerInfo | null>(isDemo ? { host_key_fpr: "SHA256:termhub-demo-fingerprint" } : null);
  const [err, setErr] = useState<string | null>(null);
  const [autoStart, setAutoStartState] = useState(false);
  const [selectedName, setSelectedName] = useState<string | null>(isDemo ? "serial-lab" : null);
  const [clients, setClients] = useState<ClientView[]>(isDemo ? demoClients() : []);

  const [showNew, setShowNew] = useState(false);
  const [newName, setNewName] = useState("");
  const [newPassword, setNewPassword] = useState(() => randomPassword());
  const [newSshUser, setNewSshUser] = useState("admin");
  const [newListen, setNewListen] = useState("0.0.0.0:2222");
  const [newAutoRc, setNewAutoRc] = useState(true);
  const [newPtyOverride, setNewPtyOverride] = useState<{ cols: number; rows: number } | null>(null);
  const [upKind, setUpKind] = useState<UpKind>("ssh");
  const [upstream, setUpstream] = useState<UpstreamSpec>(() => defaultUpstream("ssh"));
  const [serialPorts, setSerialPorts] = useState<string[]>(isDemo ? ["COM3", "COM4"] : []);

  const [showEdit, setShowEdit] = useState(false);
  const [editName, setEditName] = useState("");
  const [editSshUser, setEditSshUser] = useState("admin");
  const [editListen, setEditListen] = useState("0.0.0.0:2222");
  const [editPassword, setEditPassword] = useState("");
  const [editAutoRc, setEditAutoRc] = useState(true);
  const [editUpKind, setEditUpKind] = useState<UpKind>("ssh");
  const [editUpstream, setEditUpstream] = useState<UpstreamSpec>(defaultUpstream("ssh"));

  const selected = useMemo(() => sessions.find((s) => s.name === selectedName) || null, [sessions, selectedName]);

  const refreshSessions = useCallback(async () => {
    if (isDemo) return;
    try {
      setErr(null);
      const list = await listSessions();
      setSessions(list);
      if (selectedName && !list.find((s) => s.name === selectedName)) setSelectedName(null);
    } catch (e) {
      setErr(String(e));
    }
  }, [isDemo, selectedName]);

  const refreshServer = useCallback(async () => {
    if (isDemo) return;
    try {
      const info = await getServerInfo();
      setServer(info);
      setErr(null);
    } catch (e) {
      setErr(String(e));
    }
  }, [isDemo]);

  const refreshClients = useCallback(async (name: string) => {
    if (isDemo) return;
    try {
      setClients(await listClients(name));
    } catch {
      setClients([]);
    }
  }, [isDemo]);

  useEffect(() => {
    localStorage.setItem("termhub-lang", lang);
  }, [lang]);

  useEffect(() => {
    if (isDemo) return;
    void refreshSessions();
    void refreshServer();
    void getAutostart().then(setAutoStartState).catch(() => {});
    void invoke<string[]>("list_serial_ports").then(setSerialPorts).catch(() => {});
    const interval = setInterval(() => void refreshSessions(), 1000);
    return () => clearInterval(interval);
  }, [isDemo, refreshSessions, refreshServer]);

  useEffect(() => {
    setUpstream(defaultUpstream(upKind));
  }, [upKind]);

  useEffect(() => {
    if (isDemo) return;
    if (!selectedName) {
      setClients([]);
      return;
    }
    void refreshClients(selectedName);
    const interval = setInterval(() => void refreshClients(selectedName), 1000);
    return () => clearInterval(interval);
  }, [isDemo, selectedName, refreshClients]);

  useEffect(() => {
    if (isDemo) return;
    let alive = true;
    let drop: (() => void) | undefined;
    listen<StatusEvent>("session:status", (ev) => {
      const { name, status } = ev.payload;
      setSessions((prev) => prev.map((row) => (row.name === name ? { ...row, status } : row)));
    }).then((unlisten) => {
      if (alive) drop = unlisten;
      else unlisten();
    });
    return () => {
      alive = false;
      drop?.();
    };
  }, [isDemo]);

  function openEditDialog() {
    if (!selected) return;
    setEditName(selected.name);
    setEditSshUser(selected.ssh_user);
    setEditListen(selected.listen);
    setEditPassword("");
    setEditAutoRc(selected.auto_reconnect);
    const k = kindToUpKind(selected.upstream_kind);
    setEditUpKind(k === "loopback" ? "ssh" : k);
    setEditUpstream(selected.upstream.type === "loopback" ? defaultUpstream("ssh") : selected.upstream);
    setShowEdit(true);
  }

  async function submitNew(ev: FormEvent) {
    ev.preventDefault();
    let up = upstream;
    if (up.type === "local_shell") up = { type: "local_shell", command: up.command, args: [] };
    const isHttpProxy = up.type === "http_proxy";
    const cfg: SessionConfig = {
      name: newName.trim(),
      password: isHttpProxy ? "" : newPassword,
      ssh_user: isHttpProxy ? "" : newSshUser,
      listen: isHttpProxy ? "" : newListen,
      auto_reconnect: isHttpProxy ? false : newAutoRc,
      pty_override: isHttpProxy ? null : newPtyOverride,
      upstream: up,
    };
    try {
      await createSession(cfg);
      setShowNew(false);
      setNewName("");
      await refreshSessions();
      setErr(null);
    } catch (e) {
      setErr(String(e));
    }
  }

  async function applyEdit(ev: FormEvent) {
    ev.preventDefault();
    if (!selected) return;
    let up = editUpstream;
    if (up.type === "local_shell") up = { type: "local_shell", command: up.command, args: [] };
    const isHttpProxy = up.type === "http_proxy";
    const cfg: SessionConfig = {
      name: editName.trim(),
      password: isHttpProxy ? "" : editPassword,
      ssh_user: isHttpProxy ? "" : editSshUser,
      listen: isHttpProxy ? "" : editListen,
      auto_reconnect: isHttpProxy ? false : editAutoRc,
      pty_override: null,
      upstream: up,
    };
    try {
      await updateSession(selected.name, cfg);
      setShowEdit(false);
      setSelectedName(editName.trim());
      await refreshSessions();
    } catch (e) {
      setErr(String(e));
    }
  }

  async function toggleAutostart() {
    try {
      await setAutostart(!autoStart);
      setAutoStartState(await getAutostart());
    } catch (e) {
      setErr(String(e));
    }
  }

  async function copyFingerprint() {
    if (!server) return;
    try {
      await navigator.clipboard.writeText(server.host_key_fpr);
    } catch {
      setErr(t.copyFailed);
    }
  }

  function resetNewDialog() {
    setShowNew(true);
    setUpKind("ssh");
    setUpstream(defaultUpstream("ssh"));
    setNewName("");
    setNewPassword(randomPassword());
    setNewSshUser("admin");
    setNewListen("0.0.0.0:2222");
    setNewAutoRc(true);
    setNewPtyOverride(null);
  }

  return (
    <div className="flex h-screen flex-col bg-slate-950 text-slate-100">
      <header className="flex items-center gap-3 border-b border-slate-800 px-4 py-2 text-sm">
        <h1 className="text-base font-semibold text-white">{t.appName}</h1>
        {server && <button className="btn-sm" onClick={() => void copyFingerprint()}>{t.copyFingerprint}</button>}
        <div className="flex-1" />
        <button className="btn-sm" onClick={() => setLang(lang === "zh" ? "en" : "zh")}>{lang === "zh" ? "English" : "中文"}</button>
        <label className="flex items-center gap-1 text-xs text-slate-400">
          <input type="checkbox" checked={autoStart} onChange={() => void toggleAutostart()} />
          {t.autostart}
        </label>
        <button className="btn-sm bg-cyan-600 text-white hover:bg-cyan-500" onClick={() => void refreshSessions()}>{t.refresh}</button>
      </header>

      {err && <div className="border-b border-red-900/50 bg-red-950/30 px-4 py-1 text-xs text-red-300">{err}</div>}

      <div className="flex flex-1 overflow-hidden">
        <aside className="flex w-60 flex-col border-r border-slate-800">
          <div className="flex-1 overflow-y-auto p-2">
            {sessions.length === 0 && <p className="p-4 text-center text-xs text-slate-500">{t.noSessions}</p>}
            {sessions.map((s) => (
              <button key={s.name} className={`mb-1 flex w-full items-center gap-2 rounded-lg px-3 py-2 text-left text-sm transition-colors ${selectedName === s.name ? "bg-slate-800 text-white" : "text-slate-300 hover:bg-slate-900"}`} onClick={() => setSelectedName(s.name === selectedName ? null : s.name)}>
                <span className={`text-lg ${statusDot(s.status)}`}>●</span>
                <div className="min-w-0 flex-1">
                  <div className="truncate font-medium">{s.name}</div>
                  <div className="truncate text-xs text-slate-500">{upstreamLabel(s.upstream)}</div>
                </div>
                {s.client_count > 0 && <span className="rounded-full bg-slate-800 px-1.5 py-0.5 text-xs text-slate-400">{s.client_count}</span>}
              </button>
            ))}
          </div>
          <div className="border-t border-slate-800 p-2">
            <button className="w-full rounded-lg bg-emerald-600 py-2 text-sm font-medium text-white hover:bg-emerald-500" onClick={resetNewDialog}>+ {t.newSession}</button>
          </div>
        </aside>

        <main className="flex flex-1 flex-col overflow-hidden">
          {selected ? (
            <div className="flex flex-1 flex-col gap-4 overflow-auto p-4">
              <div className="flex items-start justify-between">
                <div>
                  <h2 className="text-xl font-semibold text-white">{selected.name}</h2>
                  <div className="mt-1 flex items-center gap-3 text-sm text-slate-400">
                    <span className={statusDot(selected.status)}>{formatStatus(selected.status, lang)}</span>
                    {selected.upstream_kind !== "http_proxy" && <><span>·</span><span>{selected.upstream_kind}</span></>}
                    <span>·</span>
                    <span className="font-mono text-xs text-slate-500">{selected.upstream_summary}</span>
                  </div>
                </div>
                <div className="flex gap-2">
                  {selected.status.state === "stopped" || selected.status.state === "failed" ? (
                    <button className="btn-sm bg-emerald-600 text-white hover:bg-emerald-500" onClick={() => void restartSession(selected.name).then(refreshSessions).catch((e) => setErr(String(e)))}>{t.start}</button>
                  ) : (
                    <>
                      <button className="btn-sm" onClick={() => void restartSession(selected.name).then(refreshSessions).catch((e) => setErr(String(e)))}>{t.restart}</button>
                      <button className="btn-sm border-amber-800 text-amber-200 hover:bg-amber-950/50" onClick={() => void stopSession(selected.name).then(refreshSessions).catch((e) => setErr(String(e)))}>{t.stop}</button>
                    </>
                  )}
                  <button className="btn-sm" onClick={openEditDialog}>{t.edit}</button>
                  <button className="btn-sm border-red-900 text-red-300 hover:bg-red-950/40" onClick={() => { setSelectedName(null); void deleteSession(selected.name).then(refreshSessions).catch((e) => setErr(String(e))); }}>{t.delete}</button>
                </div>
              </div>

              {selected.upstream_kind !== "http_proxy" && (
                <div className="rounded-lg border border-slate-800 bg-slate-900/50 p-3">
                  <div className="mb-2 text-xs text-slate-500">{t.sshAccess}</div>
                  <code className="rounded bg-slate-800 px-2 py-1 text-sm text-cyan-300">ssh {selected.ssh_user}@&lt;{t.localIp}&gt; -p {selected.listen.split(":").pop() || "2222"}</code>
                  <div className="mt-2 text-xs text-slate-500">{t.user}: {selected.ssh_user} · {t.passwordMasked}</div>
                </div>
              )}

              <div className="flex-1 overflow-auto rounded-lg border border-slate-800 bg-slate-900/50">
                <div className="border-b border-slate-800 px-3 py-2 text-xs text-slate-500">{t.downstream} ({clients.length})</div>
                {clients.length === 0 ? (
                  <p className="p-4 text-center text-sm text-slate-600">{t.noConnections}</p>
                ) : (
                  <table className="w-full text-left text-sm">
                    <thead>
                      <tr className="border-b border-slate-800 text-xs text-slate-500">
                        <th className="px-3 py-2">#</th>
                        <th className="px-3 py-2">{t.remote}</th>
                        <th className="px-3 py-2">{t.time}</th>
                        <th className="px-3 py-2">{t.traffic}</th>
                        <th className="px-3 py-2" />
                      </tr>
                    </thead>
                    <tbody>
                      {clients.map((c) => (
                        <tr key={c.id} className="border-b border-slate-800/50 text-slate-300">
                          <td className="px-3 py-2 font-mono">{c.id}</td>
                          <td className="px-3 py-2">{c.remote}</td>
                          <td className="px-3 py-2 text-xs">{formatConnectedAt(c.connected_at_secs)}</td>
                          <td className="px-3 py-2 text-xs text-slate-500">{c.bytes_in} / {c.bytes_out}</td>
                          <td className="px-3 py-2"><button className="rounded bg-slate-800 px-2 py-1 text-xs hover:bg-slate-700" onClick={() => void kickClient(selected.name, c.id).then(() => refreshClients(selected.name)).catch((e) => setErr(String(e)))}>{t.kick}</button></td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                )}
              </div>
            </div>
          ) : (
            <div className="flex flex-1 items-center justify-center text-slate-600">{t.chooseSession}</div>
          )}
        </main>
      </div>

      {showNew && (
        <SessionDialog title={t.createTitle} submitLabel={t.createAndStart} lang={lang} upKind={upKind} setUpKind={setUpKind} upstream={upstream} setUpstream={setUpstream} serialPorts={serialPorts} onCancel={() => setShowNew(false)} onSubmit={submitNew}>
          <Field label={t.sessionName}><input className="input w-full" required value={newName} onChange={(e) => setNewName(e.target.value)} /></Field>
          {upKind !== "http_proxy" && <DownstreamFields lang={lang} sshUser={newSshUser} setSshUser={setNewSshUser} listen={newListen} setListen={setNewListen} password={newPassword} setPassword={setNewPassword} autoReconnect={newAutoRc} setAutoReconnect={setNewAutoRc} ptyOverride={newPtyOverride} setPtyOverride={setNewPtyOverride} />}
        </SessionDialog>
      )}

      {showEdit && selected && (
        <SessionDialog title={t.editTitle} submitLabel={t.save} lang={lang} upKind={editUpKind} setUpKind={(k) => { setEditUpKind(k); setEditUpstream(defaultUpstream(k)); }} upstream={editUpstream} setUpstream={setEditUpstream} serialPorts={serialPorts} onCancel={() => setShowEdit(false)} onSubmit={applyEdit}>
          <Field label={t.sessionName}><input className="input w-full" required value={editName} onChange={(e) => setEditName(e.target.value)} /></Field>
          {editUpKind !== "http_proxy" && <DownstreamFields lang={lang} sshUser={editSshUser} setSshUser={setEditSshUser} listen={editListen} setListen={setEditListen} password={editPassword} setPassword={setEditPassword} autoReconnect={editAutoRc} setAutoReconnect={setEditAutoRc} passwordLabel={t.passwordKeep} passwordPlaceholder={t.newPassword} />}
        </SessionDialog>
      )}
    </div>
  );
}

function SessionDialog({
  title,
  submitLabel,
  lang,
  upKind,
  setUpKind,
  upstream,
  setUpstream,
  serialPorts,
  onCancel,
  onSubmit,
  children,
}: {
  title: string;
  submitLabel: string;
  lang: Lang;
  upKind: UpKind;
  setUpKind: (kind: UpKind) => void;
  upstream: UpstreamSpec;
  setUpstream: React.Dispatch<React.SetStateAction<UpstreamSpec>>;
  serialPorts: string[];
  onCancel: () => void;
  onSubmit: (ev: FormEvent) => void;
  children: React.ReactNode;
}) {
  const t = labels[lang];
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="flex max-h-[90vh] w-full max-w-lg flex-col rounded-2xl border border-slate-700 bg-slate-900 p-6 shadow-2xl">
        <h3 className="mb-4 text-lg font-medium text-white">{title}</h3>
        <form className="flex flex-1 flex-col gap-3 overflow-auto" onSubmit={onSubmit}>
          {children}
          <Field label={t.upstreamType}>
            <select className="input w-full" value={upKind} onChange={(e) => setUpKind(e.target.value as UpKind)}>
              {(["ssh", "telnet", "raw_tcp", "serial", "local_shell", "http_proxy"] as UpKind[]).map((kind) => <option key={kind} value={kind}>{upstreamTypeLabel(kind, lang)}</option>)}
            </select>
          </Field>
          <div>
            <div className="mb-2 text-xs font-medium text-slate-400">{t.upstreamParams}</div>
            <UpstreamFields upstream={upstream} setUpstream={setUpstream} serialPorts={serialPorts} lang={lang} />
          </div>
          <div className="mt-auto flex justify-end gap-3 pt-4">
            <button type="button" className="btn-sm" onClick={onCancel}>{t.cancel}</button>
            <button type="submit" className="btn-sm bg-emerald-600 text-white hover:bg-emerald-500">{submitLabel}</button>
          </div>
        </form>
      </div>
    </div>
  );
}

function DownstreamFields({
  lang,
  sshUser,
  setSshUser,
  listen,
  setListen,
  password,
  setPassword,
  autoReconnect,
  setAutoReconnect,
  ptyOverride,
  setPtyOverride,
  passwordLabel,
  passwordPlaceholder,
}: {
  lang: Lang;
  sshUser: string;
  setSshUser: (value: string) => void;
  listen: string;
  setListen: (value: string) => void;
  password: string;
  setPassword: (value: string) => void;
  autoReconnect: boolean;
  setAutoReconnect: (value: boolean) => void;
  ptyOverride?: { cols: number; rows: number } | null;
  setPtyOverride?: (value: { cols: number; rows: number } | null) => void;
  passwordLabel?: string;
  passwordPlaceholder?: string;
}) {
  const t = labels[lang];
  return (
    <div className="border-t border-slate-800 pt-3">
      <div className="mb-2 text-xs font-medium text-slate-400">{t.downstreamSsh}</div>
      <div className="flex flex-col gap-3">
        <div className="grid grid-cols-2 gap-2">
          <Field label={t.sshUser}><input className="input w-full" value={sshUser} onChange={(e) => setSshUser(e.target.value)} /></Field>
          <Field label={t.sshListen}><input className="input w-full" value={listen} onChange={(e) => setListen(e.target.value)} /></Field>
        </div>
        <Field label={passwordLabel || t.sshPassword}>
          <div className="flex gap-1">
            <input className="input flex-1" type="password" autoComplete="new-password" placeholder={passwordPlaceholder} value={password} onChange={(e) => setPassword(e.target.value)} />
            <button type="button" className="btn-sm" onClick={() => setPassword(randomPassword())}>{t.random}</button>
          </div>
        </Field>
        <div className="flex flex-wrap items-center gap-3 text-sm">
          <label className="flex items-center gap-1 text-slate-300"><input type="checkbox" checked={autoReconnect} onChange={(e) => setAutoReconnect(e.target.checked)} /> {t.autoReconnect}</label>
          {setPtyOverride && (
            <label className="flex items-center gap-1 text-slate-300"><input type="checkbox" checked={ptyOverride !== null} onChange={(e) => setPtyOverride(e.target.checked ? { cols: 80, rows: 24 } : null)} /> {t.ptyOverride}</label>
          )}
          {ptyOverride && setPtyOverride && (
            <div className="flex gap-1">
              <input className="input w-16" type="number" value={ptyOverride.cols} onChange={(e) => setPtyOverride({ ...ptyOverride, cols: Number(e.target.value) || 80 })} />
              <span className="self-center text-slate-500">x</span>
              <input className="input w-16" type="number" value={ptyOverride.rows} onChange={(e) => setPtyOverride({ ...ptyOverride, rows: Number(e.target.value) || 24 })} />
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
