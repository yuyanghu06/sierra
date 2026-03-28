# Sierra

A local, natural-language smart home controller.

Sierra is a desktop app that connects to your smart home devices through [Home Assistant](https://www.home-assistant.io/) and lets you control them by chatting with a locally-running LLM powered by [Ollama](https://ollama.com/). Say what you want in plain English, and the app figures out which devices to control and makes it happen. No cloud. No subscriptions. No accounts. Everything runs on your hardware.

## Requirements

### Operating System

- **macOS** (primary)
- **Windows** (primary)
- Linux is not currently supported

### Hardware

Running a local LLM requires meaningful system resources. Minimum recommendations depend on the model you choose:

| Model Size | RAM Required | Example Models |
|---|---|---|
| 7B parameters | 8 GB | Llama 3.1 7B, Mistral 7B |
| 13B parameters | 16 GB | Llama 3.1 13B |
| 70B parameters | 64 GB+ | Llama 3.1 70B |

For most users, a **7B or 13B model with 16 GB of RAM** provides a good balance of performance and quality. Models that support tool calling (function calling) work best.

### Dependencies

The app requires two services to function:

- **Ollama** — runs the LLM locally
- **Home Assistant Core** — communicates with your smart home devices

You do **not** need to install these yourself. On first launch, the app detects whether each is installed and sets them up automatically if missing. If you prefer to manage them yourself, see [Running Ollama and Home Assistant Manually](#running-ollama-and-home-assistant-manually).

## Installation

### From Installer (Recommended)

1. Download the installer for your platform:
   - **macOS:** `.dmg` file
   - **Windows:** `.exe` or `.msi` installer
2. Run the installer. No terminal commands required.

### Building from Source

Prerequisites:
- [Node.js](https://nodejs.org/) 18+
- [Rust](https://www.rust-lang.org/tools/install) (latest stable)
- [Tauri CLI](https://v2.tauri.app/start/prerequisites/)

```bash
# Clone the repository
git clone https://github.com/your-username/sierra.git
cd sierra

# Install frontend dependencies
npm install

# Run in development mode
npm run tauri dev

# Build native installer
npm run tauri build
```

Build output:
- **macOS:** `src-tauri/target/release/bundle/dmg/Sierra_0.1.0_aarch64.dmg`
- **Windows:** `src-tauri/target/release/bundle/nsis/Sierra_0.1.0_x64-setup.exe`

### First Launch

When you open the app for the first time, a setup wizard guides you through:

1. **Dependency check** — detects if Ollama and Home Assistant are installed. If both are already present, this step is skipped automatically.
2. **Install missing services** — click Install for each missing dependency. Progress is shown inline. Home Assistant is installed into a managed Python virtual environment; Ollama is installed via platform-native methods.
3. **Model selection** — choose from already-installed models or download a new one. The download streams progress from Ollama's pull API.
4. **Done** — the app starts both services (or detects your existing instances) and shows the main interface.

Subsequent launches are fast — the app checks if services are already running on their default ports, starts any that aren't, and waits for health checks to pass.

## Setup

### Connecting to Home Assistant

If you're letting the app manage Home Assistant, it starts automatically and no configuration is needed.

If you're connecting to an existing Home Assistant instance:

1. Open **Settings** from the sidebar.
2. Enter your Home Assistant URL (e.g., `http://localhost:8123`).
3. Generate a long-lived access token in Home Assistant:
   - Go to your HA instance in a browser
   - Navigate to **Profile** (click your name in the sidebar)
   - Scroll to **Long-Lived Access Tokens**
   - Click **Create Token**, give it a name, and copy the token
4. Paste the token into the app's Settings.
5. Click **Test Connection** to verify, then **Save Settings**.

When you save new HA credentials, the backend immediately reconfigures — reconnects the REST client, refreshes the device cache, and updates the WebSocket subscription. No restart needed.

### Selecting an LLM Model

1. Open **Settings** from the sidebar.
2. Under the Ollama section, select a model from the dropdown (shows all locally available models) or enter a model name.
3. Click **Refresh Models** to re-scan Ollama for new models after pulling one manually.
4. Click **Save Settings** to apply. The next chat message will use the new model.

**Recommended models for smart home control:**

- **qwen3.5:4b** — Small and fast, good tool-calling support. Default choice.
- **Llama 3.1 8B** — Good balance of speed and accuracy. Works well on 8-16 GB RAM.
- **Mistral 7B** — Fast, strong tool-calling support.
- **Llama 3.1 70B** — Best accuracy, but requires 64 GB+ RAM.

Any model Ollama supports that handles tool calling should work. Smaller models are faster but may occasionally misinterpret complex requests.

### Device Discovery

Once connected to Home Assistant, the app automatically reads all available devices from your HA instance. You'll see them populate in the **Device Panel** on the right side of the window and in the **Devices** view. Devices are grouped by room (as configured in Home Assistant).

The device list is injected into the LLM's system prompt so the model knows exactly which entities exist and can map your natural language to real entity IDs.

## Usage

### Chat Interface

The chat view is the primary way to interact with the app. Type a natural language command in the input field at the bottom and press Enter or click the send button.

**Example commands:**

| You say | What happens |
|---|---|
| "Turn off the living room lights" | Sends `light.turn_off` to the living room light entities |
| "Set the thermostat to 72" | Sends `climate.set_temperature` with target 72 degrees |
| "Dim the bedroom lights to 30%" | Sends `light.turn_on` with brightness at 30% |
| "Pause the music" | Sends `media_player.media_pause` to active media players |
| "Turn on the kitchen lights and set them to warm white" | Executes multiple actions: turns on lights and adjusts color temperature |
| "What's the temperature set to?" | Queries thermostat state and reports back |

When the LLM executes a device action, an inline confirmation pill appears in the chat showing what happened (e.g., "light.turn_on executed" with a green checkmark). Failed actions show a red pill with the error.

### Devices View

Click **Devices** in the sidebar to see a room-by-room grid of all your devices with direct controls:

- **Lights:** Toggle on/off, adjust brightness slider
- **Switches:** Toggle on/off
- **Climate:** Set temperature with +/- buttons
- **Media players:** Play/pause/stop, adjust volume slider

### Device Panel

The right sidebar shows a live summary of device states. Use the tabs at the top to filter:

- **Active** — devices currently on or in use
- **All** — every discovered device
- **Rooms** — grouped by room

The panel collapses automatically on smaller window sizes, or you can toggle it via the titlebar button.

### Service Health

The sidebar shows live health indicators at the bottom:

- **Ollama** — green when connected, red when unreachable
- **Home Assistant** — green when connected, red when disconnected
- **Active model** — shows the current model name, green when Ollama is healthy

Health is polled every 30 seconds and on window focus. The app manages child processes with automatic restart on crash (up to 3 retries).

## Running Ollama and Home Assistant Manually

If you already have Ollama or Home Assistant running on your machine, the app detects them on their default ports and skips auto-installation. You can also run them independently and point the app at your existing instances.

### Ollama

#### macOS

```bash
# Install via Homebrew
brew install ollama

# Or download directly from https://ollama.com/download

# Start the server
ollama serve

# Pull a model
ollama pull qwen3.5:4b
```

#### Windows

```powershell
# Install via winget
winget install Ollama.Ollama

# Or download the installer from https://ollama.com/download

# Start the server
ollama serve

# Pull a model
ollama pull qwen3.5:4b
```

Ollama runs on **port 11434** by default (`http://localhost:11434`).

To point the app at your own Ollama instance, open **Settings** and enter the Ollama URL.

### Home Assistant Core

Home Assistant Core runs as a Python application. It requires Python 3.12+.

#### macOS

```bash
# Create a virtual environment
python3 -m venv hass-venv
source hass-venv/bin/activate

# Install Home Assistant Core
pip install homeassistant

# If you encounter DNS resolver errors (pycares/aiodns), pin this version:
pip install pycares==4.11.0

# Start Home Assistant
hass
```

#### Windows

```powershell
# Create a virtual environment
python -m venv hass-venv
hass-venv\Scripts\activate

# Install Home Assistant Core
pip install homeassistant

# If you encounter DNS resolver errors (pycares/aiodns), pin this version:
pip install pycares==4.11.0

# Start Home Assistant
hass
```

Home Assistant runs on **port 8123** by default (`http://localhost:8123`).

On the first run, Home Assistant requires onboarding — you'll need to create an initial user account through its web interface before the app can connect.

To point the app at your own HA instance, open **Settings** and enter the HA URL and access token.

## Architecture

Sierra is a [Tauri v2](https://v2.tauri.app/) desktop app with a Rust backend and React/TypeScript frontend.

```
┌─────────────────────────────────────────────────────────────┐
│ React Frontend (thin client)                                │
│   Chat · Devices · Settings · Setup Wizard                  │
│             │ Tauri commands (IPC)                           │
├─────────────┼───────────────────────────────────────────────┤
│ Rust Backend│                                               │
│   ├── Ollama Service (streaming chat, tool calling loop)    │
│   ├── HA REST Client (service calls, state queries)         │
│   ├── HA WebSocket Client (real-time state subscriptions)   │
│   ├── Tool Registry (19 tools across 4 domains)             │
│   ├── MCP Server (HTTP, port 3001)                          │
│   ├── Device State Cache (in-memory, RwLock)                │
│   ├── Process Manager (child process lifecycle)             │
│   ├── Installer (dependency detection & auto-setup)         │
│   └── Config Store (persisted JSON, 0600 permissions)       │
├─────────────────────────────────────────────────────────────┤
│ Managed Services                                            │
│   Ollama (port 11434)  ·  Home Assistant (port 8123)        │
└─────────────────────────────────────────────────────────────┘
```

**Key design principles:**
- The frontend never calls external services directly — everything goes through Tauri commands to the Rust backend.
- All external service communication is behind trait abstractions, designed for future direct embedding.
- The tool registry is static and hand-authored — 19 tools across `light`, `switch`, `climate`, and `media_player` domains.
- Settings changes reconfigure live services without restart.

## Project Structure

```
sierra/
├── src/                          # React frontend
│   ├── App.tsx                   # Main shell, layout, setup gate
│   ├── commands/                 # Tauri command bindings
│   │   ├── chat.ts              # Chat streaming, health checks
│   │   ├── config.ts            # Settings persistence
│   │   ├── devices.ts           # Device queries and actions
│   │   └── setup.ts             # Dependency detection, installation
│   ├── views/
│   │   ├── ChatView.tsx          # Chat with tool call pills
│   │   ├── DevicesView.tsx       # Room-by-room device grid
│   │   ├── SettingsView.tsx      # HA/Ollama configuration
│   │   └── SetupView.tsx         # First-run setup wizard
│   └── styles/globals.css        # All styling (Liquid Glass theme)
├── src-tauri/                    # Rust backend
│   ├── src/
│   │   ├── lib.rs                # App initialization, lifecycle
│   │   ├── state.rs              # AppState (shared across commands)
│   │   ├── config.rs             # Config persistence
│   │   ├── devices.rs            # Device state cache
│   │   ├── prompts.rs            # System prompt builder
│   │   ├── commands/             # Tauri command handlers
│   │   ├── services/             # External service clients
│   │   │   ├── ollama.rs         # Ollama chat + tool calling
│   │   │   ├── ha_client.rs      # HA REST API client
│   │   │   ├── ha_ws.rs          # HA WebSocket client
│   │   │   ├── mcp_server.rs     # MCP HTTP server (axum)
│   │   │   ├── process_manager.rs # Child process lifecycle
│   │   │   ├── installer.rs      # Dependency auto-setup
│   │   │   └── tool_executor.rs  # HA tool execution bridge
│   │   └── tools/                # Static tool registry
│   │       ├── light.rs          # 3 tools
│   │       ├── switch.rs         # 3 tools
│   │       ├── climate.rs        # 5 tools
│   │       ├── media_player.rs   # 8 tools
│   │       └── registry.rs       # Registry loader
│   └── tauri.conf.json           # Build & bundle config
└── prompts/
    └── system-prompt.md          # LLM system prompt template
```

## Troubleshooting

### Ports Already in Use

If Ollama or Home Assistant fail to start, another process may be using the default port. The app detects externally-running instances and uses them instead of spawning new ones.

```bash
# Check if port 11434 (Ollama) is in use
lsof -i :11434    # macOS
netstat -ano | findstr :11434    # Windows

# Check if port 8123 (Home Assistant) is in use
lsof -i :8123    # macOS
netstat -ano | findstr :8123    # Windows
```

### Home Assistant Onboarding Error

If the app reports that HA is running but it can't connect, Home Assistant may be waiting for initial onboarding. Open `http://localhost:8123` in a browser and complete the setup wizard (create a user account). Then generate a long-lived access token and enter it in the app's Settings.

### Ollama Model Download Fails

Model downloads can fail due to disk space or network issues. Check:

- Available disk space (7B models need ~4 GB, 70B models need ~40 GB)
- Network connectivity
- Try pulling the model manually: `ollama pull <model-name>`

### Service Crashes

The app automatically restarts crashed services up to 3 times. After that, the service is marked as crashed and the health indicator turns red. You can restart manually from Settings, or check the logs for the underlying issue.

### Logs

Application logs are stored in the platform's standard log directory:

- **macOS:** `~/Library/Logs/Sierra/`
- **Windows:** `%APPDATA%\Sierra\logs\`

Two log files are maintained: `ollama.log` and `homeassistant.log`, capturing stdout/stderr from the managed child processes.

## Supported Devices

The app currently supports the following Home Assistant device domains:

| Domain | Actions |
|---|---|
| **Lights** | Turn on, turn off, toggle. Parameters: brightness (0-255), color temperature, RGB color, transition duration, effect |
| **Switches** | Turn on, turn off, toggle |
| **Climate** | Set temperature, set HVAC mode, set fan mode, turn on, turn off. Parameters: target temperature, high/low temperature range, HVAC mode (heat/cool/auto/off), fan mode |
| **Media Players** | Play, pause, stop, volume set/up/down, select source. Parameters: media content ID, content type, volume level (0.0-1.0), source |

More domains will be added over time, including locks, covers, fans, vacuums, cameras, alarm panels, scenes, scripts, and automations. Adding a new domain requires only a new tool definition file — no architectural changes.

## License

This project is open source. Ollama is licensed under MIT. Home Assistant is licensed under Apache 2.0.
