import { invoke } from "@tauri-apps/api/core";

import type {
  ClientView,
  ServerInfo,
  SessionConfig,
  SessionView,
} from "./types";

export async function listSessions(): Promise<SessionView[]> {
  return invoke<SessionView[]>("list_sessions");
}

export async function createSession(config: SessionConfig): Promise<void> {
  return invoke("create_session", { args: { config } });
}

export async function stopSession(name: string): Promise<void> {
  return invoke("stop_session", { name });
}

export async function deleteSession(name: string): Promise<void> {
  return invoke("delete_session", { name });
}

export async function restartSession(name: string): Promise<void> {
  return invoke("restart_session", { name });
}

export async function updateSession(name: string, config: SessionConfig): Promise<void> {
  return invoke("update_session", { args: { name, config } });
}

export async function listClients(name: string): Promise<ClientView[]> {
  return invoke("list_clients", { name });
}

export async function kickClient(session: string, clientId: number): Promise<boolean> {
  return invoke("kick_client", { session, clientId });
}

export async function getServerInfo(): Promise<ServerInfo> {
  return invoke("get_server_info");
}

export async function getAutostart(): Promise<boolean> {
  return invoke("get_autostart");
}

export async function setAutostart(enable: boolean): Promise<void> {
  return invoke("set_autostart", { enable });
}

export async function exitApp(): Promise<void> {
  return invoke("exit_app");
}
