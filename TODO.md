# TODO

Incomplete features from the project roadmap.

---

## Expand Tool Registry (Future Domains)

The four priority domains (light, switch, climate, media_player) are complete. The following domains still need tool definition files authored and registered:

- [ ] `lock` — lock, unlock
- [ ] `cover` — open, close, set position, stop
- [ ] `fan` — turn on, turn off, set speed, set direction, oscillate
- [ ] `vacuum` — start, stop, return to base, locate, set fan speed
- [ ] `camera` — snapshot, turn on, turn off
- [ ] `alarm_control_panel` — arm home, arm away, arm night, disarm, trigger
- [ ] `scene` — activate
- [ ] `script` — turn on (run), turn off (cancel)
- [ ] `automation` — turn on, turn off, trigger

Each domain requires one Rust file following the existing schema format in `src-tauri/src/tools/`, plus registration in `registry.rs`.

---

## Embed Dependencies Directly

Long-term goal: bundle Ollama and Home Assistant source code directly into the Tauri app binary, eliminating the need for separate installs. Both projects use permissive licenses (MIT and Apache 2.0).

- [ ] Embed Ollama inference engine into the Rust binary (replace HTTP client with direct calls)
- [ ] Embed Home Assistant runtime into the Rust binary (replace REST/WebSocket client with direct calls)
- [ ] Remove child process management once both are embedded
- [ ] Update installer to skip external dependency detection/installation

The abstraction layers in `ha_client.rs` and `ollama.rs` are already designed for this swap — only the backend service clients need replacement.

---

## Linux Support

Linux is not currently targeted but fallback code paths exist throughout the Rust backend.

- [ ] Test and validate all installer paths on Linux (detection, download, venv setup)
- [ ] Test process lifecycle management on Linux
- [ ] Build and test `.deb` / `.AppImage` / `.rpm` installers via Tauri bundler
- [ ] Add Linux to the README as a supported platform
- [ ] CI pipeline for Linux builds

---

## Dynamic Sunrise Background Shift

The ambient background orbs are implemented but currently use static warm tones. CLAUDE.md specifies a time-of-day gradient arc:

- [ ] Cool dusty-rose predawn tones in early morning
- [ ] Warm orange-gold at peak sunrise
- [ ] Soft amber-peach through late morning
- [ ] Neutral warm stone tone during midday and evening
- [ ] Device activity influence (heating pushes tint warmer, high activity brightens orbs)
