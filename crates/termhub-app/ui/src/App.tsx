import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { FormEvent } from "react";
import { Fragment, useCallback, useEffect, useMemo, useState } from "react";

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
  setListenAddr,
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
      return `运行中 (${s.uptime_secs}s)`;
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

type UpKind =
  | "loopback"
  | "ssh"
  | "telnet"
  | "raw_tcp"
  | "serial"
  | "local_shell"
  | "http_proxy";

function formatConnectedAt(secs: number): string {
  const d = new Date(secs * 1000);
  return d.toLocaleTimeString();
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
      return { type: "local_shell", command: "cmd.exe", args: ["/c", "echo hello"] };
    case "http_proxy":
      return { type: "http_proxy", listen: "0.0.0.0:8080", target: "127.0.0.1:80" };
  }
}

export default function App() {
  const [sessions, setSessions] = useState<SessionView[]>([]);
  const [server, setServer] = useState<ServerInfo | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [listenDraft, setListenDraft] = useState("");
  const [autoStart, setAutoStartState] = useState(false);

  const [expanded, setExpanded] = useState<string | null>(null);
  const [clients, setClients] = useState<ClientView[]>([]);
  const [editingSession, setEditingSession] = useState<string | null>(null);
  const [editPassword, setEditPassword] = useState("");
  const [editAutoRc, setEditAutoRc] = useState(true);

  const [newName, setNewName] = useState("");
  const [newPassword, setNewPassword] = useState("pass");
  const [newAutoRc, setNewAutoRc] = useState(true);
  const [newPtyOverride, setNewPtyOverride] = useState<{ cols: number; rows: number } | null>(null);
  const [upKind, setUpKind] = useState<UpKind>("loopback");
  const [upstream, setUpstream] = useState<UpstreamSpec>(() => defaultUpstream("loopback"));
  const [shellArgsText, setShellArgsText] = useState("/c\necho hello");

  const refreshSessions = useCallback(async () => {
    try {
      setErr(null);
      setSessions(await listSessions());
    } catch (e) {
      setErr(String(e));
    }
  }, []);

  const refreshServer = useCallback(async () => {
    try {
      const info = await getServerInfo();
      setServer(info);
      setListenDraft(info.listen);
      setErr(null);
    } catch (e) {
      setErr(String(e));
    }
  }, []);

  useEffect(() => {
    void refreshSessions();
    void refreshServer();
    void getAutostart().then(setAutoStartState).catch(() => {});
  }, [refreshSessions, refreshServer]);

  useEffect(() => {
    setUpstream(defaultUpstream(upKind));
    if (upKind === "local_shell") {
      setShellArgsText("/c\necho hello");
    }
  }, [upKind]);

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

  // 1Hz client list refresh when a session is expanded
  useEffect(() => {
    if (!expanded) return;
    // Initial fetch
    void listClients(expanded).then(setClients).catch(() => {});
    const interval = setInterval(() => {
      void listClients(expanded).then(setClients).catch(() => {});
    }, 1000);
    return () => {
      clearInterval(interval);
    };
  }, [expanded]);

  const upstreamFields = useMemo(() => {
    if (upstream.type === "ssh") {
      return (
        <div className="grid gap-2 sm:grid-cols-2">
          <label className="flex flex-col gap-1 text-sm">
            <span className="text-slate-400">主机</span>
            <input
              className="rounded-lg border border-slate-700 bg-slate-900 px-3 py-2"
              value={upstream.host}
              onChange={(e) => setUpstream({ ...upstream, host: e.target.value })}
            />
          </label>
          <label className="flex flex-col gap-1 text-sm">
            <span className="text-slate-400">端口</span>
            <input
              type="number"
              className="rounded-lg border border-slate-700 bg-slate-900 px-3 py-2"
              value={upstream.port}
              onChange={(e) =>
                setUpstream({ ...upstream, port: Number(e.target.value) || 22 })
              }
            />
          </label>
          <label className="flex flex-col gap-1 text-sm">
            <span className="text-slate-400">用户</span>
            <input
              className="rounded-lg border border-slate-700 bg-slate-900 px-3 py-2"
              value={upstream.user}
              onChange={(e) => setUpstream({ ...upstream, user: e.target.value })}
            />
          </label>
          <label className="flex flex-col gap-1 text-sm">
            <span className="text-slate-400">密码</span>
            <input
              type="password"
              className="rounded-lg border border-slate-700 bg-slate-900 px-3 py-2"
              value={upstream.password}
              onChange={(e) => setUpstream({ ...upstream, password: e.target.value })}
            />
          </label>
        </div>
      );
    }
    if (upstream.type === "telnet" || upstream.type === "raw_tcp") {
      return (
        <div className="grid gap-2 sm:grid-cols-2">
          <label className="flex flex-col gap-1 text-sm">
            <span className="text-slate-400">主机</span>
            <input
              className="rounded-lg border border-slate-700 bg-slate-900 px-3 py-2"
              value={upstream.host}
              onChange={(e) => setUpstream({ ...upstream, host: e.target.value })}
            />
          </label>
          <label className="flex flex-col gap-1 text-sm">
            <span className="text-slate-400">端口</span>
            <input
              type="number"
              className="rounded-lg border border-slate-700 bg-slate-900 px-3 py-2"
              value={upstream.port}
              onChange={(e) =>
                setUpstream({ ...upstream, port: Number(e.target.value) || 0 })
              }
            />
          </label>
        </div>
      );
    }
    if (upstream.type === "serial") {
      return (
        <div className="grid gap-2 sm:grid-cols-3">
          <label className="flex flex-col gap-1 text-sm">
            <span className="text-slate-400">端口</span>
            <div className="flex gap-1">
              <input
                className="flex-1 rounded-lg border border-slate-700 bg-slate-900 px-3 py-2"
                value={upstream.port}
                onChange={(e) => setUpstream({ ...upstream, port: e.target.value })}
                placeholder="COM3"
              />
              <button
                type="button"
                className="rounded-lg border border-slate-600 px-2 text-xs hover:bg-slate-800"
                onClick={async () => {
                  try {
                    const ports: string[] = await invoke("list_serial_ports");
                    if (ports.length > 0) {
                      setUpstream({ ...upstream, port: ports[0] });
                    }
                  } catch { /* ignore */ }
                }}
              >
                扫描
              </button>
            </div>
          </label>
          <label className="flex flex-col gap-1 text-sm">
            <span className="text-slate-400">波特率</span>
            <select
              className="rounded-lg border border-slate-700 bg-slate-900 px-3 py-2"
              value={upstream.baud}
              onChange={(e) => setUpstream({ ...upstream, baud: Number(e.target.value) })}
            >
              {[9600, 19200, 38400, 57600, 115200, 230400, 460800, 921600].map((b) => (
                <option key={b} value={b}>{b}</option>
              ))}
            </select>
          </label>
          <label className="flex flex-col gap-1 text-sm">
            <span className="text-slate-400">数据位</span>
            <select
              className="rounded-lg border border-slate-700 bg-slate-900 px-3 py-2"
              value={upstream.data_bits}
              onChange={(e) => setUpstream({ ...upstream, data_bits: Number(e.target.value) as 5|6|7|8 })}
            >
              {[5, 6, 7, 8].map((b) => <option key={b} value={b}>{b}</option>)}
            </select>
          </label>
          <label className="flex flex-col gap-1 text-sm">
            <span className="text-slate-400">校验</span>
            <select
              className="rounded-lg border border-slate-700 bg-slate-900 px-3 py-2"
              value={upstream.parity}
              onChange={(e) => setUpstream({ ...upstream, parity: e.target.value })}
            >
              <option value="none">None</option>
              <option value="even">Even</option>
              <option value="odd">Odd</option>
            </select>
          </label>
          <label className="flex flex-col gap-1 text-sm">
            <span className="text-slate-400">停止位</span>
            <select
              className="rounded-lg border border-slate-700 bg-slate-900 px-3 py-2"
              value={upstream.stop_bits}
              onChange={(e) => setUpstream({ ...upstream, stop_bits: Number(e.target.value) as 1|2 })}
            >
              <option value={1}>1</option>
              <option value={2}>2</option>
            </select>
          </label>
          <label className="flex flex-col gap-1 text-sm">
            <span className="text-slate-400">流控</span>
            <select
              className="rounded-lg border border-slate-700 bg-slate-900 px-3 py-2"
              value={upstream.flow}
              onChange={(e) => setUpstream({ ...upstream, flow: e.target.value })}
            >
              <option value="none">None</option>
              <option value="hardware">Hardware (RTS/CTS)</option>
              <option value="software">Software (XON/XOFF)</option>
            </select>
          </label>
          <label className="flex flex-col gap-1 text-sm">
            <span className="text-slate-400">输入行结束</span>
            <select
              className="rounded-lg border border-slate-700 bg-slate-900 px-3 py-2"
              value={upstream.input_eol}
              onChange={(e) => setUpstream({ ...upstream, input_eol: e.target.value })}
            >
              <option value="as_is">不改</option>
              <option value="cr_to_crnl">\r → \r\n</option>
              <option value="cr_to_nl">\r → \n</option>
            </select>
          </label>
          <label className="flex flex-col gap-1 text-sm">
            <span className="text-slate-400">输出行结束</span>
            <select
              className="rounded-lg border border-slate-700 bg-slate-900 px-3 py-2"
              value={upstream.output_eol}
              onChange={(e) => setUpstream({ ...upstream, output_eol: e.target.value })}
            >
              <option value="as_is">不改</option>
              <option value="cr_to_crnl">\r → \r\n</option>
            </select>
          </label>
        </div>
      );
    }
    if (upstream.type === "local_shell") {
      return (
        <div className="grid gap-2">
          <label className="flex flex-col gap-1 text-sm">
            <span className="text-slate-400">命令</span>
            <input
              className="rounded-lg border border-slate-700 bg-slate-900 px-3 py-2"
              value={upstream.command}
              onChange={(e) => setUpstream({ ...upstream, command: e.target.value })}
            />
          </label>
          <label className="flex flex-col gap-1 text-sm">
            <span className="text-slate-400">参数（每行一项）</span>
            <textarea
              rows={4}
              className="rounded-lg border border-slate-700 bg-slate-900 px-3 py-2 font-mono text-xs"
              value={shellArgsText}
              onChange={(e) => {
                const t = e.target.value;
                setShellArgsText(t);
                const args = t.split(/\r?\n/).filter(Boolean);
                setUpstream((prev) =>
                  prev.type === "local_shell" ? { ...prev, args } : prev,
                );
              }}
            />
          </label>
        </div>
      );
    }
    if (upstream.type === "http_proxy") {
      return (
        <div className="grid gap-2 sm:grid-cols-2">
          <label className="flex flex-col gap-1 text-sm">
            <span className="text-slate-400">本地监听地址</span>
            <input
              className="rounded-lg border border-slate-700 bg-slate-900 px-3 py-2"
              value={upstream.listen}
              onChange={(e) => setUpstream({ ...upstream, listen: e.target.value })}
              placeholder="0.0.0.0:8080"
            />
          </label>
          <label className="flex flex-col gap-1 text-sm">
            <span className="text-slate-400">目标地址</span>
            <input
              className="rounded-lg border border-slate-700 bg-slate-900 px-3 py-2"
              value={upstream.target}
              onChange={(e) => setUpstream({ ...upstream, target: e.target.value })}
              placeholder="192.168.1.100:80"
            />
          </label>
        </div>
      );
    }
    return <p className="text-sm text-slate-500">无需额外字段。</p>;
  }, [upstream, shellArgsText]);

  async function expandSession(name: string) {
    if (expanded === name) {
      setExpanded(null);
      return;
    }
    setExpanded(name);
    try {
      setClients(await listClients(name));
      setErr(null);
    } catch (e) {
      setErr(String(e));
    }
  }

  async function submitNew(ev: FormEvent) {
    ev.preventDefault();
    let up = upstream;
    if (up.type === "local_shell") {
      up = {
        type: "local_shell",
        command: up.command,
        args: shellArgsText.split(/\r?\n/).filter(Boolean),
      };
    }
    const cfg: SessionConfig = {
      name: newName.trim(),
      password: newPassword,
      auto_reconnect: newAutoRc,
      pty_override: newPtyOverride,
      upstream: up,
    };
    try {
      await createSession(cfg);
      setNewName("");
      await refreshSessions();
      setErr(null);
    } catch (e) {
      setErr(String(e));
    }
  }

  async function applyListen() {
    try {
      await setListenAddr(listenDraft.trim());
      await refreshServer();
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

  async function switchToLocalhost() {
    if (!server) return;
    const port = server.listen.split(":").pop() || "2222";
    try {
      await setListenAddr(`127.0.0.1:${port}`);
      await refreshServer();
    } catch (e) {
      setErr(String(e));
    }
  }

  async function copyFingerprint() {
    if (!server) return;
    try {
      await navigator.clipboard.writeText(server.host_key_fpr);
    } catch {
      // fallback: select text
      setErr("复制失败，请手动选中复制");
    }
  }

  const isPublicListen = server ? server.listen.startsWith("0.0.0.0") : false;

  return (
    <div className="min-h-screen bg-slate-950 px-4 py-6 text-slate-100">
      <div className="mx-auto flex max-w-5xl flex-col gap-6">
        <header className="flex flex-wrap items-end justify-between gap-4">
          <div>
            <h1 className="text-2xl font-semibold tracking-tight text-white">Termhub</h1>
            <p className="text-sm text-slate-400">本地 SSH 汇聚 · 会话与下联管理</p>
          </div>
          <button
            type="button"
            className="rounded-lg bg-cyan-600 px-4 py-2 text-sm font-medium text-white hover:bg-cyan-500"
            onClick={() => void refreshSessions()}
          >
            刷新会话
          </button>
        </header>

        {err && (
          <div className="rounded-xl border border-red-900/80 bg-red-950/40 px-4 py-3 text-sm text-red-200">
            {err}
          </div>
        )}

        {server && isPublicListen && (
          <div className="flex flex-wrap items-center gap-3 rounded-xl border border-amber-800/80 bg-amber-950/40 px-4 py-3 text-sm text-amber-200">
            <span className="font-medium">
              监听 {server.listen} — 局域网内任何人知道密码即可接入
            </span>
            <button
              type="button"
              className="rounded-lg border border-amber-600 px-3 py-1 text-xs font-medium hover:bg-amber-900/50"
              onClick={() => void switchToLocalhost()}
            >
              改为仅本机
            </button>
            <button
              type="button"
              className="rounded-lg border border-amber-600 px-3 py-1 text-xs font-medium hover:bg-amber-900/50"
              onClick={() => void copyFingerprint()}
            >
              复制指纹
            </button>
          </div>
        )}

        <section className="rounded-2xl border border-slate-800 bg-slate-900/60 p-6 shadow-xl">
          <h2 className="mb-4 text-lg font-medium text-white">服务器</h2>
          {server && (
            <div className="grid gap-3 text-sm">
              <div className="flex flex-wrap gap-2">
                <span className="text-slate-400">监听（内存）</span>
                <code className="rounded bg-slate-800 px-2 py-0.5 text-cyan-300">
                  {server.listen}
                </code>
              </div>
              <div className="break-all">
                <span className="text-slate-400">Host key SHA256</span>
                <div className="mt-1 flex items-center gap-2">
                  <span className="font-mono text-xs text-slate-300">{server.host_key_fpr}</span>
                  <button
                    type="button"
                    className="rounded border border-slate-600 px-2 py-0.5 text-xs hover:bg-slate-800"
                    onClick={() => void copyFingerprint()}
                  >
                    复制
                  </button>
                </div>
              </div>
              <div className="mt-2 flex flex-wrap items-center gap-3">
                <input
                  className="min-w-[200px] flex-1 rounded-lg border border-slate-700 bg-slate-950 px-3 py-2 font-mono text-xs"
                  value={listenDraft}
                  onChange={(e) => setListenDraft(e.target.value)}
                  placeholder="例 0.0.0.0:2222"
                />
                <button
                  type="button"
                  className="rounded-lg border border-slate-600 px-3 py-2 text-sm hover:bg-slate-800"
                  onClick={() => void applyListen()}
                >
                  更新监听地址
                </button>
                <label className="flex cursor-pointer items-center gap-2 text-sm text-slate-300">
                  <input type="checkbox" checked={autoStart} onChange={() => void toggleAutostart()} />
                  开机自启
                </label>
              </div>
            </div>
          )}
        </section>

        <section className="rounded-2xl border border-slate-800 bg-slate-900/60 p-6 shadow-xl">
          <h2 className="mb-4 text-lg font-medium text-white">新建会话</h2>
          <form className="flex flex-col gap-4" onSubmit={(e) => void submitNew(e)}>
            <div className="grid gap-3 sm:grid-cols-3">
              <label className="flex flex-col gap-1 text-sm">
                <span className="text-slate-400">名称</span>
                <input
                  required
                  className="rounded-lg border border-slate-700 bg-slate-900 px-3 py-2"
                  value={newName}
                  onChange={(e) => setNewName(e.target.value)}
                  placeholder="例如 demo"
                />
              </label>
              <label className="flex flex-col gap-1 text-sm">
                <span className="text-slate-400">SSH 密码（下联登录）</span>
                <div className="flex gap-1">
                  <input
                    className="flex-1 rounded-lg border border-slate-700 bg-slate-900 px-3 py-2"
                    value={newPassword}
                    onChange={(e) => setNewPassword(e.target.value)}
                  />
                  <button
                    type="button"
                    className="rounded-lg border border-slate-600 px-2 text-xs hover:bg-slate-800"
                    onClick={() => {
                      const chars = "abcdefghijkmnpqrstuvwxyz23456789";
                      const arr = Array.from({ length: 8 }, () => chars[Math.floor(Math.random() * chars.length)]);
                      setNewPassword(arr.join(""));
                    }}
                  >
                    随机
                  </button>
                </div>
              </label>
              <label className="flex items-center gap-2 pt-6 text-sm text-slate-300">
                <input
                  type="checkbox"
                  checked={newAutoRc}
                  onChange={(e) => setNewAutoRc(e.target.checked)}
                />
                自动重连
              </label>
              <label className="flex items-center gap-2 pt-6 text-sm text-slate-300">
                <input
                  type="checkbox"
                  checked={newPtyOverride !== null}
                  onChange={(e) =>
                    setNewPtyOverride(e.target.checked ? { cols: 80, rows: 24 } : null)
                  }
                />
                PTY 覆盖
              </label>
              {newPtyOverride && (
                <>
                  <label className="flex flex-col gap-1 text-sm">
                    <span className="text-slate-400">列</span>
                    <input
                      type="number"
                      className="rounded-lg border border-slate-700 bg-slate-900 px-3 py-2"
                      value={newPtyOverride.cols}
                      onChange={(e) =>
                        setNewPtyOverride({ ...newPtyOverride, cols: Number(e.target.value) || 80 })
                      }
                    />
                  </label>
                  <label className="flex flex-col gap-1 text-sm">
                    <span className="text-slate-400">行</span>
                    <input
                      type="number"
                      className="rounded-lg border border-slate-700 bg-slate-900 px-3 py-2"
                      value={newPtyOverride.rows}
                      onChange={(e) =>
                        setNewPtyOverride({ ...newPtyOverride, rows: Number(e.target.value) || 24 })
                      }
                    />
                  </label>
                </>
              )}
            </div>

            <label className="flex flex-col gap-1 text-sm">
              <span className="text-slate-400">上联类型</span>
              <select
                className="rounded-lg border border-slate-700 bg-slate-900 px-3 py-2"
                value={upKind}
                onChange={(e) => setUpKind(e.target.value as UpKind)}
              >
                <option value="loopback">loopback（环回测试）</option>
                <option value="ssh">SSH 客户端</option>
                <option value="telnet">Telnet</option>
                <option value="raw_tcp">原始 TCP</option>
                <option value="serial">串口</option>
                <option value="local_shell">本地 Shell（PTY）</option>
                <option value="http_proxy">HTTP 代理（端口转发）</option>
              </select>
            </label>

            {upstreamFields}

            <button
              type="submit"
              className="w-fit rounded-lg bg-emerald-600 px-4 py-2 text-sm font-medium text-white hover:bg-emerald-500"
            >
              创建并启动
            </button>
          </form>
        </section>

        <section className="rounded-2xl border border-slate-800 bg-slate-900/60 p-6 shadow-xl">
          <h2 className="mb-4 text-lg font-medium text-white">会话列表</h2>
          <div className="overflow-x-auto">
            <table className="w-full border-collapse text-left text-sm">
              <thead>
                <tr className="border-b border-slate-800 text-slate-400">
                  <th className="py-2 pr-4">名称</th>
                  <th className="py-2 pr-4">状态</th>
                  <th className="py-2 pr-4">上联</th>
                  <th className="py-2 pr-4">下联数</th>
                  <th className="py-2">操作</th>
                </tr>
              </thead>
              <tbody>
                {sessions.map((s) => (
                  <Fragment key={s.name}>
                    <tr className="border-b border-slate-800/80">
                      <td className="py-3 pr-4 font-medium text-white">{s.name}</td>
                      <td className="py-3 pr-4 text-slate-300">{formatStatus(s.status)}</td>
                      <td className="py-3 pr-4">
                        <div className="text-slate-400">{s.upstream_kind}</div>
                        <div className="font-mono text-xs text-slate-500">{s.upstream_summary}</div>
                      </td>
                      <td className="py-3 pr-4">{s.client_count}</td>
                      <td className="py-3">
                        <div className="flex flex-wrap gap-2">
                          <button
                            type="button"
                            className="rounded border border-slate-600 px-2 py-1 text-xs hover:bg-slate-800"
                            onClick={() => void expandSession(s.name)}
                          >
                            {expanded === s.name ? "收起" : "下联"}
                          </button>
                          <button
                            type="button"
                            className="rounded border border-emerald-800 px-2 py-1 text-xs text-emerald-200 hover:bg-emerald-950/50"
                            onClick={() =>
                              void restartSession(s.name)
                                .then(refreshSessions)
                                .catch((e) => setErr(String(e)))
                            }
                          >
                            重启
                          </button>
                          <button
                            type="button"
                            className="rounded border border-blue-800 px-2 py-1 text-xs text-blue-200 hover:bg-blue-950/50"
                            onClick={() => {
                              setEditingSession(s.name);
                              setEditPassword(s.name === editingSession ? editPassword : "");
                              setEditAutoRc(s.auto_reconnect);
                            }}
                          >
                            编辑
                          </button>
                          <button
                            type="button"
                            className="rounded border border-amber-800 px-2 py-1 text-xs text-amber-200 hover:bg-amber-950/50"
                            onClick={() =>
                              void stopSession(s.name).then(refreshSessions).catch((e) => setErr(String(e)))
                            }
                          >
                            停止
                          </button>
                          <button
                            type="button"
                            className="rounded border border-red-900 px-2 py-1 text-xs text-red-300 hover:bg-red-950/40"
                            onClick={() =>
                              void deleteSession(s.name).then(refreshSessions).catch((e) => setErr(String(e)))
                            }
                          >
                            删除
                          </button>
                        </div>
                      </td>
                    </tr>
                    {expanded === s.name && (
                      <tr className="bg-slate-950/50">
                        <td colSpan={5} className="px-2 py-3">
                          <div className="text-xs text-slate-400">下联</div>
                          {clients.length === 0 ? (
                            <p className="py-2 text-sm text-slate-500">暂无连接</p>
                          ) : (
                            <ul className="mt-2 space-y-2">
                              {clients.map((c) => (
                                <li
                                  key={c.id}
                                  className="flex flex-wrap items-center justify-between gap-2 rounded-lg border border-slate-800 bg-slate-900 px-3 py-2 text-xs"
                                >
                                  <span className="font-mono text-slate-300">
                                    #{c.id} {c.remote}
                                  </span>
                                  <span className="text-slate-500">
                                    {formatConnectedAt(c.connected_at_secs)} · in {c.bytes_in} · out {c.bytes_out}
                                  </span>
                                  <button
                                    type="button"
                                    className="rounded bg-slate-800 px-2 py-1 hover:bg-slate-700"
                                    onClick={() =>
                                      void kickClient(s.name, c.id)
                                        .then(() => expandSession(s.name))
                                        .catch((e) => setErr(String(e)))
                                    }
                                  >
                                    踢掉
                                  </button>
                                </li>
                              ))}
                            </ul>
                          )}
                        </td>
                      </tr>
                    )}
                  </Fragment>
                ))}
              </tbody>
            </table>
          </div>
        </section>

        {editingSession && (
          <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
            <div className="w-full max-w-md rounded-2xl border border-slate-700 bg-slate-900 p-6 shadow-2xl">
              <h3 className="mb-4 text-lg font-medium text-white">编辑会话: {editingSession}</h3>
              <form
                className="flex flex-col gap-4"
                onSubmit={(ev) => {
                  ev.preventDefault();
                  const s = sessions.find((x) => x.name === editingSession);
                  if (!s) return;
                  const cfg: SessionConfig = {
                    name: editingSession,
                    password: editPassword || "pass",
                    auto_reconnect: editAutoRc,
                    pty_override: null,
                    upstream: s.upstream,
                  };
                  void updateSession(editingSession, cfg)
                    .then(() => {
                      setEditingSession(null);
                      void refreshSessions();
                    })
                    .catch((e) => setErr(String(e)));
                }}
              >
                <label className="flex flex-col gap-1 text-sm">
                  <span className="text-slate-400">SSH 密码</span>
                  <input
                    className="rounded-lg border border-slate-700 bg-slate-950 px-3 py-2"
                    value={editPassword}
                    onChange={(e) => setEditPassword(e.target.value)}
                    placeholder="留空则不变"
                  />
                </label>
                <label className="flex items-center gap-2 text-sm text-slate-300">
                  <input
                    type="checkbox"
                    checked={editAutoRc}
                    onChange={(e) => setEditAutoRc(e.target.checked)}
                  />
                  自动重连
                </label>
                <div className="flex justify-end gap-3">
                  <button
                    type="button"
                    className="rounded-lg border border-slate-600 px-4 py-2 text-sm hover:bg-slate-800"
                    onClick={() => setEditingSession(null)}
                  >
                    取消
                  </button>
                  <button
                    type="submit"
                    className="rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-500"
                  >
                    保存
                  </button>
                </div>
              </form>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
