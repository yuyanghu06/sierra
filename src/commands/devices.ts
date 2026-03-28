import { invoke } from "@tauri-apps/api/core";

export interface DeviceInfo {
  entity_id: string;
  domain: string;
  friendly_name: string;
  state: string;
  attributes: Record<string, unknown>;
  room: string | null;
}

export interface RoomInfo {
  name: string;
  entity_ids: string[];
}

export async function getAllDevices(): Promise<DeviceInfo[]> {
  return invoke<DeviceInfo[]>("get_all_devices");
}

export async function getDeviceState(entityId: string): Promise<DeviceInfo> {
  return invoke<DeviceInfo>("get_device_state", { entityId });
}

export async function getRooms(): Promise<RoomInfo[]> {
  return invoke<RoomInfo[]>("get_rooms");
}

export async function getDeviceCount(): Promise<number> {
  return invoke<number>("get_device_count");
}

export async function callDeviceAction(
  domain: string,
  service: string,
  entityId: string,
  data?: Record<string, unknown>,
): Promise<void> {
  return invoke("call_device_action", { domain, service, entityId, data });
}

export async function checkHaHealth(): Promise<boolean> {
  return invoke<boolean>("check_ha_health");
}

export async function refreshDevices(): Promise<DeviceInfo[]> {
  return invoke<DeviceInfo[]>("refresh_devices");
}
