import { invoke } from "@tauri-apps/api/core";

export interface PingResponse {
  status: string;
  version: string;
}

export interface AppInfo {
  name: string;
  version: string;
  platform: string;
}

export async function ping(): Promise<PingResponse> {
  return invoke<PingResponse>("ping");
}

export async function getAppInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("get_app_info");
}
