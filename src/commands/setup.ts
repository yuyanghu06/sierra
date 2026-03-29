import { invoke, Channel } from "@tauri-apps/api/core";
export { testHaConnection, type HaConnectionStatus } from "./config";

export interface DependencyStatus {
  ollamaInstalled: boolean;
  ollamaVersion: string | null;
  homeAssistantInstalled: boolean;
  haVersion: string | null;
  pythonAvailable: boolean;
  pythonVersion: string | null;
  rustAvailable: boolean;
  rustVersion: string | null;
}

export type InstallProgressEvent =
  | { event: "started"; data: { service: string } }
  | { event: "downloading"; data: { percent: number } }
  | { event: "installing" }
  | { event: "configuring" }
  | { event: "completed" }
  | { event: "failed"; data: { error: string } };

export type PullProgressEvent =
  | { event: "downloading"; data: { percent: number; totalBytes: number } }
  | { event: "verifying" }
  | { event: "completed" }
  | { event: "failed"; data: { error: string } };

export interface ServiceStatusInfo {
  ollama: ServiceStatus;
  homeAssistant: ServiceStatus;
}

export type ServiceStatus =
  | { status: "notInstalled" }
  | { status: "installed" }
  | { status: "starting" }
  | { status: "running" }
  | { status: "stopping" }
  | { status: "crashed"; exitCode: number | null; restarts: number }
  | { status: "external" };

export async function checkDependencies(): Promise<DependencyStatus> {
  return invoke("check_dependencies");
}

export async function installOllama(
  onProgress: (event: InstallProgressEvent) => void,
): Promise<void> {
  const channel = new Channel<InstallProgressEvent>();
  channel.onmessage = onProgress;
  return invoke("install_ollama", { onProgress: channel });
}

export async function installPython(
  onProgress: (event: InstallProgressEvent) => void,
): Promise<void> {
  const channel = new Channel<InstallProgressEvent>();
  channel.onmessage = onProgress;
  return invoke("install_python", { onProgress: channel });
}

export async function installRust(
  onProgress: (event: InstallProgressEvent) => void,
): Promise<void> {
  const channel = new Channel<InstallProgressEvent>();
  channel.onmessage = onProgress;
  return invoke("install_rust", { onProgress: channel });
}

export async function installHomeAssistant(
  onProgress: (event: InstallProgressEvent) => void,
): Promise<void> {
  const channel = new Channel<InstallProgressEvent>();
  channel.onmessage = onProgress;
  return invoke("install_home_assistant", { onProgress: channel });
}

export async function pullModel(
  modelName: string,
  onProgress: (event: PullProgressEvent) => void,
): Promise<void> {
  const channel = new Channel<PullProgressEvent>();
  channel.onmessage = onProgress;
  return invoke("pull_model", { modelName, onProgress: channel });
}

export async function getServiceStatus(): Promise<ServiceStatusInfo> {
  return invoke("get_service_status");
}

export async function restartService(service: string): Promise<void> {
  return invoke("restart_service", { service });
}

export async function startServices(): Promise<ServiceStatusInfo> {
  return invoke("start_services");
}
