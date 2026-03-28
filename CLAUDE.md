# CLAUDE.md

## Project Name

**Sierra** — a local, natural-language smart home controller.

## Project Overview

Sierra is a Tauri v2 desktop application that acts as an MCP (Model Context Protocol) server for smart home device control via a locally-hosted LLM. The app abstracts all smart home devices available on a network through Home Assistant and exposes them as callable tools to an LLM running on Ollama. Users interact through a chat interface — they say what they want, the model figures out which devices to control, and the app executes those actions.

The entire stack is local. No cloud dependencies. No subscriptions. Just a native desktop app that makes your home controllable through natural language.

---

## Architecture

### Three External Dependencies

- **Home Assistant (Apache 2.0):** Handles all device communication. Runs headless. The app talks to it via its REST API (one-off commands) and WebSocket API (real-time state subscriptions), authenticated with a long-lived access token. HA is the device layer — it speaks Zigbee, Z-Wave, WiFi, Bluetooth, etc. so this app doesn't have to.
- **Ollama (MIT):** Runs LLM inference locally. Acts as the MCP client — it connects to this app's MCP server over HTTP/SSE to discover available tools and invoke them.
- **LLM Model:** The actual model running inside Ollama (e.g., Llama, Mistral, etc.). The app is model-agnostic — any model Ollama supports that can handle tool calling should work.

### The App Itself (Tauri v2)

The app has two API surfaces managed by the Rust backend:

1. **Internal API (Tauri Commands):** The TypeScript frontend calls these. Every user interaction — sending a chat message, requesting device states, changing settings — goes through a Tauri command. The frontend never communicates with HA, Ollama, or anything external directly.

2. **External API (MCP Server over HTTP/SSE):** Ollama connects to this. The app serves tool definitions (derived from the static tool registry) and receives tool call requests from the model. The Rust backend translates these into HA API calls, executes them, and returns results to Ollama.

### Request Flow

```
User types message in UI
        │
        ▼
TS Frontend ──(Tauri command)──► Rust Backend
                                      │
                                      ▼
                              Sends message to Ollama
                                      │
                                      ▼
                              Ollama selects tool(s)
                                      │
                                      ▼
                              MCP tool call request
                                      │
                                      ▼
                              Rust Backend receives it,
                              translates to HA API call
                                      │
                                      ▼
                              Home Assistant executes
                              (light turns on, thermostat adjusts, etc.)
                                      │
                                      ▼
                              Result returned to Ollama
                                      │
                                      ▼
                              Ollama produces final response
                                      │
                                      ▼
                              Rust Backend returns to Frontend
                                      │
                                      ▼
                              UI renders response + device state change
```

---

## Tech Stack

### Rust Backend (src-tauri/)
- Owns ALL orchestration logic
- Runs the MCP server (HTTP/SSE transport)
- Manages HA connections (REST + WebSocket clients)
- Communicates with Ollama's API
- Exposes Tauri commands for the frontend
- Manages child processes (Ollama, HA) on app launch/shutdown
- Handles platform-specific logic (macOS vs Windows)

### TypeScript Frontend (src/)
- Thin client only — renders UI, calls Tauri commands
- Chat interface for LLM interaction
- Device state dashboard
- App configuration / settings
- Never imports or calls any external service directly
- All data comes from Rust backend via Tauri commands

### External Services (managed as child processes)
- Home Assistant Core (Python) — device communication layer
- Ollama (Go) — local LLM inference runtime

---

## Tool Registry

The tool registry is a static, schema-defined collection of all HA service domain actions the LLM can invoke. It is the bridge between what HA can do and what the model sees as available tools.

### Schema Format

Every tool definition must follow this structure:

- **Domain:** The HA device category (e.g., `light`, `climate`, `switch`, `media_player`)
- **Service:** The specific action within that domain (e.g., `turn_on`, `set_temperature`, `play_media`)
- **Parameters:** Typed parameter list with names, types, whether required or optional, and valid ranges/enums where applicable
- **Description:** A brief, plain-English explanation of what the tool does and when to use it. This is what the LLM reads to decide which tool fits the user's intent. Write these as if explaining to a capable person who has never seen the HA API.

### Authoring Rules

- One file per domain. All services for that domain live in the same file.
- Descriptions must be concise but unambiguous. The LLM should be able to distinguish between similar tools (e.g., `light.turn_on` with brightness vs `light.toggle`) from the description alone.
- Parameter descriptions should include valid ranges and units where relevant (e.g., "brightness: integer, 0-255" or "temperature: float, in degrees Fahrenheit or Celsius depending on HA config").
- If a parameter accepts an enum, list all valid values.

### Priority Domains (implement first)

1. **light** — turn_on, turn_off, toggle (params: brightness, color_temp, rgb_color, transition, effect)
2. **switch** — turn_on, turn_off, toggle
3. **climate** — set_temperature, set_hvac_mode, set_fan_mode, turn_on, turn_off (params: temperature, target_temp_high, target_temp_low, hvac_mode, fan_mode)
4. **media_player** — play_media, media_pause, media_play, media_stop, volume_set, volume_up, volume_down, select_source (params: media_content_id, media_content_type, volume_level, source)

### Expanding the Registry

After the priority domains are stable and tested end-to-end, add domains incrementally:
- lock, cover, fan, vacuum, camera, alarm_control_panel, scene, script, automation, etc.

Adding a new domain requires NO architectural changes. Just author a new tool definition file following the schema format and register it. The MCP server picks it up and exposes it to the LLM.

---

## Build & Distribution

### Installer Format

The app must be distributed as a native installer. Never as a "clone and run" script.

- **macOS:** `.dmg`
- **Windows:** `.exe` or `.msi`

Tauri v2's built-in bundler handles producing these formats. All final build scripts must output a native installer for the target OS.

### First-Run Dependency Setup (Beta Milestone)

On first launch, the app must detect whether Ollama and Home Assistant are already installed on the user's machine. If either is missing, the app downloads and installs them automatically. The user should never have to open a terminal.

**This logic is platform-specific.** The install and startup procedures for Ollama and HA differ between macOS and Windows. When writing any code related to dependency detection, installation, or process management, always account for both platforms with separate code paths.

- **macOS:** May use Homebrew, direct binary downloads, pip in a managed venv, etc.
- **Windows:** May use winget, direct installer downloads, pip in a managed venv, etc.

### Process Lifecycle

The app manages Ollama and Home Assistant as child processes:

- **On launch:** Start both services as child processes. Wait for them to be healthy (accepting connections) before considering the app ready.
- **On exit:** Gracefully shut down both child processes. Do not leave orphaned Ollama or HA processes running.
- **On crash:** Handle unexpected child process termination. Attempt restart or surface an error to the user.

---

## Target Platforms

- macOS (primary)
- Windows (primary)
- Linux: not targeted for now

---

## Future Roadmap

### Embed Dependencies Directly

Long-term goal: bundle Ollama and Home Assistant source code directly into the Tauri app binary, eliminating the need for separate installs entirely. Both projects use permissive licenses (MIT and Apache 2.0 respectively) that allow redistribution and embedding.

**This is why the Rust backend must abstract all communication with Ollama and HA behind clean interfaces.** The current implementation talks to them over HTTP/WebSocket. A future version may call embedded inference engines or an embedded HA runtime directly. If the abstraction boundary is clean, this swap requires no changes to the frontend, the tool registry, or the MCP server logic — only the backend service clients get replaced.

Do not build tight coupling to HTTP clients. Always go through an abstraction layer.

---

## README Guidelines

The README.md is user-facing documentation. It should be written for someone who has just downloaded the app or found the repo — not for contributors or developers. It must cover the following sections:

### What the App Does
- One-paragraph explanation of what this is: a local, natural-language smart home controller
- Emphasize: fully local, no cloud, no subscriptions, no accounts

### Requirements
- Supported operating systems (macOS, Windows)
- Minimum hardware recommendations (enough RAM to run an LLM locally — this depends on model size, so give general guidance)
- Note that Ollama and Home Assistant are required but the app handles installing them automatically on first run

### Installation
- Download the installer for your platform (`.dmg` for macOS, `.exe`/`.msi` for Windows)
- Run it. That's it. No terminal commands.
- Explain what happens on first launch: the app detects missing dependencies and installs them

### Setup
- How to connect the app to Home Assistant (entering the HA URL, generating a long-lived access token)
- How to select/download an LLM model through the app (which models work well, recommended defaults)
- First-time device discovery — what happens when the app reads your HA instance's available devices

### Running Ollama and Home Assistant Manually
- If a user already has Ollama or HA running, explain how the app detects this and skips auto-install
- If a user wants to run them independently (outside the app's child process management), document how:
  - **Ollama:** How to install standalone, how to start the server (`ollama serve`), default port, how to pull a model
  - **macOS vs Windows** differences for Ollama setup
  - **Home Assistant Core:** How to install in a Python venv, how to start it (`hass`), default port, known issues (e.g., pycares/aiodns compatibility — pin `pycares==4.11.0` if DNS resolver errors occur)
  - **macOS vs Windows** differences for HA setup
- How to point the app at externally-running instances instead of letting it manage them as child processes

### Usage
- How to chat with the app to control devices
- Example commands and what happens ("Turn off the living room lights", "Set the thermostat to 72", "Pause the music")
- How to view device states in the dashboard

### Troubleshooting
- Common first-run issues (ports already in use, HA onboarding errors, Ollama model download failures)
- How to check if HA and Ollama are running and healthy
- Where logs are stored

### Supported Devices
- List the currently supported HA domains (lights, switches, climate, media players for the initial release)
- Note that more domains will be added over time
- Link to or reference the tool registry for the full list of supported actions

---

## Key Design Principles

1. **Frontend is a thin client.** It calls Tauri commands and renders responses. It never touches external services. If you're writing an HTTP call in TypeScript, you're doing it wrong.

2. **Rust backend is the single point of orchestration.** All external communication — HA, Ollama, MCP — flows through the Rust layer. Two API surfaces: Tauri commands (internal, frontend-facing) and MCP server (external, Ollama-facing).

3. **Tool registry is the contract.** The schema format for tool definitions is sacred. Once defined, every domain follows it exactly. This is what makes the system extensible without architectural changes.

4. **Platform-aware from day one.** Any code that touches the filesystem, installs dependencies, manages processes, or interacts with the OS must have separate macOS and Windows code paths. Never assume Unix.

5. **Design for future embedding.** Every external service interaction goes through an abstraction layer. Today it's HTTP. Tomorrow it might be a direct function call to an embedded runtime. The rest of the app shouldn't care.

6. **Everything is local.** No cloud services. No telemetry. No accounts. The app runs entirely on the user's machine.

---

## UI Design

### Design Language

Sierra follows Apple's Liquid Glass aesthetic — translucent surfaces, backdrop blur, capsule-shaped controls, and content-first hierarchy. The UI should feel like it belongs on macOS Tahoe. On Windows, the same visual language applies (Chromium's webview supports all the required CSS); we are not chasing Fluent Design or platform-native chrome. One look, both platforms.

The visual identity is inspired by a Sierra Nevada sunrise: warm, luminous, and alive. This is a lighter theme than a typical glass UI — not a dark cave, but the first light cresting over granite ridgelines. The ambient background is a soft, warm off-white or pale stone tone, with drifting color orbs in sunrise hues (deep orange, amber, rose-gold, and pale gold) that give the translucent panels something warm to refract against. Glass effects on a light base require higher opacity and careful contrast management — treat them as frosted alpine air, not smoked glass.

The background should subtly shift across a sunrise gradient arc throughout the day: cool dusty-rose predawn tones in the early morning, warm orange-gold at peak sunrise, soft amber-peach through late morning, and settling into a neutral warm stone tone during midday and evening. Device activity can also influence the warmth — heating running pushes the tint warmer, high device activity brightens the orbs slightly.

### Glass Hierarchy

Three tiers of translucency, used consistently:

On a light warm base, glass tiers use warm-tinted whites and higher opacity than a dark-base design — frosted alpine air, not black mirror.

- **Glass Subtle:** Structural chrome — sidebar, device panel, status bar. Lightest blur, low opacity warm white. `backdrop-filter: blur(16px); background: rgba(255,248,240,0.45); border: 0.5px solid rgba(210,160,100,0.18)`
- **Glass Standard:** Interactive elements — device cards, input capsule, AI message bubbles. Medium blur, slightly more opaque. `backdrop-filter: blur(24px); background: rgba(255,248,240,0.60); border: 0.5px solid rgba(210,160,100,0.25)`
- **Glass Strong:** Emphasis — modals, active selections, user message bubbles. Strongest blur, most visible. `backdrop-filter: blur(32px); background: rgba(255,248,240,0.75); border: 0.5px solid rgba(210,160,100,0.35)`

Never mix tiers arbitrarily. If an element is structural, it's always Subtle. If it's interactive, it's always Standard. Promote to Strong only for things that need to visually pop above everything else.

All borders are `0.5px solid rgba(210,160,100, ...)` — a warm amber-tan that reads as a sun-lit edge. No solid borders. No box shadows except functional focus rings.

### Layout

Three-panel layout:

```
┌──────────┬─────────────────────────┬──────────────┐
│          │                         │              │
│ Sidebar  │      Main Area          │ Device Panel │
│ (220px)  │      (flex: 1)          │  (240px)     │
│          │                         │              │
│ Nav      │  Titlebar               │ Active tab   │
│ items    │  ───────────────────    │ filter       │
│          │                         │              │
│          │  Chat messages /        │ Device cards │
│          │  Device dashboard /     │ grouped by   │
│          │  Settings view          │ room         │
│          │                         │              │
│          │  ───────────────────    │              │
│ Service  │  Input capsule          │              │
│ health   │                         │              │
└──────────┴─────────────────────────┴──────────────┘
```

**Sidebar (left, 220px):** App identity at top. Navigation items below (Chat, Devices, Settings). Service health indicators pinned to the bottom — HA, Ollama, and active model, each with a colored status dot (green = connected, amber = loading/degraded, red = down). The sidebar is always visible. It uses Glass Subtle.

**Main Area (center, flexible):** Hosts the active view. A thin titlebar at top shows the current context and a status pill (e.g., "14 devices"). Below that, the view content fills the space. At the bottom, the input capsule is always present in the Chat view. The main area has no glass treatment — it's the content canvas.

**Device Panel (right, 240px):** A live device state sidebar. Glass Subtle background. Tab filter at top (Active / All / Rooms). Device cards stacked vertically, grouped by room. Each card shows device name, current state, and a toggle or status indicator. This panel is collapsible — the user can dismiss it to give the chat full width. It should also auto-collapse on narrow window sizes (below ~900px).

### Views

**Chat View (default):** The primary interface. Conversation fills the center column. User messages align right with a warm orange-tinted glass treatment (sunrise-shifted). AI messages align left with neutral warm glass. When the LLM executes a tool call, an inline status pill appears below the AI message confirming what happened — capsule-shaped, briefly pulses on creation, then settles. The user never needs to check the device panel to know an action succeeded.

**Devices View:** Replaces the center column with a room-by-room grid of device cards. Larger cards than the sidebar panel, with controls inline — brightness sliders for lights, temperature controls for climate, transport controls for media. This is for users who want direct manipulation without going through the chat.

**Settings View:** Connection configuration for HA (URL + token) and Ollama (URL + model selection). Process management controls (restart HA, restart Ollama). Model pull/download interface. All in-app — no terminal required.

### Controls & Components

**Input Capsule:** Capsule-shaped (border-radius: 24px), Glass Standard, with a circular send button on the right. Placeholder text: "Tell me what to do..." The capsule expands focus state slightly (border brightens, background opacity increases). This is the single most-used element in the app — it must feel responsive and inviting.

**Device Cards:** Glass Standard, border-radius: 14px. Header row: device name (left) + toggle or status icon (right). Below: state text (e.g., "60% · 2700K warm") and optional control (brightness bar, temperature readout). Cards are interactive — clicking opens an expanded view with full controls.

**Toggle Switches:** 36×20px track, 16px thumb. Off = `rgba(180,140,100,0.25)` track (warm stone). On = sunrise-orange track (`rgba(224,120,50,0.55)`). Thumb is always `rgba(255,255,255,0.95)`. 250ms transition.

**Status Dots:** 6px circles with a matching color glow (`box-shadow: 0 0 6px`). Green for healthy, amber for degraded/loading, red for disconnected. Used in sidebar service health and inline where connection state matters.

**Action Confirmation Pills:** Capsule-shaped, appear inline in chat after a tool call executes. Green-tinted glass for success, red-tinted for failure. Include a checkmark/X icon and the action name (e.g., "light.turn_on executed"). Pulse animation on creation (one cycle, ~1s), then static.

### Typography

System font stack: `-apple-system, BlinkMacSystemFont, 'SF Pro Display', 'Segoe UI', system-ui, sans-serif`. No custom font loading. The app should feel native, not branded.

- Body text: 13.5px, line-height 1.55, `rgba(60,35,15,0.9)` (deep warm brown — legible on warm white glass)
- Secondary text (device states, timestamps): 11-12px, `rgba(120,80,40,0.6)`
- Titles (sidebar, panel headers): 14px, weight 500, `rgba(60,35,15,0.85)`
- Section labels: 11px, weight 500, uppercase, letter-spacing 0.06em, `rgba(160,100,50,0.55)`
- Temperature / large readouts: 20px, weight 300, `rgba(60,35,15,0.8)`

### Color

Sierra is light-only. No dark mode. The sunrise glass aesthetic is built on a warm, luminous base — a dark mode would require a fundamentally different design language, not just an inverted palette.

The base background is a warm off-white with a subtle stone undertone: `hsl(30, 40%, 96%)` to `hsl(25, 50%, 92%)`. Ambient color orbs drift in sunrise hues underneath the glass panels.

Accent palette (used sparingly — these are the colors of first light on granite):
- **Sunrise Orange** (`rgba(224,108,40,...)`): User messages, send button, primary actions. The dominant accent — warm, energetic, unmistakably Sierra.
- **Amber Gold** (`rgba(210,155,40,...)`): Secondary highlights, active states, warm device indicators. The color of alpenglow.
- **Rose** (`rgba(210,100,80,...)`): Warnings, elevated attention states. Predawn pink on the peaks.
- **Sage Green** (`rgba(100,150,110,...)`): Success states, "on" toggles, healthy connections. The color of high-elevation pine.
- **Red** (`rgba(200,60,50,...)`): Errors, disconnected states, failure pills.

These are used at low opacity for backgrounds and borders, full opacity only for small indicators (dots, icons). Never fill large surfaces with accent colors — the warmth comes from the background, not the UI chrome.

### Animation

Minimal, purposeful, fast.

- **Message entry:** 300ms ease-out, translateY(8px) → 0 + opacity 0 → 1. Messages should feel like they're settling into place, not flying in.
- **Action confirmation pulse:** Single box-shadow pulse on creation, 1s ease-out. Draws attention without looping.
- **Toggle state change:** 250ms transition on track color and thumb position. Snappy, not bouncy.
- **Background gradient drift:** 12-15s ease-in-out infinite loops on the ambient color orbs. Slow enough to be subliminal. Users should not consciously notice the background moving.
- **Panel collapse/expand:** 200ms ease-out on width. Content fades simultaneously.

No spring physics. No bounce. No parallax. No particle effects. The glass aesthetic is already visually rich — animation should be restrained to avoid feeling overwrought.

Always wrap ambient/looping animations in `@media (prefers-reduced-motion: no-preference)`. Action confirmations and state transitions are fine without the media query since they're functional, not decorative.

### Streaming LLM Responses

The AI message bubble should appear immediately when the response starts streaming, with text rendering token-by-token. The bubble grows in height as content arrives — no fixed height, no scroll-within-the-bubble. The chat area auto-scrolls to keep the latest content visible.

If the model emits a tool call mid-response, the confirmation pill should appear inline at the point in the stream where the tool call was made, not appended after the full response arrives. This means the frontend needs to parse the streaming response for tool call boundaries and insert UI elements in real-time.

While a response is streaming, the input capsule should show a subtle loading state (pulsing border or a thin progress indicator) and disable input. Once the response completes, focus returns to the input immediately.

### Responsive Behavior

The app targets desktop, but window sizes vary:

- **≥1100px:** Full three-panel layout.
- **900-1099px:** Device panel collapses to an icon-toggle in the titlebar. Click to slide it in as an overlay.
- **<900px:** Sidebar collapses to icons only (no labels). Device panel hidden, accessible via titlebar toggle.

The chat view's input capsule and message area must remain usable at all sizes. Never sacrifice the primary interaction surface for secondary panels.

### Accessibility

Glass effects on a light base can wash out dark text if opacity is too low. Mitigate aggressively:

- All text on glass surfaces must meet WCAG AA contrast (4.5:1 for body text, 3:1 for large text). Test against the lightest expected background state (peak-brightness ambient orb directly behind the panel).
- The warm brown text palette (`rgba(60,35,15,0.9)` on warm white glass) must be validated — do not assume it passes; measure it.
- Provide a "Reduce transparency" toggle in Settings that replaces glass backgrounds with solid warm surfaces (`rgba(250,242,230,0.98)`) while keeping the same layout. Respect the OS-level "Reduce transparency" preference automatically.
- Focus indicators must be clearly visible — use a 2px solid ring in the sunrise orange, offset by 2px from the element edge.
- All interactive elements must be keyboard-navigable. Tab order follows the visual layout: sidebar → main area → device panel.
- Device toggles must be operable via keyboard (Space/Enter) and have appropriate ARIA roles (`role="switch"`, `aria-checked`).

### What NOT to Do

- Do not add a dark mode. The Sierra design is light-and-warm by nature. A dark mode would require a fundamentally different aesthetic, not just an inversion.
- Do not use CSS gradients on glass surfaces. The translucency IS the visual interest. Adding gradients on top muddies it.
- Do not put device controls in the chat flow. The chat is for natural language. Direct manipulation goes in the device panel or devices view. The only device-related UI in chat is the read-only confirmation pill.
- Do not animate glass opacity/blur values. Transitioning `backdrop-filter` is GPU-expensive and janky on most hardware. Animate transforms and opacity on the container instead.
- Do not use rounded corners above 24px except on the input capsule. Overly rounded elements fight the glass layering — they look bubbly instead of refined.
- Do not introduce custom icons or an icon font. Use inline SVGs from a consistent set (Lucide or similar). Keep strokes at 1.5px to match the thin glass borders.
- Do not use cool or neutral greys. Every surface tone should lean warm. A neutral `rgba(150,150,150)` will look dead against the sunrise palette — use warm stone equivalents instead.