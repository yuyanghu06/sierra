import { invoke } from "@tauri-apps/api/core";

export interface AppConfig {
  ha_url: string | null;
  ha_token: string | null;
  ollama_url: string | null;
  ollama_model: string | null;
}

export async function getConfig(): Promise<AppConfig> {
  return invoke<AppConfig>("get_config");
}

export async function saveConfig(config: AppConfig): Promise<void> {
  return invoke("save_config", { config });
}

export async function testHaConnection(
  url: string,
  token: string,
): Promise<boolean> {
  return invoke<boolean>("test_ha_connection", { url, token });
}

export async function getActiveModel(): Promise<string | null> {
  return invoke<string | null>("get_active_model");
}
