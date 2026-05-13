/** Mirrors backend serde shapes (`SessionStatus`, `UpstreamSpec`, `SessionConfig`). */

export type SessionStatus =
  | { state: "idle" }
  | { state: "starting" }
  | { state: "running"; uptime_secs: number }
  | { state: "reconnecting"; attempt: number }
  | { state: "failed"; reason: string }
  | { state: "stopped" };

export type UpstreamSpec =
  | { type: "loopback" }
  | {
      type: "serial";
      port: string;
      baud: number;
      data_bits: number;
      parity: string;
      stop_bits: number;
      flow: string;
      input_eol: string;
      output_eol: string;
    }
  | {
      type: "ssh";
      host: string;
      port: number;
      user: string;
      password: string;
    }
  | { type: "telnet"; host: string; port: number }
  | { type: "raw_tcp"; host: string; port: number }
  | { type: "local_shell"; command: string; args: string[] }
  | { type: "http_proxy"; listen: string; target: string; protocol: string };

export interface SessionConfig {
  name: string;
  password: string;
  ssh_user: string;
  listen: string;
  auto_reconnect: boolean;
  pty_override: { cols: number; rows: number } | null;
  upstream: UpstreamSpec;
}

export interface SessionView {
  name: string;
  ssh_user: string;
  listen: string;
  status: SessionStatus;
  upstream_kind: string;
  upstream_summary: string;
  upstream: UpstreamSpec;
  auto_reconnect: boolean;
  client_count: number;
}

export interface ClientView {
  id: number;
  remote: string;
  connected_at_secs: number;
  bytes_in: number;
  bytes_out: number;
}

export interface ServerInfo {
  host_key_fpr: string;
}

export interface StatusEvent {
  name: string;
  status: SessionStatus;
}
