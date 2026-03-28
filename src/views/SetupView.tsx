import { useState, useEffect } from "react";
import {
  checkDependencies,
  installOllama,
  installHomeAssistant,
  pullModel,
  startServices,
  type DependencyStatus,
  type InstallProgressEvent,
  type PullProgressEvent,
} from "../commands/setup";
import { listModels, checkOllamaHealth } from "../commands/chat";
import { saveConfig, getConfig } from "../commands/config";

type SetupStep = "check" | "install" | "startingOllama" | "model" | "ready";

interface StepState {
  ollamaInstalling: boolean;
  ollamaProgress: string;
  ollamaError: string | null;
  haInstalling: boolean;
  haProgress: string;
  haError: string | null;
  modelPulling: boolean;
  modelProgress: string;
  modelError: string | null;
}

export default function SetupView({ onComplete }: { onComplete: () => void }) {
  const [step, setStep] = useState<SetupStep>("check");
  const [deps, setDeps] = useState<DependencyStatus | null>(null);
  const [models, setModels] = useState<string[]>([]);
  const [selectedModel, setSelectedModel] = useState("");
  const [customModel, setCustomModel] = useState("");
  const [modelReady, setModelReady] = useState(false);
  const [startingServices, setStartingServices] = useState(false);
  const [stepState, setStepState] = useState<StepState>({
    ollamaInstalling: false,
    ollamaProgress: "",
    ollamaError: null,
    haInstalling: false,
    haProgress: "",
    haError: null,
    modelPulling: false,
    modelProgress: "",
    modelError: null,
  });

  useEffect(() => {
    runCheck();
  }, []);

  async function runCheck() {
    try {
      const status = await checkDependencies();
      setDeps(status);

      if (status.ollama_installed && status.home_assistant_installed) {
        // Both installed — start Ollama and go to model step
        await ensureOllamaRunningThenModelStep();
      } else {
        setStep("install");
      }
    } catch {
      setDeps(null);
      setStep("install");
    }
  }

  async function ensureOllamaRunningThenModelStep() {
    setStep("startingOllama");

    // First try starting services so Ollama is running
    try {
      await startServices();
    } catch {
      // May fail if services aren't installed as managed — that's ok
    }

    // Poll until Ollama is healthy (up to 30s)
    let healthy = false;
    for (let i = 0; i < 30; i++) {
      try {
        healthy = await checkOllamaHealth();
        if (healthy) break;
      } catch {
        // ignore
      }
      await new Promise((r) => setTimeout(r, 1000));
    }

    if (!healthy) {
      // Ollama didn't come up — still go to model step, user may need to start it
      setStep("model");
      return;
    }

    // Ollama is running, load models and go to model step
    await loadModels();
    setStep("model");
  }

  async function loadModels() {
    try {
      const list = await listModels();
      setModels(list);
      if (list.length > 0 && !selectedModel) {
        setSelectedModel(list[0]);
        setModelReady(true);
      }
    } catch {
      setModels([]);
    }
  }

  async function handleInstallOllama() {
    setStepState((s) => ({
      ...s,
      ollamaInstalling: true,
      ollamaProgress: "Starting...",
      ollamaError: null,
    }));

    try {
      await installOllama((event: InstallProgressEvent) => {
        switch (event.event) {
          case "started":
            setStepState((s) => ({ ...s, ollamaProgress: "Starting installation..." }));
            break;
          case "downloading":
            setStepState((s) => ({
              ...s,
              ollamaProgress: `Downloading... ${Math.round(event.data.percent)}%`,
            }));
            break;
          case "installing":
            setStepState((s) => ({ ...s, ollamaProgress: "Installing..." }));
            break;
          case "completed":
            setStepState((s) => ({
              ...s,
              ollamaInstalling: false,
              ollamaProgress: "Installed",
            }));
            setDeps((d) => (d ? { ...d, ollama_installed: true } : d));
            break;
          case "failed":
            setStepState((s) => ({
              ...s,
              ollamaInstalling: false,
              ollamaError: event.data.error,
              ollamaProgress: "",
            }));
            break;
        }
      });
    } catch (e) {
      setStepState((s) => ({
        ...s,
        ollamaInstalling: false,
        ollamaError: String(e),
        ollamaProgress: "",
      }));
    }
  }

  async function handleInstallHa() {
    setStepState((s) => ({
      ...s,
      haInstalling: true,
      haProgress: "Starting...",
      haError: null,
    }));

    try {
      await installHomeAssistant((event: InstallProgressEvent) => {
        switch (event.event) {
          case "started":
            setStepState((s) => ({ ...s, haProgress: "Starting installation..." }));
            break;
          case "downloading":
            setStepState((s) => ({
              ...s,
              haProgress: `Installing packages... ${Math.round(event.data.percent)}%`,
            }));
            break;
          case "installing":
            setStepState((s) => ({ ...s, haProgress: "Creating virtual environment..." }));
            break;
          case "configuring":
            setStepState((s) => ({ ...s, haProgress: "Configuring..." }));
            break;
          case "completed":
            setStepState((s) => ({
              ...s,
              haInstalling: false,
              haProgress: "Installed",
            }));
            setDeps((d) => (d ? { ...d, home_assistant_installed: true } : d));
            break;
          case "failed":
            setStepState((s) => ({
              ...s,
              haInstalling: false,
              haError: event.data.error,
              haProgress: "",
            }));
            break;
        }
      });
    } catch (e) {
      setStepState((s) => ({
        ...s,
        haInstalling: false,
        haError: String(e),
        haProgress: "",
      }));
    }
  }

  async function handleContinueToModel() {
    await ensureOllamaRunningThenModelStep();
  }

  async function handlePullModel() {
    const modelName = customModel.trim() || selectedModel;
    if (!modelName) return;

    setModelReady(false);
    setStepState((s) => ({
      ...s,
      modelPulling: true,
      modelProgress: "Starting download...",
      modelError: null,
    }));

    try {
      await pullModel(modelName, (event: PullProgressEvent) => {
        switch (event.event) {
          case "downloading":
            setStepState((s) => ({
              ...s,
              modelProgress: `Downloading... ${Math.round(event.data.percent)}%`,
            }));
            break;
          case "verifying":
            setStepState((s) => ({ ...s, modelProgress: "Verifying..." }));
            break;
          case "completed":
            setStepState((s) => ({
              ...s,
              modelPulling: false,
              modelProgress: "Ready",
            }));
            break;
          case "failed":
            setStepState((s) => ({
              ...s,
              modelPulling: false,
              modelError: event.data.error,
              modelProgress: "",
            }));
            break;
        }
      });

      // Model pulled successfully — select it and mark ready
      const pulledName = customModel.trim() || selectedModel;
      setSelectedModel(pulledName);
      setCustomModel("");
      setModelReady(true);

      // Refresh model list
      await loadModels();
    } catch (e) {
      setStepState((s) => ({
        ...s,
        modelPulling: false,
        modelError: String(e),
        modelProgress: "",
      }));
    }
  }

  function handleSelectExistingModel(model: string) {
    setSelectedModel(model);
    setModelReady(true);
  }

  async function handleFinish() {
    setStartingServices(true);
    try {
      // Save the selected model to config
      const cfg = await getConfig();
      await saveConfig({
        ...cfg,
        ollama_model: selectedModel || null,
      });

      // Start remaining services (HA if not already running)
      await startServices();
    } catch {
      // Services will start on their own
    }
    setStartingServices(false);
    onComplete();
  }

  const bothInstalled = deps?.ollama_installed && deps?.home_assistant_installed;

  return (
    <div className="setup-view">
      <div className="setup-card">
        <div className="setup-header">
          <img
            className="setup-logo"
            src="/sierra-logo.png"
            alt="Sierra"
            width="40"
            height="40"
          />
          <h1 className="setup-title">Welcome to Sierra</h1>
          <p className="setup-subtitle">
            Sierra needs two services to control your smart home.
          </p>
        </div>

        {/* Step: Checking */}
        {step === "check" && (
          <div className="setup-body">
            <div className="setup-checking">
              <span className="setup-spinner" />
              <span>Checking system...</span>
            </div>
          </div>
        )}

        {/* Step: Starting Ollama */}
        {step === "startingOllama" && (
          <div className="setup-body">
            <div className="setup-checking">
              <span className="setup-spinner" />
              <span>Starting Ollama...</span>
            </div>
          </div>
        )}

        {/* Step: Install dependencies */}
        {step === "install" && (
          <div className="setup-body">
            {/* Ollama */}
            <div className="setup-dep">
              <div className="setup-dep-row">
                <div className="setup-dep-info">
                  <span
                    className={`setup-dep-check ${deps?.ollama_installed ? "setup-dep-check-ok" : ""}`}
                  >
                    {deps?.ollama_installed ? "\u2713" : "\u2022"}
                  </span>
                  <div>
                    <span className="setup-dep-name">Ollama</span>
                    <span className="setup-dep-desc">Local LLM inference</span>
                  </div>
                </div>
                {!deps?.ollama_installed && (
                  <button
                    className="form-btn form-btn-primary"
                    onClick={handleInstallOllama}
                    disabled={stepState.ollamaInstalling}
                  >
                    {stepState.ollamaInstalling ? "Installing..." : "Install"}
                  </button>
                )}
                {deps?.ollama_installed && (
                  <span className="setup-dep-installed">Installed</span>
                )}
              </div>
              {stepState.ollamaProgress && (
                <div className="setup-dep-progress">{stepState.ollamaProgress}</div>
              )}
              {stepState.ollamaError && (
                <div className="setup-dep-error">{stepState.ollamaError}</div>
              )}
            </div>

            {/* Home Assistant */}
            <div className="setup-dep">
              <div className="setup-dep-row">
                <div className="setup-dep-info">
                  <span
                    className={`setup-dep-check ${deps?.home_assistant_installed ? "setup-dep-check-ok" : ""}`}
                  >
                    {deps?.home_assistant_installed ? "\u2713" : "\u2022"}
                  </span>
                  <div>
                    <span className="setup-dep-name">Home Assistant</span>
                    <span className="setup-dep-desc">Smart home device bridge</span>
                  </div>
                </div>
                {!deps?.home_assistant_installed && (
                  <button
                    className="form-btn form-btn-primary"
                    onClick={handleInstallHa}
                    disabled={stepState.haInstalling || !deps?.python_available}
                  >
                    {stepState.haInstalling ? "Installing..." : "Install"}
                  </button>
                )}
                {deps?.home_assistant_installed && (
                  <span className="setup-dep-installed">Installed</span>
                )}
              </div>
              {!deps?.python_available && !deps?.home_assistant_installed && (
                <div className="setup-dep-error">
                  Python 3.12+ is required. Please install Python first.
                </div>
              )}
              {stepState.haProgress && (
                <div className="setup-dep-progress">{stepState.haProgress}</div>
              )}
              {stepState.haError && (
                <div className="setup-dep-error">{stepState.haError}</div>
              )}
            </div>

            <div className="setup-actions">
              <button
                className="form-btn form-btn-primary"
                onClick={handleContinueToModel}
                disabled={!bothInstalled}
              >
                Continue
              </button>
            </div>
          </div>
        )}

        {/* Step: Model selection */}
        {step === "model" && (
          <div className="setup-body">
            <div className="setup-section">
              <h3 className="setup-section-title">Choose a Model</h3>
              <p className="setup-section-desc">
                Select an LLM model to power Sierra. Models with tool calling
                support work best. You need at least one model to continue.
              </p>

              {models.length > 0 && (
                <div className="setup-model-list">
                  <label className="settings-field-label" htmlFor="setup-model">
                    Installed Models
                  </label>
                  <select
                    id="setup-model"
                    className="form-select"
                    value={selectedModel}
                    onChange={(e) => handleSelectExistingModel(e.target.value)}
                  >
                    {!selectedModel && (
                      <option value="" disabled>
                        Select a model...
                      </option>
                    )}
                    {models.map((m) => (
                      <option key={m} value={m}>
                        {m}
                      </option>
                    ))}
                  </select>
                </div>
              )}

              <div className="setup-model-pull">
                <label className="settings-field-label" htmlFor="setup-custom-model">
                  {models.length > 0
                    ? "Or download a different model"
                    : "Download a Model"}
                </label>
                <div className="setup-model-pull-row">
                  <input
                    id="setup-custom-model"
                    className="form-input"
                    type="text"
                    value={customModel}
                    onChange={(e) => setCustomModel(e.target.value)}
                    placeholder="e.g., llama3.2, qwen3.5:4b, mistral"
                  />
                  <button
                    className="form-btn form-btn-primary"
                    onClick={handlePullModel}
                    disabled={
                      stepState.modelPulling ||
                      (!customModel.trim() && !selectedModel)
                    }
                  >
                    {stepState.modelPulling ? "Downloading..." : "Download"}
                  </button>
                </div>
                {stepState.modelProgress && (
                  <div className="setup-dep-progress">{stepState.modelProgress}</div>
                )}
                {stepState.modelError && (
                  <div className="setup-dep-error">{stepState.modelError}</div>
                )}
              </div>
            </div>

            <div className="setup-actions">
              <button
                className="form-btn form-btn-primary"
                onClick={handleFinish}
                disabled={!modelReady || startingServices || stepState.modelPulling}
              >
                {startingServices ? "Starting..." : "Get Started"}
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
