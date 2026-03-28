import { useState, useEffect } from "react";
import { getAllDevices, callDeviceAction, type DeviceInfo } from "../commands/devices";

function DeviceCard({ device, onToggle }: { device: DeviceInfo; onToggle: () => void }) {
  const isOn = device.state === "on" || device.state === "playing";
  const attrs = device.attributes as Record<string, unknown>;

  function stateText(): string {
    if (device.domain === "light") {
      if (!isOn) return "Off";
      const parts: string[] = [];
      if (typeof attrs.brightness === "number") {
        parts.push(`${Math.round((attrs.brightness as number / 255) * 100)}%`);
      }
      if (typeof attrs.color_temp === "number") {
        parts.push(`${attrs.color_temp} mireds`);
      }
      return parts.length > 0 ? parts.join(" \u00b7 ") : "On";
    }
    if (device.domain === "climate") {
      const parts: string[] = [];
      if (typeof attrs.current_temperature === "number") {
        parts.push(`${attrs.current_temperature}\u00b0`);
      }
      if (typeof attrs.hvac_action === "string") {
        parts.push(attrs.hvac_action as string);
      } else if (typeof attrs.hvac_mode === "string") {
        parts.push(attrs.hvac_mode as string);
      }
      return parts.length > 0 ? parts.join(" \u00b7 ") : device.state;
    }
    if (device.domain === "media_player") {
      if (device.state === "playing" && typeof attrs.media_title === "string") {
        return attrs.media_title as string;
      }
      return device.state.charAt(0).toUpperCase() + device.state.slice(1);
    }
    return isOn ? "On" : "Off";
  }

  function handleBrightness(e: React.ChangeEvent<HTMLInputElement>) {
    const brightness = Math.round((parseInt(e.target.value) / 100) * 255);
    callDeviceAction("light", "turn_on", device.entity_id, { brightness });
  }

  function handleVolume(e: React.ChangeEvent<HTMLInputElement>) {
    const volume_level = parseInt(e.target.value) / 100;
    callDeviceAction("media_player", "volume_set", device.entity_id, { volume_level });
  }

  function handleTemperature(delta: number) {
    const current = typeof attrs.temperature === "number" ? (attrs.temperature as number) : 72;
    callDeviceAction("climate", "set_temperature", device.entity_id, {
      temperature: current + delta,
    });
  }

  return (
    <div className={`device-card-large ${isOn ? "device-card-on" : ""}`}>
      <div className="device-card-header">
        <div>
          <div className="device-card-name">{device.friendly_name}</div>
          <div className="device-card-state">{stateText()}</div>
        </div>
        <label className="toggle-switch toggle-switch-sm" aria-label={`Toggle ${device.friendly_name}`}>
          <input
            type="checkbox"
            role="switch"
            aria-checked={isOn}
            checked={isOn}
            onChange={onToggle}
          />
          <span className="toggle-track" />
        </label>
      </div>

      {device.domain === "light" && isOn && typeof attrs.brightness === "number" && (
        <div className="device-card-control">
          <input
            type="range"
            className="device-slider"
            min="0"
            max="100"
            value={Math.round(((attrs.brightness as number) / 255) * 100)}
            onChange={handleBrightness}
          />
        </div>
      )}

      {device.domain === "media_player" && (
        <div className="device-card-control device-card-media">
          <button className="device-media-btn" onClick={() => callDeviceAction("media_player", "media_play", device.entity_id)}>&#9654;</button>
          <button className="device-media-btn" onClick={() => callDeviceAction("media_player", "media_pause", device.entity_id)}>&#10074;&#10074;</button>
          <button className="device-media-btn" onClick={() => callDeviceAction("media_player", "media_stop", device.entity_id)}>&#9632;</button>
          {typeof attrs.volume_level === "number" && (
            <input
              type="range"
              className="device-slider device-slider-volume"
              min="0"
              max="100"
              value={Math.round((attrs.volume_level as number) * 100)}
              onChange={handleVolume}
            />
          )}
        </div>
      )}

      {device.domain === "climate" && isOn && (
        <div className="device-card-control device-card-climate">
          <button className="device-temp-btn" onClick={() => handleTemperature(-1)}>-</button>
          <span className="device-temp-display">
            {typeof attrs.temperature === "number" ? `${attrs.temperature}\u00b0` : "--"}
          </span>
          <button className="device-temp-btn" onClick={() => handleTemperature(1)}>+</button>
        </div>
      )}
    </div>
  );
}

export default function DevicesView({ onNavigate }: { onNavigate?: (view: string) => void }) {
  const [devices, setDevices] = useState<DeviceInfo[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    loadDevices();
  }, []);

  async function loadDevices() {
    try {
      const list = await getAllDevices();
      setDevices(list);
    } catch {
      setDevices([]);
    }
    setLoading(false);
  }

  function handleToggle(device: DeviceInfo) {
    const isOn = device.state === "on" || device.state === "playing";
    const service = isOn ? "turn_off" : "turn_on";
    // Optimistic update
    setDevices((prev) =>
      prev.map((d) =>
        d.entity_id === device.entity_id
          ? { ...d, state: isOn ? "off" : "on" }
          : d,
      ),
    );
    callDeviceAction(device.domain, service, device.entity_id).catch(() => {
      // Revert on failure
      loadDevices();
    });
  }

  // Group by room
  const grouped = new Map<string, DeviceInfo[]>();
  for (const device of devices) {
    const room = device.room || "Unassigned";
    if (!grouped.has(room)) grouped.set(room, []);
    grouped.get(room)!.push(device);
  }

  if (loading) {
    return (
      <div className="devices-view">
        <div className="devices-empty">
          <p className="devices-empty-subtitle">Loading devices...</p>
        </div>
      </div>
    );
  }

  if (devices.length === 0) {
    return (
      <div className="devices-view">
        <div className="devices-empty">
          <p className="devices-empty-title">No devices connected</p>
          <p className="devices-empty-subtitle">
            Connect to Home Assistant in Settings to discover and control your
            smart home devices.
          </p>
          <button
            className="devices-empty-btn"
            onClick={() => onNavigate?.("settings")}
          >
            Open Settings
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="devices-view">
      {Array.from(grouped.entries()).map(([room, roomDevices]) => (
        <div key={room} className="devices-room-section">
          <h4 className="devices-room-label">{room}</h4>
          <div className="devices-grid">
            {roomDevices.map((device) => (
              <DeviceCard
                key={device.entity_id}
                device={device}
                onToggle={() => handleToggle(device)}
              />
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}
