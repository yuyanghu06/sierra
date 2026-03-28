import { useState, useEffect } from "react";
import { listModels, checkOllamaHealth } from "../commands/chat";
import { getConfig, saveConfig } from "../commands/config";
import { pullModel, type PullProgressEvent } from "../commands/setup";

export default function SettingsView() {
  const [haUrl, setHaUrl] = useState("http://localhost:8123");
  const [haToken, setHaToken] = useState("");
  const [ollamaUrl, setOllamaUrl] = useState("http://localhost:11434");
  const [models, setModels] = useState<string[]>([]);
  const [selectedModel, setSelectedModel] = useState("qwen3.5:4b");
  const [savedModel, setSavedModel] = useState("qwen3.5:4b");
  const [ollamaHealthy, setOllamaHealthy] = useState<boolean | null>(null);
  const [haTestResult, setHaTestResult] = useState<boolean | null>(null);
  const [saving, setSaving] = useState(false);
  const [saveSuccess, setSaveSuccess] = useState(false);
  const [reduceTransparency, setReduceTransparency] = useState(false);

  // Model download state
  const [pulling, setPulling] = useState(false);
  const [pullProgress, setPullProgress] = useState("");
  const [pullError, setPullError] = useState<string | null>(null);
  const [modelDownloaded, setModelDownloaded] = useState(true);

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
      if (cfg.ollama_model) {
        setSelectedModel(cfg.ollama_model);
        setSavedModel(cfg.ollama_model);
      }
    } catch {
      // Use defaults
    }
  }

  async function loadModels() {
    try {
      const list = await listModels();
      setModels(list);
      return list;
    } catch {
      setModels([]);
      return [];
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
      const { testHaConnection } = await import("../commands/config");
      const result = await testHaConnection(haUrl, haToken);
      setHaTestResult(result);
    } catch {
      setHaTestResult(false);
    }
  }

  function handleModelChange(model: string) {
    setSelectedModel(model);
    // If the new model is already in the local model list, it's downloaded
    setModelDownloaded(models.includes(model));
    setPullError(null);
    setPullProgress("");
  }

  async function handlePullSelectedModel() {
    if (!selectedModel) return;

    setPulling(true);
    setPullProgress("Starting download...");
    setPullError(null);
    setModelDownloaded(false);

    try {
      await pullModel(selectedModel, (event: PullProgressEvent) => {
        switch (event.event) {
          case "downloading":
            setPullProgress(`Downloading... ${Math.round(event.data.percent)}%`);
            break;
          case "verifying":
            setPullProgress("Verifying...");
            break;
          case "completed":
            setPullProgress("Downloaded");
            break;
          case "failed":
            setPullError(event.data.error);
            setPullProgress("");
            break;
        }
      });

      setModelDownloaded(true);
      setPulling(false);

      // Refresh model list
      await loadModels();
    } catch (e) {
      setPulling(false);
      setPullError(String(e));
    }
  }

  async function handleSave() {
    // If the model changed and isn't downloaded yet, pull it first
    if (selectedModel !== savedModel && !modelDownloaded) {
      await handlePullSelectedModel();
      // If pull failed, don't save
      if (!modelDownloaded) return;
    }

    setSaving(true);
    setSaveSuccess(false);
    try {
      await saveConfig({
        ha_url: haUrl || null,
        ha_token: haToken || null,
        ollama_url: ollamaUrl || null,
        ollama_model: selectedModel || null,
      });
      setSavedModel(selectedModel);
      setSaveSuccess(true);
      setTimeout(() => setSaveSuccess(false), 3000);
      checkHealth();
    } catch {
      // Save failed
    }
    setSaving(false);
  }

  const modelChanged = selectedModel !== savedModel;
  const needsDownload = modelChanged && !modelDownloaded;

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
          <div className="settings-model-row">
            <select
              id="ollama-model"
              className="form-select"
              value={selectedModel}
              onChange={(e) => handleModelChange(e.target.value)}
              disabled={pulling}
            >
              {models.length === 0 && (
                <option value={selectedModel}>{selectedModel}</option>
              )}
              {models.map((m) => (
                <option key={m} value={m}>{m}</option>
              ))}
            </select>
            {needsDownload && !pulling && (
              <button
                className="form-btn form-btn-primary"
                onClick={handlePullSelectedModel}
              >
                Download
              </button>
            )}
          </div>
          {pulling && (
            <div className="settings-pull-progress">{pullProgress}</div>
          )}
          {pullError && (
            <div className="settings-pull-error">{pullError}</div>
          )}
          {modelChanged && modelDownloaded && (
            <div className="settings-pull-ready">Model ready — save to apply</div>
          )}
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
            disabled={saving || pulling}
          >
            {pulling
              ? "Downloading model..."
              : saving
                ? "Saving..."
                : needsDownload
                  ? "Download & Save"
                  : "Save Settings"}
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
