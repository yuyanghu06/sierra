import { useState, useEffect, useRef } from "react";
import {
  checkDependencies,
  checkOllamaUpdate,
  installWsl,
  installOllama,
  installPython,
  installRust,
  installHomeAssistant,
  pullModel,
  startServices,
  type DependencyStatus,
  type OllamaUpdateStatus,
  type InstallProgressEvent,
  type PullProgressEvent,
  type HaConnectionStatus,
} from "../commands/setup";
import { listModels } from "../commands/chat";
import { saveConfig, getConfig, testHaConnection as testHaConnectionDirect } from "../commands/config";
import { checkHaHealth } from "../commands/devices";

type SetupStep = "check" | "install" | "ha-onboarding" | "model" | "done";

const DEFAULT_MODEL = "qwen3.5:4b";

export default function SetupView({ onComplete }: { onComplete: () => void }) {
  const [step, setStep] = useState<SetupStep>("check");
  const [deps, setDeps] = useState<DependencyStatus | null>(null);

  // Install step
  const [wslInstalling, setWslInstalling] = useState(false);
  const [wslProgress, setWslProgress] = useState("");
  const [wslError, setWslError] = useState<string | null>(null);
  const [wslRestartRequired, setWslRestartRequired] = useState(false);
  const [pythonInstalling, setPythonInstalling] = useState(false);
  const [pythonProgress, setPythonProgress] = useState("");
  const [pythonError, setPythonError] = useState<string | null>(null);
  const [rustInstalling, setRustInstalling] = useState(false);
  const [rustProgress, setRustProgress] = useState("");
  const [rustError, setRustError] = useState<string | null>(null);
  const [ollamaInstalling, setOllamaInstalling] = useState(false);
  const [ollamaProgress, setOllamaProgress] = useState("");
  const [ollamaError, setOllamaError] = useState<string | null>(null);
  const [ollamaUpdateStatus, setOllamaUpdateStatus] = useState<OllamaUpdateStatus | null>(null);
  const [haInstalling, setHaInstalling] = useState(false);
  const [haProgress, setHaProgress] = useState("");
  const [haError, setHaError] = useState<string | null>(null);

  // HA onboarding step
  const [haUrl, setHaUrl] = useState("http://localhost:8123");
  const [haStarting, setHaStarting] = useState(false);
  const [haLive, setHaLive] = useState(false);
  const [haToken, setHaToken] = useState("");
  const [tokenStatus, setTokenStatus] = useState<HaConnectionStatus | null>(null);
  const [tokenTesting, setTokenTesting] = useState(false);
  const haPollingRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // Model step
  const [modelProgress, setModelProgress] = useState("");
  const [modelError, setModelError] = useState<string | null>(null);
  const [modelReady, setModelReady] = useState(false);
  const [fallbackModel, setFallbackModel] = useState("");
  const [models, setModels] = useState<string[]>([]);

  useEffect(() => {
    runCheck();
  }, []);

  // Auto-start HA polling when entering ha-onboarding
  useEffect(() => {
    if (step !== "ha-onboarding") return;
    startServicesAndPollHa();
    return () => {
      if (haPollingRef.current) clearInterval(haPollingRef.current);
    };
  }, [step]);

  // Auto-download default model when entering model step
  useEffect(() => {
    if (step !== "model") return;
    loadInstalledModels().then((installed) => {
      if (installed.includes(DEFAULT_MODEL)) {
        setModelReady(true);
        setModelProgress("Ready");
      } else {
        downloadDefaultModel();
      }
    });
  }, [step]);

  async function runCheck() {
    try {
      const status = await checkDependencies();
      setDeps(status);
      setStep("install");
      if (status.ollamaInstalled) {
        checkOllamaUpdate().then(setOllamaUpdateStatus).catch(() => {});
      }
    } catch {
      setDeps(null);
      setStep("install");
    }
  }

  async function startServicesAndPollHa() {
    setHaStarting(true);
    setHaLive(false);
    try {
      await startServices();
      // Read effective HA URL from config — may be WSL2 IP if localhost forwarding is broken
      const cfg = await getConfig();
      if (cfg.ha_url) setHaUrl(cfg.ha_url);
    } catch {
      // ignore — HA may already be starting from app launch
    }

    // Poll :8123 until HA responds (up to 3 min for first run)
    let elapsed = 0;
    haPollingRef.current = setInterval(async () => {
      elapsed += 2;
      try {
        const alive = await checkHaHealth();
        if (alive) {
          clearInterval(haPollingRef.current!);
          setHaStarting(false);
          setHaLive(true);
        }
      } catch {
        // not yet
      }
      if (elapsed >= 180) {
        clearInterval(haPollingRef.current!);
        setHaStarting(false);
        // Show HA URL anyway so user can try manually
        setHaLive(true);
      }
    }, 2000);
  }

  async function loadInstalledModels() {
    try {
      const list = await listModels();
      setModels(list);
      return list;
    } catch {
      return [];
    }
  }

  async function downloadDefaultModel() {
    setModelProgress("Starting download...");
    setModelError(null);
    setModelReady(false);
    try {
      await pullModel(DEFAULT_MODEL, (event: PullProgressEvent) => {
        switch (event.event) {
          case "downloading":
            setModelProgress(`Downloading ${DEFAULT_MODEL}… ${Math.round(event.data.percent)}%`);
            break;
          case "verifying":
            setModelProgress("Verifying...");
            break;
          case "completed":
            setModelProgress(`${DEFAULT_MODEL} ready`);
            break;
          case "failed":
            setModelError(event.data.error);
            setModelProgress("");
            break;
        }
      });
      setModelReady(true);
      await loadInstalledModels();
    } catch (e) {
      setModelError(String(e));
      setModelProgress("");
    }
  }

  async function handleInstallWsl() {
    setWslInstalling(true);
    setWslProgress("Starting...");
    setWslError(null);
    setWslRestartRequired(false);
    try {
      await installWsl((event: InstallProgressEvent) => {
        if (event.event === "started") setWslProgress("Starting WSL installation…");
        else if (event.event === "installing") setWslProgress("Installing WSL (a UAC prompt may appear)…");
        else if (event.event === "completed") {
          setWslProgress("Installed — restart required");
          setWslRestartRequired(true);
        }
        else if (event.event === "failed") { setWslError(event.data.error); setWslProgress(""); }
      });
    } catch (e) {
      setWslError(String(e));
      setWslProgress("");
    }
    setWslInstalling(false);
  }

  async function handleInstallPython() {
    setPythonInstalling(true);
    setPythonProgress("Starting...");
    setPythonError(null);
    try {
      await installPython((event: InstallProgressEvent) => {
        if (event.event === "started") setPythonProgress("Starting installation...");
        else if (event.event === "downloading") setPythonProgress(`Downloading… ${Math.round(event.data.percent)}%`);
        else if (event.event === "installing") setPythonProgress("Installing...");
        else if (event.event === "completed") { setPythonProgress("Installed"); setDeps((d) => d ? { ...d, pythonAvailable: true } : d); }
        else if (event.event === "failed") { setPythonError(event.data.error); setPythonProgress(""); }
      });
    } catch (e) {
      setPythonError(String(e));
      setPythonProgress("");
    }
    setPythonInstalling(false);
  }

  async function handleInstallRust() {
    setRustInstalling(true);
    setRustProgress("Starting...");
    setRustError(null);
    try {
      await installRust((event: InstallProgressEvent) => {
        if (event.event === "started") setRustProgress("Starting installation...");
        else if (event.event === "downloading") setRustProgress(`Downloading… ${Math.round(event.data.percent)}%`);
        else if (event.event === "installing") setRustProgress("Installing...");
        else if (event.event === "completed") { setRustProgress("Installed"); setDeps((d) => d ? { ...d, rustAvailable: true } : d); }
        else if (event.event === "failed") { setRustError(event.data.error); setRustProgress(""); }
      });
    } catch (e) {
      setRustError(String(e));
      setRustProgress("");
    }
    setRustInstalling(false);
  }

  async function handleInstallOllama() {
    setOllamaInstalling(true);
    setOllamaProgress("Starting...");
    setOllamaError(null);
    try {
      await installOllama((event: InstallProgressEvent) => {
        if (event.event === "started") setOllamaProgress("Starting installation...");
        else if (event.event === "downloading") setOllamaProgress(`Downloading… ${Math.round(event.data.percent)}%`);
        else if (event.event === "installing") setOllamaProgress("Installing...");
        else if (event.event === "completed") {
          setOllamaProgress("Installed");
          setDeps((d) => d ? { ...d, ollamaInstalled: true } : d);
          setOllamaUpdateStatus(null);
          checkOllamaUpdate().then(setOllamaUpdateStatus).catch(() => {});
        }
        else if (event.event === "failed") { setOllamaError(event.data.error); setOllamaProgress(""); }
      });
    } catch (e) {
      setOllamaError(String(e));
      setOllamaProgress("");
    }
    setOllamaInstalling(false);
  }

  async function handleInstallHa() {
    setHaInstalling(true);
    setHaProgress("Starting...");
    setHaError(null);
    try {
      await installHomeAssistant((event: InstallProgressEvent) => {
        if (event.event === "started") setHaProgress("Starting installation...");
        else if (event.event === "downloading") setHaProgress(`Installing packages… ${Math.round(event.data.percent)}%`);
        else if (event.event === "installing") setHaProgress("Installing system dependencies...");
        else if (event.event === "configuring") setHaProgress("Configuring...");
        else if (event.event === "completed") { setHaProgress("Installed"); setDeps((d) => d ? { ...d, homeAssistantInstalled: true } : d); }
        else if (event.event === "failed") { setHaError(event.data.error); setHaProgress(""); }
      });
    } catch (e) {
      setHaError(String(e));
      setHaProgress("");
    }
    setHaInstalling(false);
  }

  async function handleTestToken() {
    if (!haToken.trim()) return;
    setTokenTesting(true);
    setTokenStatus(null);
    try {
      const result = await testHaConnectionDirect(haUrl, haToken.trim());
      setTokenStatus(result);
    } catch {
      setTokenStatus({ status: "unreachable" });
    }
    setTokenTesting(false);
  }

  async function handleSaveTokenAndContinue() {
    const cfg = await getConfig();
    await saveConfig({ ...cfg, ha_url: haUrl, ha_token: haToken.trim() });
    setStep("model");
  }

  async function handleFallbackDownload() {
    const model = fallbackModel.trim() || DEFAULT_MODEL;
    setFallbackModel("");
    setModelProgress("Starting download...");
    setModelError(null);
    setModelReady(false);
    try {
      await pullModel(model, (event: PullProgressEvent) => {
        if (event.event === "downloading") setModelProgress(`Downloading ${model}… ${Math.round(event.data.percent)}%`);
        else if (event.event === "verifying") setModelProgress("Verifying...");
        else if (event.event === "completed") setModelProgress(`${model} ready`);
        else if (event.event === "failed") { setModelError(event.data.error); setModelProgress(""); }
      });
      setModelReady(true);
      await loadInstalledModels();
    } catch (e) {
      setModelError(String(e));
    }
  }

  async function handleSelectExisting(model: string) {
    setModelProgress(`${model} selected`);
    setModelReady(true);
  }

  async function handleFinish() {
    const cfg = await getConfig();
    await saveConfig({
      ...cfg,
      ollama_model: (models.includes(DEFAULT_MODEL) ? DEFAULT_MODEL : models[0]) || null,
    });
    onComplete();
  }

  const bothInstalled = deps?.ollamaInstalled && deps?.homeAssistantInstalled &&
    deps?.pythonAvailable && deps?.rustAvailable && deps?.wslAvailable;
  const tokenConnected = tokenStatus?.status === "connected";

  return (
    <div className="setup-view">
      <div className="setup-card">
        <div className="setup-header">
          <img className="setup-logo" src="/sierra-logo.png" alt="Sierra" width="40" height="40" />
          <h1 className="setup-title">Welcome to Sierra</h1>
          <p className="setup-subtitle">Local natural-language smart home control.</p>
        </div>

        {/* Checking */}
        {step === "check" && (
          <div className="setup-body">
            <div className="setup-checking">
              <span className="setup-spinner" />
              <span>Checking system...</span>
            </div>
          </div>
        )}

        {/* Install dependencies */}
        {step === "install" && (
          <div className="setup-body">
            {/* WSL — only shown when not already available (Windows without WSL) */}
            {deps && !deps.wslAvailable && (
              <SetupDep
                name="WSL (Windows Subsystem for Linux)"
                desc="Required to run Home Assistant on Windows"
                installed={false}
                installing={wslInstalling}
                progress={wslProgress}
                error={wslError}
                onInstall={handleInstallWsl}
                restartRequired={wslRestartRequired}
              />
            )}

            {wslRestartRequired && (
              <div className="setup-restart-banner">
                <span className="setup-restart-icon">⚠</span>
                <div>
                  <strong>Restart required</strong>
                  <p>WSL was installed but needs a system restart to finish setup. Save your work, restart your computer, then reopen Sierra.</p>
                </div>
              </div>
            )}

            <SetupDep
              name="Python 3.12"
              desc="Required for Home Assistant"
              installed={!!deps?.pythonAvailable}
              installing={pythonInstalling}
              progress={pythonProgress}
              error={pythonError}
              onInstall={handleInstallPython}
            />
            <SetupDep
              name="Rust"
              desc="Required to build native components"
              installed={!!deps?.rustAvailable}
              installing={rustInstalling}
              progress={rustProgress}
              error={rustError}
              onInstall={handleInstallRust}
            />
            <SetupDep
              name="Ollama"
              desc="Local LLM inference"
              installed={!!deps?.ollamaInstalled}
              installing={ollamaInstalling}
              progress={ollamaProgress}
              error={ollamaError}
              onInstall={handleInstallOllama}
              updateAvailable={ollamaUpdateStatus?.updateAvailable}
              latestVersion={ollamaUpdateStatus?.latestVersion ?? undefined}
              currentVersion={ollamaUpdateStatus?.currentVersion ?? undefined}
            />
            <SetupDep
              name="Home Assistant"
              desc="Smart home device bridge"
              installed={!!deps?.homeAssistantInstalled}
              installing={haInstalling}
              progress={haProgress}
              error={haError}
              onInstall={handleInstallHa}
              disabled={!deps?.pythonAvailable}
              disabledReason={!deps?.pythonAvailable ? "Python 3.12+ is required. Please install Python first." : undefined}
            />
            <div className="setup-actions">
              <button
                className="form-btn form-btn-primary"
                onClick={() => setStep("ha-onboarding")}
                disabled={!bothInstalled}
              >
                Continue
              </button>
            </div>
          </div>
        )}

        {/* HA Onboarding */}
        {step === "ha-onboarding" && (
          <div className="setup-body">
            <div className="setup-section">
              <h3 className="setup-section-title">Connect Home Assistant</h3>

              {haStarting && (
                <div className="setup-checking setup-checking-inline">
                  <span className="setup-spinner" />
                  <span>Starting Home Assistant…</span>
                </div>
              )}

              {haLive && (
                <>
                  <div className="setup-onboarding-steps">
                    <p className="setup-onboarding-intro">
                      Home Assistant is running. Follow these steps to connect Sierra:
                    </p>
                    <ol className="setup-onboarding-list">
                      <li>
                        Open{" "}
                      <a
                          href={haUrl}
                          className="setup-link"
                          onClick={(e) => e.preventDefault()}
                        >
                          {haUrl}
                        </a>{" "}
                        in your browser
                      </li>
                      <li>Create your Home Assistant account when prompted</li>
                      <li>
                        Once logged in, click your profile avatar (bottom-left) →{" "}
                        <strong>Profile</strong>
                      </li>
                      <li>
                        Scroll down to <strong>Long-Lived Access Tokens</strong> and click{" "}
                        <strong>Create Token</strong>
                      </li>
                      <li>Give it any name (e.g. "Sierra"), then copy the token</li>
                      <li>Paste it below and click <strong>Test Connection</strong></li>
                    </ol>
                  </div>

                  <div className="setup-token-row">
                    <input
                      className="form-input"
                      type="password"
                      placeholder="Paste your Long-Lived Access Token here"
                      value={haToken}
                      onChange={(e) => { setHaToken(e.target.value); setTokenStatus(null); }}
                    />
                    <button
                      className="form-btn form-btn-primary"
                      onClick={handleTestToken}
                      disabled={tokenTesting || !haToken.trim()}
                    >
                      {tokenTesting ? "Testing…" : "Test"}
                    </button>
                  </div>

                  {tokenStatus && (
                    <div className={`setup-token-status ${tokenConnected ? "setup-token-status-ok" : tokenStatus.status === "needsOnboarding" ? "setup-token-status-warn" : "setup-token-status-err"}`}>
                      {tokenConnected && "✓ Connected — Sierra can reach Home Assistant"}
                      {tokenStatus.status === "needsOnboarding" && `Complete setup at ${haUrl} first, then generate a token`}
                      {tokenStatus.status === "invalidToken" && "Token not recognised — make sure you copied the full token"}
                      {tokenStatus.status === "unreachable" && `Home Assistant isn't responding at ${haUrl}`}
                    </div>
                  )}
                </>
              )}
            </div>

            <div className="setup-actions">
              <button
                className="form-btn form-btn-primary"
                onClick={handleSaveTokenAndContinue}
                disabled={!tokenConnected}
              >
                Continue
              </button>
            </div>
          </div>
        )}

        {/* Model download */}
        {step === "model" && (
          <div className="setup-body">
            <div className="setup-section">
              <h3 className="setup-section-title">Downloading AI Model</h3>
              <p className="setup-section-desc">
                Sierra uses <strong>{DEFAULT_MODEL}</strong> as its default model.
                This is a one-time download (~2.6 GB).
              </p>

              {modelProgress && !modelError && (
                <div className="setup-model-progress">
                  {!modelReady && <span className="setup-spinner setup-spinner-sm" />}
                  <span>{modelProgress}</span>
                </div>
              )}

              {modelError && (
                <div className="setup-dep-error">
                  <p>Download failed: {modelError}</p>
                  <p className="setup-error-sub">
                    Make sure Ollama is running, then retry or enter a different model name.
                  </p>
                  <div className="setup-model-pull-row">
                    <input
                      className="form-input"
                      type="text"
                      placeholder={`Retry ${DEFAULT_MODEL} or enter another model`}
                      value={fallbackModel}
                      onChange={(e) => setFallbackModel(e.target.value)}
                    />
                    <button className="form-btn form-btn-primary" onClick={handleFallbackDownload}>
                      Download
                    </button>
                  </div>
                </div>
              )}

              {models.length > 1 && !modelReady && !modelError && (
                <div className="setup-model-existing">
                  <p className="settings-field-label">Or use an existing model</p>
                  <div className="setup-model-existing-list">
                    {models.filter((m) => m !== DEFAULT_MODEL).map((m) => (
                      <button key={m} className="setup-model-chip" onClick={() => handleSelectExisting(m)}>
                        {m}
                      </button>
                    ))}
                  </div>
                </div>
              )}
            </div>

            <div className="setup-actions">
              <button
                className="form-btn form-btn-primary"
                onClick={handleFinish}
                disabled={!modelReady}
              >
                Get Started
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

/* ── Sub-components ────────────────────────────────────────────────────────── */

function SetupDep({
  name, desc, installed, installing, progress, error, onInstall, disabled, disabledReason, restartRequired,
  updateAvailable, latestVersion, currentVersion,
}: {
  name: string;
  desc: string;
  installed: boolean;
  installing: boolean;
  progress: string;
  error: string | null;
  onInstall: () => void;
  disabled?: boolean;
  disabledReason?: string;
  restartRequired?: boolean;
  updateAvailable?: boolean;
  latestVersion?: string;
  currentVersion?: string;
}) {
  return (
    <div className="setup-dep">
      <div className="setup-dep-row">
        <div className="setup-dep-info">
          <span className={`setup-dep-check ${installed && !updateAvailable ? "setup-dep-check-ok" : restartRequired ? "setup-dep-check-warn" : updateAvailable ? "setup-dep-check-warn" : ""}`}>
            {installed && !updateAvailable ? "✓" : restartRequired ? "↻" : updateAvailable ? "↑" : "•"}
          </span>
          <div>
            <span className="setup-dep-name">{name}</span>
            <span className="setup-dep-desc">
              {updateAvailable
                ? `Update available: ${currentVersion} → ${latestVersion}`
                : desc}
            </span>
          </div>
        </div>
        {restartRequired ? (
          <span className="setup-dep-restart">Restart required</span>
        ) : !installed ? (
          <button
            className="form-btn form-btn-primary"
            onClick={onInstall}
            disabled={installing || disabled}
          >
            {installing ? "Installing…" : "Install"}
          </button>
        ) : updateAvailable ? (
          <button
            className="form-btn form-btn-primary"
            onClick={onInstall}
            disabled={installing}
          >
            {installing ? "Updating…" : "Update"}
          </button>
        ) : (
          <span className="setup-dep-installed">Installed</span>
        )}
      </div>
      {disabledReason && !installed && (
        <div className="setup-dep-error">{disabledReason}</div>
      )}
      {progress && <div className="setup-dep-progress">{progress}</div>}
      {error && <div className="setup-dep-error">{error}</div>}
    </div>
  );
}

