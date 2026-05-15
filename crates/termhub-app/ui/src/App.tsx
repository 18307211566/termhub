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

function formatStatus(s: SessionStatus): string {
  switch (s.state) {
    case "idle":
      return "空闲";
    case "starting":
      return "启动中";
    case "running":
      return `运行 ${s.uptime_secs}s`;
    case "reconnecting":
      return `重连 #${s.attempt}`;
    case "failed":
      return `失败: ${s.reason}`;
    case "stopped":
      return "已停止";
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

type UpKind =
  | "loopback"
  | "ssh"
  | "telnet"
  | "raw_tcp"
  | "serial"
  | "local_shell"
  | "http_proxy";

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
      return `${spec.listen} → ${spec.target} (${spec.protocol.toUpperCase()})`;
  }
}

function kindToUpKind(kind: string): UpKind {
  if (kind === "loopback" || kind === "ssh" || kind === "telnet" || kind === "raw_tcp" || kind === "serial" || kind === "local_shell" || kind === "http_proxy") {
    return kind;
  }
  return "loopback";
}

function formatConnectedAt(secs: number): string {
  const d = new Date(secs * 1000);
  return d.toLocaleTimeString();
}

/** Shared upstream-fields renderer used by both create and edit dialogs. */
function UpstreamFields({
  upstream,
  setUpstream,
  serialPorts,
}: {
  upstream: UpstreamSpec;
  setUpstream: React.Dispatch<React.SetStateAction<UpstreamSpec>>;
  serialPorts: string[];
}) {
  if (upstream.type === "ssh") {
    return (
      <div className="grid grid-cols-2 gap-2">
        <div>
          <label className="mb-1 block text-xs text-slate-500">主机</label>
          <input className="input w-full" value={upstream.host} onChange={(e) => setUpstream({ ...upstream, host: e.target.value })} />
        </div>
        <div>
          <label className="mb-1 block text-xs text-slate-500">端口</label>
          <input className="input w-full" type="number" value={upstream.port} onChange={(e) => setUpstream({ ...upstream, port: Number(e.target.value) || 22 })} />
        </div>
        <div>
          <label className="mb-1 block text-xs text-slate-500">用户</label>
          <input className="input w-full" value={upstream.user} onChange={(e) => setUpstream({ ...upstream, user: e.target.value })} />
        </div>
        <div>
          <label className="mb-1 block text-xs text-slate-500">密码</label>
          <input className="input w-full" type="password" value={upstream.password} onChange={(e) => setUpstream({ ...upstream, password: e.target.value })} />
        </div>
      </div>
    );
  }
  if (upstream.type === "telnet" || upstream.type === "raw_tcp") {
    return (
      <div className="grid grid-cols-2 gap-2">
        <div>
          <label className="mb-1 block text-xs text-slate-500">主机</label>
          <input className="input w-full" value={upstream.host} onChange={(e) => setUpstream({ ...upstream, host: e.target.value })} />
        </div>
        <div>
          <label className="mb-1 block text-xs text-slate-500">端口</label>
          <input className="input w-full" type="number" value={upstream.port} onChange={(e) => setUpstream({ ...upstream, port: Number(e.target.value) || 0 })} />
        </div>
      </div>
    );
  }
  if (upstream.type === "serial") {
    return (
      <div className="grid grid-cols-3 gap-2">
        <div>
          <label className="mb-1 block text-xs text-slate-500">串口</label>
          <select className="input w-full" value={upstream.port} onChange={(e) => setUpstream({ ...upstream, port: e.target.value })}>
            {serialPorts.length === 0 && <option value="">暂无串口</option>}
            {serialPorts.map((p) => (
              <option key={p} value={p}>{p}</option>
            ))}
          </select>
        </div>
        <div>
          <label className="mb-1 block text-xs text-slate-500">波特率</label>
          <select className="input w-full" value={upstream.baud} onChange={(e) => setUpstream({ ...upstream, baud: Number(e.target.value) })}>
            {[9600, 19200, 38400, 57600, 115200, 230400, 460800, 921600].map((b) => (
              <option key={b} value={b}>{b}</option>
            ))}
          </select>
        </div>
        <div>
          <label className="mb-1 block text-xs text-slate-500">数据位</label>
          <select className="input w-full" value={upstream.data_bits} onChange={(e) => setUpstream({ ...upstream, data_bits: Number(e.target.value) as 5|6|7|8 })}>
            {[5, 6, 7, 8].map((b) => <option key={b} value={b}>{b}</option>)}
          </select>
        </div>
        <div>
          <label className="mb-1 block text-xs text-slate-500">校验</label>
          <select className="input w-full" value={upstream.parity} onChange={(e) => setUpstream({ ...upstream, parity: e.target.value })}>
            <option value="none">None</option>
            <option value="even">Even</option>
            <option value="odd">Odd</option>
          </select>
        </div>
        <div>
          <label className="mb-1 block text-xs text-slate-500">停止位</label>
          <select className="input w-full" value={upstream.stop_bits} onChange={(e) => setUpstream({ ...upstream, stop_bits: Number(e.target.value) as 1|2 })}>
            <option value="1">1</option>
            <option value="2">2</option>
          </select>
        </div>
        <div>
          <label className="mb-1 block text-xs text-slate-500">流控</label>
          <select className="input w-full" value={upstream.flow} onChange={(e) => setUpstream({ ...upstream, flow: e.target.value })}>
            <option value="none">None</option>
            <option value="hardware">Hardware</option>
            <option value="software">Software</option>
          </select>
        </div>
      </div>
    );
  }
  if (upstream.type === "local_shell") {
    return (
      <div>
        <label className="mb-1 block text-xs text-slate-500">命令</label>
        <input
          className="input w-full"
          value={upstream.command}
          onChange={(e) => setUpstream({ ...upstream, command: e.target.value, args: [] })}
        />
      </div>
    );
  }
  if (upstream.type === "http_proxy") {
    return (
      <div className="grid grid-cols-3 gap-2">
        <div>
          <label className="mb-1 block text-xs text-slate-500">监听地址</label>
          <input className="input w-full" value={upstream.listen} onChange={(e) => setUpstream({ ...upstream, listen: e.target.value })} />
        </div>
        <div>
          <label className="mb-1 block text-xs text-slate-500">目标地址</label>
          <input className="input w-full" value={upstream.target} onChange={(e) => setUpstream({ ...upstream, target: e.target.value })} />
        </div>
        <div>
          <label className="mb-1 block text-xs text-slate-500">协议</label>
          <select className="input w-full" value={upstream.protocol} onChange={(e) => setUpstream({ ...upstream, protocol: e.target.value })}>
            <option value="tcp">TCP</option>
            <option value="udp">UDP</option>
          </select>
        </div>
      </div>
    );
  }
  return null;
}

export default function App() {
  const [sessions, setSessions] = useState<SessionView[]>([]);
  const [server, setServer] = useState<ServerInfo | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [autoStart, setAutoStartState] = useState(false);

  const [selectedName, setSelectedName] = useState<string | null>(null);
  const [clients, setClients] = useState<ClientView[]>([]);

  // 新建会话弹窗
  const [showNew, setShowNew] = useState(false);
  const [newName, setNewName] = useState("");
  const [newPassword, setNewPassword] = useState("admin");
  const [newSshUser, setNewSshUser] = useState("admin");
  const [newListen, setNewListen] = useState("0.0.0.0:2222");
  const [newAutoRc, setNewAutoRc] = useState(true);
  const [newPtyOverride, setNewPtyOverride] = useState<{ cols: number; rows: number } | null>(null);
  const [upKind, setUpKind] = useState<UpKind>("ssh");
  const [upstream, setUpstream] = useState<UpstreamSpec>(() => defaultUpstream("ssh"));
  const [serialPorts, setSerialPorts] = useState<string[]>([]);

  // 编辑弹窗
  const [showEdit, setShowEdit] = useState(false);
  const [editName, setEditName] = useState("");
  const [editSshUser, setEditSshUser] = useState("admin");
  const [editListen, setEditListen] = useState("0.0.0.0:2222");
  const [editPassword, setEditPassword] = useState("");
  const [editAutoRc, setEditAutoRc] = useState(true);
  const [editUpKind, setEditUpKind] = useState<UpKind>("ssh");
  const [editUpstream, setEditUpstream] = useState<UpstreamSpec>(defaultUpstream("ssh"));

  const selected = useMemo(
    () => sessions.find((s) => s.name === selectedName) || null,
    [sessions, selectedName],
  );

  const refreshSessions = useCallback(async () => {
    try {
      setErr(null);
      const list = await listSessions();
      setSessions(list);
      if (selectedName && !list.find((s) => s.name === selectedName)) {
        setSelectedName(null);
      }
    } catch (e) {
      setErr(String(e));
    }
  }, [selectedName]);

  const refreshServer = useCallback(async () => {
    try {
      const info = await getServerInfo();
      setServer(info);
      setErr(null);
    } catch (e) {
      setErr(String(e));
    }
  }, []);

  const refreshClients = useCallback(async (name: string) => {
    try {
      setClients(await listClients(name));
    } catch {
      setClients([]);
    }
  }, []);

  useEffect(() => {
    void refreshSessions();
    void refreshServer();
    void getAutostart().then(setAutoStartState).catch(() => {});
    void invoke<string[]>("list_serial_ports").then(setSerialPorts).catch(() => {});
    // 每秒刷新会话列表（确保运行时间实时更新）
    const interval = setInterval(() => void refreshSessions(), 1000);
    return () => clearInterval(interval);
  }, [refreshSessions, refreshServer]);

  useEffect(() => {
    setUpstream(defaultUpstream(upKind));
  }, [upKind]);

  // 1Hz 刷新选中会话的下联
  useEffect(() => {
    if (!selectedName) {
      setClients([]);
      return;
    }
    void refreshClients(selectedName);
    const interval = setInterval(() => {
      void refreshClients(selectedName);
    }, 1000);
    return () => clearInterval(interval);
  }, [selectedName, refreshClients]);

  useEffect(() => {
    let alive = true;
    let drop: (() => void) | undefined;

    listen<StatusEvent>("session:status", (ev) => {
      const { name, status } = ev.payload;
      setSessions((prev) =>
        prev.map((row) => (row.name === name ? { ...row, status } : row)),
      );
    }).then((unlisten) => {
      if (alive) drop = unlisten;
      else unlisten();
    });

    return () => {
      alive = false;
      drop?.();
    };
  }, []);

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
    if (up.type === "local_shell") {
      up = { type: "local_shell", command: up.command, args: [] };
    }
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
    if (up.type === "local_shell") {
      up = { type: "local_shell", command: up.command, args: [] };
    }
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
    try { await navigator.clipboard.writeText(server.host_key_fpr); } catch { setErr("复制失败"); }
  }

  return (
    <div className="flex h-screen flex-col bg-slate-950 text-slate-100">
      {/* 顶部栏 */}
      <header className="flex items-center gap-4 border-b border-slate-800 px-4 py-2 text-sm">
        <h1 className="text-base font-semibold text-white">Termhub</h1>
        {server && (
          <>
            <button className="btn-sm" onClick={() => void copyFingerprint()}>复制指纹</button>
          </>
        )}
        <div className="flex-1" />
        <label className="flex items-center gap-1 text-xs text-slate-400">
          <input type="checkbox" checked={autoStart} onChange={() => void toggleAutostart()} />
          自启
        </label>
        <button className="btn-sm bg-cyan-600 text-white hover:bg-cyan-500" onClick={() => void refreshSessions()}>刷新</button>
      </header>

      {err && (
        <div className="border-b border-red-900/50 bg-red-950/30 px-4 py-1 text-xs text-red-300">
          {err}
        </div>
      )}

      {/* 主区域 */}
      <div className="flex flex-1 overflow-hidden">
        {/* 左栏：会话列表 */}
        <aside className="flex w-60 flex-col border-r border-slate-800">
          <div className="flex-1 overflow-y-auto p-2">
            {sessions.length === 0 && (
              <p className="p-4 text-center text-xs text-slate-500">暂无会话</p>
            )}
            {sessions.map((s) => (
              <button
                key={s.name}
                className={`mb-1 flex w-full items-center gap-2 rounded-lg px-3 py-2 text-left text-sm transition-colors ${
                  selectedName === s.name
                    ? "bg-slate-800 text-white"
                    : "text-slate-300 hover:bg-slate-900"
                }`}
                onClick={() => setSelectedName(s.name === selectedName ? null : s.name)}
              >
                <span className={`text-lg ${statusDot(s.status)}`}>●</span>
                <div className="min-w-0 flex-1">
                  <div className="truncate font-medium">{s.name}</div>
                  <div className="truncate text-xs text-slate-500">{upstreamLabel(s.upstream)}</div>
                </div>
                {s.client_count > 0 && (
                  <span className="rounded-full bg-slate-800 px-1.5 py-0.5 text-xs text-slate-400">
                    {s.client_count}
                  </span>
                )}
              </button>
            ))}
          </div>
          <div className="border-t border-slate-800 p-2">
            <button
              className="w-full rounded-lg bg-emerald-600 py-2 text-sm font-medium text-white hover:bg-emerald-500"
              onClick={() => {
                setShowNew(true);
                setUpKind("ssh");
                setUpstream(defaultUpstream("ssh"));
                setNewName("");
                setNewPassword("admin");
                setNewSshUser("admin");
                setNewListen("0.0.0.0:2222");
                setNewAutoRc(true);
                setNewPtyOverride(null);
              }}
            >
              + 新建会话
            </button>
          </div>
        </aside>

        {/* 右栏：详情 */}
        <main className="flex flex-1 flex-col overflow-hidden">
          {selected ? (
            <div className="flex flex-1 flex-col gap-4 overflow-auto p-4">
              {/* 头部信息 */}
              <div className="flex items-start justify-between">
                <div>
                  <h2 className="text-xl font-semibold text-white">{selected.name}</h2>
                  <div className="mt-1 flex items-center gap-3 text-sm text-slate-400">
                    <span className={statusDot(selected.status)}>
                      {formatStatus(selected.status)}
                    </span>
                    {selected.upstream_kind !== "http_proxy" && (
                      <>
                        <span>·</span>
                        <span>{selected.upstream_kind}</span>
                      </>
                    )}
                    <span>·</span>
                    <span className="font-mono text-xs text-slate-500">{selected.upstream_summary}</span>
                  </div>
                </div>
                <div className="flex gap-2">
                  {selected.status.state === "stopped" || selected.status.state === "failed" ? (
                    <button className="btn-sm bg-emerald-600 text-white hover:bg-emerald-500" onClick={() => void restartSession(selected.name).then(refreshSessions).catch((e) => setErr(String(e)))}>启动</button>
                  ) : (
                    <>
                      <button className="btn-sm" onClick={() => void restartSession(selected.name).then(refreshSessions).catch((e) => setErr(String(e)))}>重启</button>
                      <button className="btn-sm border-amber-800 text-amber-200 hover:bg-amber-950/50" onClick={() => void stopSession(selected.name).then(refreshSessions).catch((e) => setErr(String(e)))}>停止</button>
                    </>
                  )}
                  <button className="btn-sm" onClick={openEditDialog}>编辑</button>
                  <button className="btn-sm border-red-900 text-red-300 hover:bg-red-950/40" onClick={() => { setSelectedName(null); void deleteSession(selected.name).then(refreshSessions).catch((e) => setErr(String(e))); }}>删除</button>
                </div>
              </div>

              {/* SSH 接入信息（HTTP代理不显示） */}
              {selected.upstream_kind !== "http_proxy" && (
              <div className="rounded-lg border border-slate-800 bg-slate-900/50 p-3">
                <div className="mb-2 text-xs text-slate-500">SSH 接入</div>
                <code className="rounded bg-slate-800 px-2 py-1 text-sm text-cyan-300">
                  ssh {selected.ssh_user}@&lt;本机IP&gt; -p {selected.listen.split(":").pop() || "2222"}
                </code>
                <div className="mt-2 text-xs text-slate-500">用户: {selected.ssh_user} · 密码: ***</div>
              </div>
              )}

              {/* 下联列表 */}
              <div className="flex-1 overflow-auto rounded-lg border border-slate-800 bg-slate-900/50">
                <div className="border-b border-slate-800 px-3 py-2 text-xs text-slate-500">
                  下联 ({clients.length})
                </div>
                {clients.length === 0 ? (
                  <p className="p-4 text-center text-sm text-slate-600">暂无连接</p>
                ) : (
                  <table className="w-full text-left text-sm">
                    <thead>
                      <tr className="border-b border-slate-800 text-xs text-slate-500">
                        <th className="px-3 py-2">#</th>
                        <th className="px-3 py-2">远程</th>
                        <th className="px-3 py-2">时间</th>
                        <th className="px-3 py-2">入/出</th>
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
                          <td className="px-3 py-2">
                            <button className="rounded bg-slate-800 px-2 py-1 text-xs hover:bg-slate-700" onClick={() => void kickClient(selected.name, c.id).then(() => refreshClients(selected.name)).catch((e) => setErr(String(e)))}>踢</button>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                )}
              </div>
            </div>
          ) : (
            <div className="flex flex-1 items-center justify-center text-slate-600">
              选择左侧会话查看详情
            </div>
          )}
        </main>
      </div>

      {/* 新建会话弹窗 */}
      {showNew && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
          <div className="flex max-h-[90vh] w-full max-w-lg flex-col rounded-2xl border border-slate-700 bg-slate-900 p-6 shadow-2xl">
            <h3 className="mb-4 text-lg font-medium text-white">新建会话</h3>
            <form className="flex flex-1 flex-col gap-3 overflow-auto" onSubmit={(e) => void submitNew(e)}>
              <div>
                <label className="mb-1 block text-xs text-slate-500">会话名称</label>
                <input className="input w-full" required value={newName} onChange={(e) => setNewName(e.target.value)} />
              </div>
              <div>
                <label className="mb-1 block text-xs text-slate-500">上游类型</label>
                <select className="input w-full" value={upKind} onChange={(e) => setUpKind(e.target.value as UpKind)}>
                  <option value="ssh">SSH 客户端</option>
                  <option value="telnet">Telnet</option>
                  <option value="raw_tcp">原始 TCP</option>
                  <option value="serial">串口</option>
                  <option value="local_shell">本地 Shell</option>
                  <option value="http_proxy">端口转发</option>
                </select>
              </div>
              <div>
                <div className="mb-2 text-xs font-medium text-slate-400">上游参数</div>
                <UpstreamFields upstream={upstream} setUpstream={setUpstream} serialPorts={serialPorts} />
              </div>
              {upKind !== "http_proxy" && (
                <div className="border-t border-slate-800 pt-3">
                  <div className="mb-2 text-xs font-medium text-slate-400">下联（SSH 终端接入）</div>
                  <div className="flex flex-col gap-3">
                    <div className="grid grid-cols-2 gap-2">
                      <div>
                        <label className="mb-1 block text-xs text-slate-500">SSH 用户名</label>
                        <input className="input w-full" value={newSshUser} onChange={(e) => setNewSshUser(e.target.value)} />
                      </div>
                      <div>
                        <label className="mb-1 block text-xs text-slate-500">SSH 监听地址</label>
                        <input className="input w-full" value={newListen} onChange={(e) => setNewListen(e.target.value)} />
                      </div>
                    </div>
                    <div>
                      <label className="mb-1 block text-xs text-slate-500">SSH 密码</label>
                      <div className="flex gap-1">
                        <input className="input flex-1" value={newPassword} onChange={(e) => setNewPassword(e.target.value)} />
                        <button type="button" className="btn-sm" onClick={() => {
                          const chars = "abcdefghijkmnpqrstuvwxyz23456789";
                          setNewPassword(Array.from({ length: 8 }, () => chars[Math.floor(Math.random() * chars.length)]).join(""));
                        }}>随机</button>
                      </div>
                    </div>
                    <div className="flex flex-wrap items-center gap-3 text-sm">
                      <label className="flex items-center gap-1 text-slate-300">
                        <input type="checkbox" checked={newAutoRc} onChange={(e) => setNewAutoRc(e.target.checked)} /> 自动重连
                      </label>
                      <label className="flex items-center gap-1 text-slate-300">
                        <input type="checkbox" checked={newPtyOverride !== null} onChange={(e) => setNewPtyOverride(e.target.checked ? { cols: 80, rows: 24 } : null)} /> PTY 覆盖
                      </label>
                      {newPtyOverride && (
                        <div className="flex gap-1">
                          <input className="input w-16" type="number" value={newPtyOverride.cols} onChange={(e) => setNewPtyOverride({ ...newPtyOverride, cols: Number(e.target.value) || 80 })} />
                          <span className="self-center text-slate-500">×</span>
                          <input className="input w-16" type="number" value={newPtyOverride.rows} onChange={(e) => setNewPtyOverride({ ...newPtyOverride, rows: Number(e.target.value) || 24 })} />
                        </div>
                      )}
                    </div>
                  </div>
                </div>
              )}
              <div className="mt-auto flex justify-end gap-3 pt-4">
                <button type="button" className="btn-sm" onClick={() => setShowNew(false)}>取消</button>
                <button type="submit" className="btn-sm bg-emerald-600 text-white hover:bg-emerald-500">创建并启动</button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* 编辑弹窗 */}
      {showEdit && selected && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
          <div className="flex max-h-[90vh] w-full max-w-lg flex-col rounded-2xl border border-slate-700 bg-slate-900 p-6 shadow-2xl">
            <h3 className="mb-4 text-lg font-medium text-white">编辑会话</h3>
            <form className="flex flex-1 flex-col gap-3 overflow-auto" onSubmit={(e) => void applyEdit(e)}>
              <div>
                <label className="mb-1 block text-xs text-slate-500">会话名称</label>
                <input className="input w-full" required value={editName} onChange={(e) => setEditName(e.target.value)} />
              </div>
              <div>
                <label className="mb-1 block text-xs text-slate-500">上游类型</label>
                <select className="input w-full" value={editUpKind} onChange={(e) => {
                  const k = e.target.value as UpKind;
                  setEditUpKind(k);
                  setEditUpstream(defaultUpstream(k));
                }}>
                  <option value="ssh">SSH 客户端</option>
                  <option value="telnet">Telnet</option>
                  <option value="raw_tcp">原始 TCP</option>
                  <option value="serial">串口</option>
                  <option value="local_shell">本地 Shell</option>
                  <option value="http_proxy">端口转发</option>
                </select>
              </div>
              <div>
                <div className="mb-2 text-xs font-medium text-slate-400">上游参数</div>
                <UpstreamFields upstream={editUpstream} setUpstream={setEditUpstream} serialPorts={serialPorts} />
              </div>
              {editUpKind !== "http_proxy" && (
                <div className="border-t border-slate-800 pt-3">
                  <div className="mb-2 text-xs font-medium text-slate-400">下联（SSH 终端接入）</div>
                  <div className="flex flex-col gap-3">
                    <div className="grid grid-cols-2 gap-2">
                      <div>
                        <label className="mb-1 block text-xs text-slate-500">SSH 用户名</label>
                        <input className="input w-full" value={editSshUser} onChange={(e) => setEditSshUser(e.target.value)} />
                      </div>
                      <div>
                        <label className="mb-1 block text-xs text-slate-500">SSH 监听地址</label>
                        <input className="input w-full" value={editListen} onChange={(e) => setEditListen(e.target.value)} />
                      </div>
                    </div>
                    <div>
                      <label className="mb-1 block text-xs text-slate-500">密码（留空则不修改）</label>
                      <div className="flex gap-1">
                        <input className="input flex-1" placeholder="新密码" value={editPassword} onChange={(e) => setEditPassword(e.target.value)} />
                        <button type="button" className="btn-sm" onClick={() => {
                          const chars = "abcdefghijkmnpqrstuvwxyz23456789";
                          setEditPassword(Array.from({ length: 8 }, () => chars[Math.floor(Math.random() * chars.length)]).join(""));
                        }}>随机</button>
                      </div>
                    </div>
                    <label className="flex items-center gap-1 text-sm text-slate-300">
                      <input type="checkbox" checked={editAutoRc} onChange={(e) => setEditAutoRc(e.target.checked)} /> 自动重连
                    </label>
                  </div>
                </div>
              )}
              <div className="mt-auto flex justify-end gap-3 pt-4">
                <button type="button" className="btn-sm" onClick={() => setShowEdit(false)}>取消</button>
                <button type="submit" className="btn-sm bg-blue-600 text-white hover:bg-blue-500">保存</button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
}
