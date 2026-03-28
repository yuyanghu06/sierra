import { useState, useEffect, useCallback } from "react";
import { checkOllamaHealth } from "./commands/chat";
import ChatView from "./views/ChatView";
import DevicesView from "./views/DevicesView";
import SettingsView from "./views/SettingsView";

type View = "chat" | "devices" | "settings";

const VIEW_LABELS: Record<View, string> = {
  chat: "Chat",
  devices: "Devices",
  settings: "Settings",
};

function App() {
  const [activeView, setActiveView] = useState<View>("chat");
  const [ollamaHealthy, setOllamaHealthy] = useState<boolean | null>(null);
  const [devicePanelOpen, setDevicePanelOpen] = useState(() => window.innerWidth >= 1100);

  const pollHealth = useCallback(async () => {
    try {
      const healthy = await checkOllamaHealth();
      setOllamaHealthy(healthy);
    } catch {
      setOllamaHealthy(false);
    }
  }, []);

  useEffect(() => {
    pollHealth();
    const interval = setInterval(pollHealth, 30000);
    const handleFocus = () => pollHealth();
    window.addEventListener("focus", handleFocus);
    return () => {
      clearInterval(interval);
      window.removeEventListener("focus", handleFocus);
    };
  }, [pollHealth]);

  useEffect(() => {
    const handleResize = () => {
      if (window.innerWidth < 1100) {
        setDevicePanelOpen(false);
      }
    };
    window.addEventListener("resize", handleResize);
    return () => window.removeEventListener("resize", handleResize);
  }, []);

  return (
    <div className="app-shell">
      {/* Ambient background — sunrise orbs */}
      <div className="ambient-bg">
        <div className="ambient-orb ambient-orb-1" />
        <div className="ambient-orb ambient-orb-2" />
        <div className="ambient-orb ambient-orb-3" />
        <div className="ambient-orb ambient-orb-4" />
      </div>

      {/* Sidebar — Glass Subtle */}
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
            <span className="health-dot health-dot-loading" />
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
            <span className="health-label">qwen3.5:4b</span>
          </div>
        </div>
      </aside>

      {/* Main Area */}
      <main className="main-area">
        <div className="titlebar">
          <span className="titlebar-context">{VIEW_LABELS[activeView]}</span>
          <div className="titlebar-actions">
            <span className="titlebar-pill">0 devices</span>
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
          {activeView === "devices" && <DevicesView />}
          {activeView === "settings" && <SettingsView />}
        </div>
      </main>

      {/* Device Panel — Glass Subtle */}
      <aside className={`device-panel ${devicePanelOpen ? "" : "device-panel-collapsed"}`}>
        <div className="device-panel-header">
          <div className="device-panel-tabs">
            <button className="device-panel-tab device-panel-tab-active">Active</button>
            <button className="device-panel-tab">All</button>
            <button className="device-panel-tab">Rooms</button>
          </div>
        </div>
        <div className="device-panel-content">
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
