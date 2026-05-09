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
  setAutostart,
  setListenAddr,
  stopSession,
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
  | "local_shell";

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

  const [newName, setNewName] = useState("");
  const [newPassword, setNewPassword] = useState("pass");
  const [newAutoRc, setNewAutoRc] = useState(true);
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
        <div className="grid gap-2 sm:grid-cols-2">
          <label className="flex flex-col gap-1 text-sm">
            <span className="text-slate-400">端口</span>
            <input
              className="rounded-lg border border-slate-700 bg-slate-900 px-3 py-2"
              value={upstream.port}
              onChange={(e) => setUpstream({ ...upstream, port: e.target.value })}
            />
          </label>
          <label className="flex flex-col gap-1 text-sm">
            <span className="text-slate-400">波特率</span>
            <input
              type="number"
              className="rounded-lg border border-slate-700 bg-slate-900 px-3 py-2"
              value={upstream.baud}
              onChange={(e) =>
                setUpstream({ ...upstream, baud: Number(e.target.value) || 9600 })
              }
            />
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
      pty_override: null,
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
                <div className="mt-1 font-mono text-xs text-slate-300">{server.host_key_fpr}</div>
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
              <p className="text-xs text-slate-500">
                修改监听地址目前仅更新内存中的展示值；要让 SSH server 实际换绑请重启应用（后续可接热重载）。
              </p>
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
                <input
                  className="rounded-lg border border-slate-700 bg-slate-900 px-3 py-2"
                  value={newPassword}
                  onChange={(e) => setNewPassword(e.target.value)}
                />
              </label>
              <label className="flex items-center gap-2 pt-6 text-sm text-slate-300">
                <input
                  type="checkbox"
                  checked={newAutoRc}
                  onChange={(e) => setNewAutoRc(e.target.checked)}
                />
                自动重连
              </label>
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
                                    in {c.bytes_in} · out {c.bytes_out}
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
      </div>
    </div>
  );
}
