import { useState, useEffect, useCallback } from "react";
import { checkOllamaHealth } from "./commands/chat";
import { getAllDevices, callDeviceAction, getDeviceCount, checkHaHealth, type DeviceInfo } from "./commands/devices";
import { getActiveModel } from "./commands/config";
import { checkDependencies } from "./commands/setup";
import ChatView from "./views/ChatView";
import DevicesView from "./views/DevicesView";
import SettingsView from "./views/SettingsView";
import SetupView from "./views/SetupView";

type View = "chat" | "devices" | "settings";
type PanelTab = "active" | "all" | "rooms";

const VIEW_LABELS: Record<View, string> = {
  chat: "Chat",
  devices: "Devices",
  settings: "Settings",
};

function DevicePanelCard({
  device,
  onToggle,
}: {
  device: DeviceInfo;
  onToggle: () => void;
}) {
  const isOn = device.state === "on" || device.state === "playing";

  function stateText(): string {
    const attrs = device.attributes as Record<string, unknown>;
    if (device.domain === "light" && isOn) {
      if (typeof attrs.brightness === "number") {
        return `${Math.round(((attrs.brightness as number) / 255) * 100)}%`;
      }
      return "On";
    }
    if (device.domain === "climate") {
      if (typeof attrs.current_temperature === "number") {
        return `${attrs.current_temperature}\u00b0`;
      }
    }
    if (device.domain === "media_player" && device.state === "playing") {
      if (typeof attrs.media_title === "string") return attrs.media_title as string;
    }
    return isOn ? "On" : "Off";
  }

  return (
    <div className={`device-panel-card ${isOn ? "device-panel-card-on" : ""}`}>
      <div className="device-panel-card-row">
        <span className="device-panel-card-name">{device.friendly_name}</span>
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
      <span className="device-panel-card-state">{stateText()}</span>
    </div>
  );
}

function App() {
  const [setupComplete, setSetupComplete] = useState<boolean | null>(null);
  const [activeView, setActiveView] = useState<View>("chat");
  const [ollamaHealthy, setOllamaHealthy] = useState<boolean | null>(null);
  const [haHealthy, setHaHealthy] = useState<boolean | null>(null);
  const [activeModel, setActiveModel] = useState<string | null>(null);
  const [deviceCount, setDeviceCount] = useState(0);
  const [panelDevices, setPanelDevices] = useState<DeviceInfo[]>([]);
  const [panelTab, setPanelTab] = useState<PanelTab>("active");
  const [devicePanelOpen, setDevicePanelOpen] = useState(() => window.innerWidth >= 1100);

  const pollHealth = useCallback(async () => {
    try {
      const healthy = await checkOllamaHealth();
      setOllamaHealthy(healthy);
    } catch {
      setOllamaHealthy(false);
    }
    try {
      const healthy = await checkHaHealth();
      setHaHealthy(healthy);
    } catch {
      setHaHealthy(false);
    }
    try {
      const model = await getActiveModel();
      setActiveModel(model);
    } catch {
      setActiveModel(null);
    }
  }, []);

  const loadDevices = useCallback(async () => {
    try {
      const count = await getDeviceCount();
      setDeviceCount(count);
      const devices = await getAllDevices();
      setPanelDevices(devices);
    } catch {
      setDeviceCount(0);
      setPanelDevices([]);
    }
  }, []);

  useEffect(() => {
    Promise.all([checkDependencies(), import("./commands/config").then(m => m.getConfig())])
      .then(([status, cfg]) => {
        console.log("[setup] dependencies:", status);
        const depsReady = status.ollamaInstalled && status.homeAssistantInstalled;
        const configured = !!(cfg.ha_token && cfg.ollama_model);
        setSetupComplete(depsReady && configured);
      })
      .catch((e) => {
        console.error("[setup] check failed:", e);
        setSetupComplete(false);
      });
  }, []);

  useEffect(() => {
    if (setupComplete !== true) return;
    pollHealth();
    loadDevices();
    const interval = setInterval(() => {
      pollHealth();
      loadDevices();
    }, 30000);
    const handleFocus = () => {
      pollHealth();
      loadDevices();
    };
    window.addEventListener("focus", handleFocus);
    return () => {
      clearInterval(interval);
      window.removeEventListener("focus", handleFocus);
    };
  }, [setupComplete, pollHealth, loadDevices]);

  useEffect(() => {
    const handleResize = () => {
      if (window.innerWidth < 1100) {
        setDevicePanelOpen(false);
      }
    };
    window.addEventListener("resize", handleResize);
    return () => window.removeEventListener("resize", handleResize);
  }, []);

  function handlePanelToggle(device: DeviceInfo) {
    const isOn = device.state === "on" || device.state === "playing";
    const service = isOn ? "turn_off" : "turn_on";
    // Optimistic update
    setPanelDevices((prev) =>
      prev.map((d) =>
        d.entity_id === device.entity_id
          ? { ...d, state: isOn ? "off" : "on" }
          : d,
      ),
    );
    callDeviceAction(device.domain, service, device.entity_id).then(loadDevices).catch(loadDevices);
  }

  // Filter panel devices by tab
  let filteredDevices = panelDevices;
  if (panelTab === "active") {
    filteredDevices = panelDevices.filter(
      (d) => d.state !== "off" && d.state !== "unavailable",
    );
  }

  // Group by room for rooms tab
  const grouped = new Map<string, DeviceInfo[]>();
  if (panelTab === "rooms") {
    for (const d of panelDevices) {
      const room = d.room || "Unassigned";
      if (!grouped.has(room)) grouped.set(room, []);
      grouped.get(room)!.push(d);
    }
  }

  // Show loading while checking dependencies
  if (setupComplete === null) {
    return (
      <div className="app-shell">
        <div className="ambient-bg">
          <div className="ambient-orb ambient-orb-1" />
          <div className="ambient-orb ambient-orb-2" />
        </div>
        <div className="setup-loading">
          <img src="/sierra-logo.png" alt="Sierra" width="48" height="48" />
          <p className="setup-loading-text">Loading...</p>
        </div>
      </div>
    );
  }

  // Show setup wizard if dependencies are missing
  if (setupComplete === false) {
    return (
      <div className="app-shell">
        <div className="ambient-bg">
          <div className="ambient-orb ambient-orb-1" />
          <div className="ambient-orb ambient-orb-2" />
          <div className="ambient-orb ambient-orb-3" />
        </div>
        <SetupView onComplete={() => setSetupComplete(true)} />
      </div>
    );
  }

  return (
    <div className="app-shell">
      {/* Ambient background */}
      <div className="ambient-bg">
        <div className="ambient-orb ambient-orb-1" />
        <div className="ambient-orb ambient-orb-2" />
        <div className="ambient-orb ambient-orb-3" />
        <div className="ambient-orb ambient-orb-4" />
      </div>

      {/* Sidebar */}
      <aside className="sidebar">
        <div className="sidebar-header">
          <img className="sidebar-logo" src="/sierra-logo.png" alt="Sierra" width="16" height="16" />
          <span className="sidebar-app-name">Sierra</span>
        </div>

        <nav className="sidebar-nav">
          {([
            { id: "chat" as View, label: "Chat", icon: ICON_CHAT },
            { id: "devices" as View, label: "Devices", icon: ICON_DEVICES },
            { id: "settings" as View, label: "Settings", icon: ICON_SETTINGS },
          ]).map((item) => (
            <button
              key={item.id}
              className={`sidebar-nav-item ${activeView === item.id ? "sidebar-nav-item-active" : ""}`}
              onClick={() => setActiveView(item.id)}
            >
              <span
                className="sidebar-nav-icon"
                dangerouslySetInnerHTML={{ __html: item.icon }}
              />
              <span className="sidebar-nav-label">{item.label}</span>
            </button>
          ))}
        </nav>

        <div className="sidebar-health">
          <div className="health-item">
            <span
              className={`health-dot ${
                ollamaHealthy === true
                  ? "health-dot-ok"
                  : ollamaHealthy === false
                    ? "health-dot-error"
                    : "health-dot-loading"
              }`}
            />
            <span className="health-label">Ollama</span>
          </div>
          <div className="health-item">
            <span
              className={`health-dot ${
                haHealthy === true
                  ? "health-dot-ok"
                  : haHealthy === false
                    ? "health-dot-error"
                    : "health-dot-loading"
              }`}
            />
            <span className="health-label">Home Assistant</span>
          </div>
          <div className="health-item">
            <span
              className={`health-dot ${
                ollamaHealthy === true
                  ? "health-dot-ok"
                  : "health-dot-loading"
              }`}
            />
            <span className="health-label">{activeModel || "No model"}</span>
          </div>
        </div>
      </aside>

      {/* Main Area */}
      <main className="main-area">
        <div className="titlebar">
          <span className="titlebar-context">{VIEW_LABELS[activeView]}</span>
          <div className="titlebar-actions">
            <span className="titlebar-pill">{deviceCount} device{deviceCount !== 1 ? "s" : ""}</span>
            <button
              className="titlebar-btn"
              onClick={() => setDevicePanelOpen((o) => !o)}
              aria-label={devicePanelOpen ? "Hide device panel" : "Show device panel"}
              dangerouslySetInnerHTML={{ __html: ICON_PANEL }}
            />
          </div>
        </div>

        <div className="main-content">
          {activeView === "chat" && <ChatView />}
          {activeView === "devices" && <DevicesView onNavigate={(v) => setActiveView(v as View)} />}
          {activeView === "settings" && <SettingsView />}
        </div>
      </main>

      {/* Device Panel */}
      <aside className={`device-panel ${devicePanelOpen ? "" : "device-panel-collapsed"}`}>
        <div className="device-panel-header">
          <div className="device-panel-tabs">
            {(["active", "all", "rooms"] as PanelTab[]).map((tab) => (
              <button
                key={tab}
                className={`device-panel-tab ${panelTab === tab ? "device-panel-tab-active" : ""}`}
                onClick={() => setPanelTab(tab)}
              >
                {tab.charAt(0).toUpperCase() + tab.slice(1)}
              </button>
            ))}
          </div>
        </div>
        <div className="device-panel-content">
          {panelDevices.length === 0 ? (
            <div className="device-panel-empty">
              <span
                className="device-panel-empty-icon"
                dangerouslySetInnerHTML={{ __html: ICON_DEVICES_LARGE }}
              />
              <p className="device-panel-empty-title">No devices</p>
              <p className="device-panel-empty-subtitle">
                Connect to Home Assistant to see your devices here
              </p>
            </div>
          ) : panelTab === "rooms" ? (
            Array.from(grouped.entries()).map(([room, devices]) => (
              <div key={room} className="device-panel-room">
                <div className="device-panel-room-label">{room}</div>
                {devices.map((d) => (
                  <DevicePanelCard
                    key={d.entity_id}
                    device={d}
                    onToggle={() => handlePanelToggle(d)}
                  />
                ))}
              </div>
            ))
          ) : filteredDevices.length === 0 ? (
            <div className="device-panel-empty">
              <p className="device-panel-empty-subtitle">
                {panelTab === "active" ? "No active devices" : "No devices"}
              </p>
            </div>
          ) : (
            filteredDevices.map((d) => (
              <DevicePanelCard
                key={d.entity_id}
                device={d}
                onToggle={() => handlePanelToggle(d)}
              />
            ))
          )}
        </div>
      </aside>
    </div>
  );
}

/* Lucide-style inline SVGs — 1.5px stroke */

const ICON_CHAT = `<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg>`;

const ICON_DEVICES = `<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/></svg>`;

const ICON_SETTINGS = `<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09a1.65 1.65 0 0 0-1.08-1.51 1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.32 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>`;

const ICON_PANEL = `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2"/><line x1="15" y1="3" x2="15" y2="21"/></svg>`;

const ICON_DEVICES_LARGE = `<svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/></svg>`;

export default App;
