import { useState, useEffect } from "react";
import { listModels, checkOllamaHealth } from "../commands/chat";
import { getConfig, saveConfig, testHaConnection } from "../commands/config";

export default function SettingsView() {
  const [haUrl, setHaUrl] = useState("http://localhost:8123");
  const [haToken, setHaToken] = useState("");
  const [ollamaUrl, setOllamaUrl] = useState("http://localhost:11434");
  const [models, setModels] = useState<string[]>([]);
  const [selectedModel, setSelectedModel] = useState("qwen3.5:4b");
  const [ollamaHealthy, setOllamaHealthy] = useState<boolean | null>(null);
  const [haTestResult, setHaTestResult] = useState<boolean | null>(null);
  const [saving, setSaving] = useState(false);
  const [saveSuccess, setSaveSuccess] = useState(false);
  const [reduceTransparency, setReduceTransparency] = useState(false);

  useEffect(() => {
    loadConfig();
    loadModels();
    checkHealth();
  }, []);

  useEffect(() => {
    document.documentElement.classList.toggle("reduce-transparency", reduceTransparency);
  }, [reduceTransparency]);

  async function loadConfig() {
    try {
      const cfg = await getConfig();
      if (cfg.ha_url) setHaUrl(cfg.ha_url);
      if (cfg.ha_token) setHaToken(cfg.ha_token);
      if (cfg.ollama_url) setOllamaUrl(cfg.ollama_url);
      if (cfg.ollama_model) setSelectedModel(cfg.ollama_model);
    } catch {
      // Use defaults
    }
  }

  async function loadModels() {
    try {
      const list = await listModels();
      setModels(list);
    } catch {
      setModels([]);
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

  async function handleTestHa() {
    setHaTestResult(null);
    try {
      const result = await testHaConnection(haUrl, haToken);
      setHaTestResult(result);
    } catch {
      setHaTestResult(false);
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
        ollama_model: selectedModel || null,
      });
      setSaveSuccess(true);
      setTimeout(() => setSaveSuccess(false), 3000);
      checkHealth();
    } catch {
      // Save failed
    }
    setSaving(false);
  }

  return (
    <div className="settings-view">
      {/* Home Assistant Connection */}
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
            onChange={(e) => setHaToken(e.target.value)}
            placeholder="Paste your token here"
          />
          <p className="settings-field-hint">
            Generate a token in Home Assistant under Profile &rarr; Long-Lived Access Tokens.
          </p>
        </div>

        <div className="settings-btn-row">
          <button className="form-btn form-btn-primary" onClick={handleTestHa}>
            Test Connection
          </button>
          {haTestResult !== null && (
            <span className="settings-status">
              <span className={`health-dot ${haTestResult ? "health-dot-ok" : "health-dot-error"}`} />
              <span>{haTestResult ? "Connected" : "Connection failed"}</span>
            </span>
          )}
        </div>
      </div>

      {/* Ollama Configuration */}
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
            <span
              className={`health-dot ${
                ollamaHealthy === true
                  ? "health-dot-ok"
                  : ollamaHealthy === false
                    ? "health-dot-error"
                    : "health-dot-loading"
              }`}
            />
            <span>
              {ollamaHealthy === true
                ? "Connected"
                : ollamaHealthy === false
                  ? "Not reachable"
                  : "Checking..."}
            </span>
          </div>
        </div>

        <div className="settings-field">
          <label className="settings-field-label" htmlFor="ollama-model">Model</label>
          <select
            id="ollama-model"
            className="form-select"
            value={selectedModel}
            onChange={(e) => setSelectedModel(e.target.value)}
          >
            {models.length === 0 && (
              <option value={selectedModel}>{selectedModel}</option>
            )}
            {models.map((m) => (
              <option key={m} value={m}>{m}</option>
            ))}
          </select>
        </div>

        <div className="settings-btn-row">
          <button className="form-btn" onClick={loadModels}>Refresh Models</button>
        </div>
      </div>

      {/* Save */}
      <div className="settings-section">
        <div className="settings-btn-row">
          <button
            className="form-btn form-btn-primary"
            onClick={handleSave}
            disabled={saving}
          >
            {saving ? "Saving..." : "Save Settings"}
          </button>
          {saveSuccess && (
            <span className="settings-status">
              <span className="health-dot health-dot-ok" />
              <span>Settings saved</span>
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
