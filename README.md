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

1. Download the installer for your platform:
   - **macOS:** `.dmg` file
   - **Windows:** `.exe` or `.msi` installer
2. Run the installer. No terminal commands required.

### First Launch

When you open the app for the first time, it will:

1. Check if Ollama is installed. If not, it downloads and installs it automatically.
2. Check if Home Assistant Core is installed. If not, it sets it up in a managed Python environment.
3. Start both services as background processes.
4. Wait for both services to be healthy before showing the main interface.

This may take a few minutes on the first run, especially if Ollama needs to download a model. Subsequent launches are fast — the app just starts the services and waits for them to accept connections.

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
4. Paste the token into the app's Settings and save.

### Selecting an LLM Model

1. Open **Settings** from the sidebar.
2. Under the Ollama section, browse available models or enter a model name.
3. Click **Download** to pull the model. Progress is shown in the app.

**Recommended models for smart home control:**

- **Llama 3.1 8B** — Good balance of speed and accuracy. Works well on 8-16 GB RAM.
- **Mistral 7B** — Fast, strong tool-calling support.
- **Llama 3.1 70B** — Best accuracy, but requires 64 GB+ RAM.

Any model Ollama supports that handles tool calling should work. Smaller models are faster but may occasionally misinterpret complex requests.

### Device Discovery

Once connected to Home Assistant, the app automatically reads all available devices from your HA instance. You'll see them populate in the **Device Panel** on the right side of the window and in the **Devices** view. Devices are grouped by room (as configured in Home Assistant).

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

When the LLM executes a device action, an inline confirmation pill appears in the chat showing what happened (e.g., "light.turn_on executed"). You don't need to check the device panel to know an action succeeded.

### Devices View

Click **Devices** in the sidebar to see a room-by-room grid of all your devices with direct controls:

- **Lights:** Toggle on/off, adjust brightness slider, change color temperature
- **Switches:** Toggle on/off
- **Climate:** Set temperature, change HVAC mode, adjust fan mode
- **Media players:** Play/pause/stop, adjust volume, select source

### Device Panel

The right sidebar shows a live summary of device states. Use the tabs at the top to filter:

- **Active** — devices currently on or in use
- **All** — every discovered device
- **Rooms** — grouped by room

The panel collapses automatically on smaller window sizes, or you can dismiss it manually to give the chat more space.

## Running Ollama and Home Assistant Manually

If you already have Ollama or Home Assistant running on your machine, the app detects them and skips auto-installation. You can also run them independently and point the app at your existing instances.

### Ollama

#### macOS

```bash
# Install via Homebrew
brew install ollama

# Or download directly from https://ollama.com/download

# Start the server
ollama serve

# Pull a model
ollama pull llama3.1:8b
```

#### Windows

```powershell
# Install via winget
winget install Ollama.Ollama

# Or download the installer from https://ollama.com/download

# Start the server
ollama serve

# Pull a model
ollama pull llama3.1:8b
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

## Troubleshooting

### Ports Already in Use

If Ollama or Home Assistant fail to start, another process may be using the default port.

```bash
# Check if port 11434 (Ollama) is in use
lsof -i :11434    # macOS
netstat -ano | findstr :11434    # Windows

# Check if port 8123 (Home Assistant) is in use
lsof -i :8123    # macOS
netstat -ano | findstr :8123    # Windows
```

Stop the conflicting process or configure the app to use a different port in **Settings**.

### Home Assistant Onboarding Error

If the app reports that HA is running but it can't connect, Home Assistant may be waiting for initial onboarding. Open `http://localhost:8123` in a browser and complete the setup wizard (create a user account). Then generate a long-lived access token and enter it in the app's Settings.

### Ollama Model Download Fails

Model downloads can fail due to disk space or network issues. Check:

- Available disk space (7B models need ~4 GB, 70B models need ~40 GB)
- Network connectivity
- Try pulling the model manually: `ollama pull <model-name>`

### Checking Service Health

The sidebar shows service health at the bottom:

- Green dot — connected and healthy
- Amber dot — loading or degraded
- Red dot — disconnected or crashed

You can also check manually:

```bash
# Ollama health check
curl http://localhost:11434/api/tags

# Home Assistant health check
curl http://localhost:8123/api/ -H "Authorization: Bearer YOUR_TOKEN"
```

### Logs

Application logs are stored in the platform's standard log directory:

- **macOS:** `~/Library/Logs/Sierra/`
- **Windows:** `%APPDATA%\Sierra\logs\`

Ollama and Home Assistant also produce their own logs in the terminal where they were started, or in the app's managed log directory if the app started them.

## Supported Devices

The app currently supports the following Home Assistant device domains:

| Domain | Actions |
|---|---|
| **Lights** | Turn on, turn off, toggle. Parameters: brightness (0-255), color temperature, RGB color, transition duration, effect |
| **Switches** | Turn on, turn off, toggle |
| **Climate** | Set temperature, set HVAC mode, set fan mode, turn on, turn off. Parameters: target temperature, high/low temperature range, HVAC mode (heat/cool/auto/off), fan mode |
| **Media Players** | Play, pause, stop, volume set/up/down, select source. Parameters: media content ID, content type, volume level (0.0-1.0), source |

More domains will be added over time, including locks, covers, fans, vacuums, cameras, alarm panels, scenes, scripts, and automations.

## License

This project is open source. Ollama is licensed under MIT. Home Assistant is licensed under Apache 2.0.
