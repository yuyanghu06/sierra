import { useState, useEffect, useRef } from "react";
import { checkOllamaHealth } from "../commands/chat";
import { getConfig, saveConfig, testHaConnection } from "../commands/config";
import { pullModel, type PullProgressEvent } from "../commands/setup";

/* Curated list of models the user can install from within Sierra */
const CURATED_MODELS: {
  id: string;
  name: string;
  params: string;
  size: string;
  description: string;
  recommended?: boolean;
}[] = [
  { id: "qwen3.5:4b",    name: "Qwen 3.5",     params: "4B",  size: "2.6 GB", description: "Fast tool calling. Works on most systems.",                       recommended: true },
  { id: "llama3.2:3b",   name: "Llama 3.2",    params: "3B",  size: "2.0 GB", description: "Meta's compact model. Great on constrained hardware." },
  { id: "llama3.2:1b",   name: "Llama 3.2",    params: "1B",  size: "1.3 GB", description: "Smallest option. For systems with 8 GB RAM or less." },
  { id: "gemma3:4b",     name: "Gemma 3",      params: "4B",  size: "3.3 GB", description: "Google's efficient 4B model." },
  { id: "phi4-mini:3.8b",name: "Phi-4 Mini",   params: "3.8B",size: "2.5 GB", description: "Microsoft's compact reasoning model." },
  { id: "mistral:7b",    name: "Mistral 7B",   params: "7B",  size: "4.1 GB", description: "Strong instruction following. Needs 16 GB RAM." },
  { id: "llama3.1:8b",   name: "Llama 3.1",    params: "8B",  size: "4.7 GB", description: "Meta's flagship 8B. Excellent tool calling. 16 GB RAM." },
  { id: "qwen2.5:14b",   name: "Qwen 2.5",     params: "14B", size: "9.0 GB", description: "High quality at 14B. 32 GB+ RAM recommended." },
  { id: "llama3.3:70b",  name: "Llama 3.3",    params: "70B", size: "43 GB",  description: "Top quality. Requires 64 GB RAM." },
];

export default function SettingsView() {
  const [haUrl, setHaUrl] = useState("http://localhost:8123");
  const [haToken, setHaToken] = useState("");
  const [ollamaUrl, setOllamaUrl] = useState("http://localhost:11434");
  const [installedModels, setInstalledModels] = useState<string[]>([]);
  const [activeModel, setActiveModel] = useState<string | null>(null);
  const [ollamaHealthy, setOllamaHealthy] = useState<boolean | null>(null);
  const [haTestResult, setHaTestResult] = useState<import("../commands/config").HaConnectionStatus | null>(null);
  const [tokenSaved, setTokenSaved] = useState(false);
  const [saving, setSaving] = useState(false);
  const [saveSuccess, setSaveSuccess] = useState(false);
  const [reduceTransparency, setReduceTransparency] = useState(false);

  // Per-model install state
  const [installing, setInstalling] = useState<Record<string, string>>({}); // modelId → progress string
  const [installErrors, setInstallErrors] = useState<Record<string, string>>({});

  // Track saved token value to know when it's changed
  const savedTokenRef = useRef("");

  useEffect(() => {
    loadConfig();
    checkHealth();
  }, []);

  useEffect(() => {
    document.documentElement.classList.toggle("reduce-transparency", reduceTransparency);
  }, [reduceTransparency]);

  async function loadConfig() {
    try {
      const cfg = await getConfig();
      if (cfg.ha_url) setHaUrl(cfg.ha_url);
      if (cfg.ha_token) { setHaToken(cfg.ha_token); savedTokenRef.current = cfg.ha_token; }
      if (cfg.ollama_url) setOllamaUrl(cfg.ollama_url);
      if (cfg.ollama_model) setActiveModel(cfg.ollama_model);
    } catch {
      // use defaults
    }
    try {
      const { listModels } = await import("../commands/chat");
      const list = await listModels();
      setInstalledModels(list);
    } catch {
      setInstalledModels([]);
    }
  }

  async function checkHealth() {
    try {
      const healthy = await checkOllamaHealth();
      setOllamaHealthy(healthy);
    } catch {
      setOllamaHealthy(false);
    }
  }

  // Auto-save HA token when the field loses focus, if value changed
  async function handleTokenBlur() {
    if (haToken === savedTokenRef.current) return;
    setTokenSaved(false);
    try {
      const cfg = await getConfig();
      await saveConfig({ ...cfg, ha_url: haUrl, ha_token: haToken || null });
      savedTokenRef.current = haToken;
      setTokenSaved(true);
      setTimeout(() => setTokenSaved(false), 2500);
      // Re-test connection with the new token
      if (haToken.trim()) {
        const result = await testHaConnection(haUrl, haToken.trim());
        setHaTestResult(result);
      }
    } catch {
      // silently ignore — user can still press Save Settings
    }
  }

  async function handleTestHa() {
    setHaTestResult(null);
    try {
      const result = await testHaConnection(haUrl, haToken);
      setHaTestResult(result);
    } catch {
      setHaTestResult({ status: "unreachable" });
    }
  }

  async function handleInstallModel(modelId: string) {
    setInstalling((s) => ({ ...s, [modelId]: "Starting…" }));
    setInstallErrors((s) => { const n = { ...s }; delete n[modelId]; return n; });

    try {
      await pullModel(modelId, (event: PullProgressEvent) => {
        if (event.event === "downloading") {
          setInstalling((s) => ({ ...s, [modelId]: `${Math.round(event.data.percent)}%` }));
        } else if (event.event === "verifying") {
          setInstalling((s) => ({ ...s, [modelId]: "Verifying…" }));
        } else if (event.event === "completed") {
          setInstalling((s) => { const n = { ...s }; delete n[modelId]; return n; });
        } else if (event.event === "failed") {
          setInstallErrors((s) => ({ ...s, [modelId]: event.data.error }));
          setInstalling((s) => { const n = { ...s }; delete n[modelId]; return n; });
        }
      });

      // Refresh installed list and set as active model
      const { listModels } = await import("../commands/chat");
      const list = await listModels();
      setInstalledModels(list);
      // Auto-select this model if none active
      setActiveModel((prev) => prev ?? modelId);
    } catch (e) {
      setInstallErrors((s) => ({ ...s, [modelId]: String(e) }));
      setInstalling((s) => { const n = { ...s }; delete n[modelId]; return n; });
    }
  }

  async function handleActivateModel(modelId: string) {
    setActiveModel(modelId);
    try {
      const cfg = await getConfig();
      await saveConfig({ ...cfg, ollama_model: modelId });
    } catch {
      // ignore
    }
  }

  async function handleSave() {
    setSaving(true);
    setSaveSuccess(false);
    try {
      await saveConfig({
        ha_url: haUrl || null,
        ha_token: haToken || null,
        ollama_url: ollamaUrl || null,
        ollama_model: activeModel || null,
      });
      savedTokenRef.current = haToken;
      setSaveSuccess(true);
      setTimeout(() => setSaveSuccess(false), 3000);
      checkHealth();
    } catch {
      // save failed
    }
    setSaving(false);
  }

  return (
    <div className="settings-view">
      {/* Home Assistant */}
      <div className="settings-section">
        <h3 className="settings-section-title">Home Assistant</h3>

        <div className="settings-field">
          <label className="settings-field-label" htmlFor="ha-url">Server URL</label>
          <input
            id="ha-url"
            className="form-input"
            type="url"
            value={haUrl}
            onChange={(e) => setHaUrl(e.target.value)}
            placeholder="http://localhost:8123"
          />
        </div>

        <div className="settings-field">
          <label className="settings-field-label" htmlFor="ha-token">
            Long-Lived Access Token
          </label>
          <input
            id="ha-token"
            className="form-input"
            type="password"
            value={haToken}
            onChange={(e) => { setHaToken(e.target.value); setTokenSaved(false); setHaTestResult(null); }}
            onBlur={handleTokenBlur}
            placeholder="Paste your token here"
          />
          <p className="settings-field-hint">
            Generate in Home Assistant under <strong>Profile → Security → Long-Lived Access Tokens</strong>.
            Token saves automatically when you leave this field.
          </p>
          {tokenSaved && (
            <div className="settings-token-saved">
              <span className="health-dot health-dot-ok" /> Token saved
            </div>
          )}
        </div>

        <div className="settings-btn-row">
          <button className="form-btn form-btn-primary" onClick={handleTestHa}>
            Test Connection
          </button>
          {haTestResult !== null && (() => {
            const { status } = haTestResult;
            const ok = status === "connected";
            const warn = status === "needsOnboarding";
            const dotClass = ok ? "health-dot-ok" : warn ? "health-dot-loading" : "health-dot-error";
            const label =
              status === "connected" ? "Connected"
              : status === "needsOnboarding" ? `Setup required — open ${haUrl} in your browser, create an account, then generate a Long-Lived Access Token`
              : status === "invalidToken" ? "Invalid token — check your Long-Lived Access Token"
              : "Home Assistant is not reachable at this URL";
            return (
              <span className={`settings-status ${warn ? "settings-status-wrap" : ""}`}>
                <span className={`health-dot ${dotClass}`} />
                <span>{label}</span>
              </span>
            );
          })()}
        </div>
      </div>

      {/* Ollama */}
      <div className="settings-section">
        <h3 className="settings-section-title">Ollama</h3>

        <div className="settings-field">
          <label className="settings-field-label" htmlFor="ollama-url">Server URL</label>
          <input
            id="ollama-url"
            className="form-input"
            type="url"
            value={ollamaUrl}
            onChange={(e) => setOllamaUrl(e.target.value)}
            placeholder="http://localhost:11434"
          />
          <div className="settings-status">
            <span className={`health-dot ${ollamaHealthy === true ? "health-dot-ok" : ollamaHealthy === false ? "health-dot-error" : "health-dot-loading"}`} />
            <span>{ollamaHealthy === true ? "Connected" : ollamaHealthy === false ? "Not reachable" : "Checking..."}</span>
          </div>
        </div>

        {/* Model list */}
        <div className="settings-field">
          <label className="settings-field-label">Models</label>
          <p className="settings-field-hint">Active model is used for all chat. Install others to switch later.</p>
          <div className="settings-model-list">
            {CURATED_MODELS.map((m) => {
              const isInstalled = installedModels.includes(m.id);
              const isActive = activeModel === m.id;
              const progress = installing[m.id];
              const error = installErrors[m.id];
              return (
                <div key={m.id} className={`settings-model-row-item ${isActive ? "settings-model-row-active" : ""}`}>
                  <div className="settings-model-info">
                    <span className="settings-model-name">
                      {m.name}
                      {m.recommended && <span className="settings-model-badge">Recommended</span>}
                    </span>
                    <span className="settings-model-meta">{m.params} · {m.size}</span>
                    <span className="settings-model-desc">{m.description}</span>
                    {error && <span className="settings-model-error">{error}</span>}
                    {progress && <span className="settings-model-progress">{progress}</span>}
                  </div>
                  <div className="settings-model-actions">
                    {isInstalled ? (
                      isActive ? (
                        <span className="settings-model-active-label">Active</span>
                      ) : (
                        <button className="form-btn form-btn-sm" onClick={() => handleActivateModel(m.id)}>
                          Use
                        </button>
                      )
                    ) : (
                      <button
                        className="form-btn form-btn-primary form-btn-sm"
                        onClick={() => handleInstallModel(m.id)}
                        disabled={!!progress}
                      >
                        {progress ? progress : "Install"}
                      </button>
                    )}
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      </div>

      {/* Save */}
      <div className="settings-section">
        <div className="settings-btn-row">
          <button className="form-btn form-btn-primary" onClick={handleSave} disabled={saving}>
            {saving ? "Saving…" : "Save Settings"}
          </button>
          {saveSuccess && (
            <span className="settings-status">
              <span className="health-dot health-dot-ok" />
              <span>Saved</span>
            </span>
          )}
        </div>
      </div>

      {/* Appearance */}
      <div className="settings-section">
        <h3 className="settings-section-title">Appearance</h3>

        <div className="settings-row">
          <div>
            <p className="settings-row-label">Reduce transparency</p>
            <p className="settings-row-description">
              Replace glass effects with solid warm surfaces
            </p>
          </div>
          <label className="toggle-switch" aria-label="Reduce transparency">
            <input
              type="checkbox"
              role="switch"
              aria-checked={reduceTransparency}
              checked={reduceTransparency}
              onChange={(e) => setReduceTransparency(e.target.checked)}
            />
            <span className="toggle-track" />
          </label>
        </div>
      </div>
    </div>
  );
}
