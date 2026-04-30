# Cable

[한국어 README](./README-ko.md)

<p align="center">
  <img src="./docs/images/readme-diagram.jpg" alt="Cable node graph diagram" width="720" />
</p>

A real-time audio routing desktop app for Windows.Build a node graph that connects physical and virtual audio devices, and create virtual audio endpoints on demand through a custom kernel driver.

> **Warning**
> Cable is in early development. The virtual audio device feature requires Windows test signing to be enabled.

---

## Project Summary

Cable lets you wire any audio source to any audio sink on Windows from a visual node graph. Built-in nodes cover physical input/output devices (via `cpal`), virtual devices created by the bundled CableAudio kernel driver, app loopback capture (WASAPI), VST3 plugin hosting, and a small library of utility processors (Mixer, Gain, Channel Split/Merge, Delay, Compressor, Reverb, Echo, Spectrum Analyzer, Waveform Monitor).

The app is built on Tauri v2: a React + React Flow frontend drives a Rust audio runtime that processes the graph in topological order on a sub-millisecond spin-loop thread. A type system on the frontend statically validates each connection (channels / sample rate / bit depth) before the graph is pushed to the runtime, so only fully-validated graphs ever reach the audio engine.

---

## Getting Started

### Install a release

1. Download the latest `Cable_<version>_x64-setup.exe` from the [Releases page](https://github.com/SieR-VR/cable/releases).
2. Run the installer.
3. (Optional, for virtual audio devices) Enable Windows test signing — see [Notes](#notes).
4. Launch **Cable** from the Start Menu.

The first launch creates two placeholder nodes (Audio Input Device → Audio Output Device). Right-click the canvas to add more nodes; drag from one handle to another to connect.

### System requirements

- Windows 10 (1809+) or Windows 11
- x64 CPU
- For virtual audio devices: test-signing mode enabled and the CableAudio driver installed

---

## Features

- **Visual node graph** — React Flow canvas with drag-to-connect, right-click context menus, and per-edge type badges
- **Physical I/O** — Enumerate and route any WASAPI / DirectSound / WDM device through `cpal`
- **Virtual audio devices** — Create render and capture endpoints on the fly via the CableAudio kernel driver; rename them with UAC elevation
- **App loopback capture** — Capture audio from a specific running process (per-app capture, Windows 10 2004+)
- **VST3 host** — Load and run third-party VST3 plugins inside the graph
- **Built-in processors** — Mixer, Gain, Channel Split / Merge (2/4/6/8 channels), Delay, Compressor, Reverb, Echo
- **Visualizers** — Spectrum Analyzer (FFT) and Waveform Monitor (oscilloscope-style)
- **Static type checking** — Every edge is validated against the format expected by its sink; mismatched edges are highlighted and the graph is held back from the runtime until valid
- **Save / load** — Persist a graph as JSON and drag-and-drop to reload
- **Sub-millisecond runtime** — Topological-order spin-loop processing thread for low-jitter audio routing

---

## Notes

- **Test signing for virtual devices.** The CableAudio kernel driver is currently signed with a self-generated test certificate. To install it you must enable test mode:
  ```powershell
  bcdedit /set testsigning on
  ```
  followed by a reboot. A production-signed driver is on the roadmap.
- **Buffer size.** The runtime defaults to a 512-frame buffer at the device's native sample rate. Smaller buffers reduce latency but increase the chance of underruns on heavily loaded graphs.
- **Virtual device naming.** Renaming a virtual endpoint writes to `PKEY_Device_FriendlyName`, which requires administrator privileges. Cable re-launches itself with elevation only for the rename action; the main window does not need admin rights.
- **Graph validity gate.** If any node fails validation (e.g. a stereo source feeding a mono sink), the runtime keeps running the last fully-valid graph. Fix the offending edge or node and the new graph is pushed automatically.
- **Licensing.** App code (`src/`, `crates/`) is GPL-3.0; the kernel driver (`driver/`) is MS-PL, derived from the Microsoft WDK samples and [VirtualDrivers/Virtual-Audio-Driver](https://github.com/VirtualDrivers/Virtual-Audio-Driver). See [LICENSE](./LICENSE).

---

## Build Instructions

### Prerequisites

- [Rust](https://rustup.rs) (stable, with `x86_64-pc-windows-msvc`)
- [Node.js](https://nodejs.org) ≥ 20
- [pnpm](https://pnpm.io) ≥ 10
- [Visual Studio Build Tools](https://visualstudio.microsoft.com/downloads/) with the **Desktop development with C++** workload
- [Windows Driver Kit (WDK)](https://learn.microsoft.com/en-us/windows-hardware/drivers/download-the-wdk) — only needed if you build the driver

### Common workflows

```powershell
# Install JS deps
pnpm install

# Run the app in dev mode (frontend HMR + Tauri)
pnpm tauri dev

# Frontend-only dev server (no Rust)
pnpm dev

# Production app build
pnpm tauri build

# One-shot full build (driver + frontend + Tauri app)
.\scripts\build.ps1

# Driver only (requires WDK)
.\scripts\build.ps1 -Target Driver

# App only (skip driver)
.\scripts\build.ps1 -Target App
```

### Quality gates

```powershell
pnpm lint            # oxlint
pnpm fmt:check       # oxfmt --check
pnpm test            # Vitest (frontend)
cargo test --workspace   # Rust unit + ABI tests
cargo fmt            # rustfmt
```

### VM-based driver tests

Driver integration tests run inside a VMware guest. Prerequisites: VMware Workstation + `vmrun` on PATH, driver build artifacts under `driver/x64/Debug/package/`, and a `.env` with `VM_PASSWORD=...`.

```powershell
.\.vm\setup.ps1                      # one-time VM provisioning
.\.vm\test.ps1                       # run all Pester suites
.\.vm\test.ps1 -TestFilter "*IOCTL*" # filter by Pester FullName
.\.vm\exec.ps1 "Get-PnpDevice -Class MEDIA"
```

---

## Roadmap

- [ ] Production-signed kernel driver (no more test-signing required)
- [ ] Per-edge sample rate conversion and bit-depth dithering
- [ ] MIDI-style modulation graph (control-rate edges) alongside audio edges
- [ ] More built-in processors: parametric EQ, limiter, noise gate, convolution reverb
- [ ] LV2 / CLAP plugin hosts in addition to VST3
- [ ] Multi-graph project files with per-graph enable/disable
- [ ] macOS port (Core Audio backend; virtual device support TBD)
- [ ] Real-time CPU and latency telemetry per node
