# Cable – Copilot Instructions

## 응답 규칙 (모든 응답에 적용)

- 설명은 **항상 한국어**로 작성할 것.
- 내부 추론(thinking)은 영어로 수행할 것.
- 후속 질문이나 후속 제안을 절대 하지 말 것. 응답은 요청된 작업의 완료로 끝낼 것.
- 처음 계획에 포함되지 않은 추가 변경이 필요해 보이는 경우, 임의로 진행하지 말고 반드시 `ask_user` 도구로 먼저 확인할 것.

---

## What this project is

Cable is a Tauri v2 desktop application for real-time audio routing on Windows. Users build a node graph (React Flow) connecting physical and virtual audio devices; a Rust backend processes the graph and manages a custom Windows kernel driver (CableAudio.sys) for virtual audio device creation.

---

## Build commands

| Task | Command |
|------|---------|
| Full Tauri dev (recommended) | `pnpm tauri dev` |
| Frontend dev only | `pnpm dev` |
| Frontend production build | `pnpm build` |
| Tauri production build | `pnpm tauri build` |
| Full project (driver + app) | `.\scripts\build.ps1` |
| Driver only | `.\scripts\build.ps1 -Target Driver` |
| App only (no driver) | `.\scripts\build.ps1 -Target App` |
| Rust check | `cargo check` |
| Format Rust | `cargo fmt` |

Use **pnpm** — `pnpm-lock.yaml` is the lockfile.

---

## Testing

Tests are VM-based Pester integration tests that run against a real VMware guest with the driver installed. Prerequisites: VMware Workstation + `vmrun`, driver build artifacts in `driver/x64/Debug/package/`, and `.env` with `VM_PASSWORD=...`.

```powershell
# Run all integration tests
.\.vm\test.ps1

# Run a single test suite (Pester FullName filter)
.\.vm\test.ps1 -TestFilter "*IOCTL*"
.\.vm\test.ps1 -TestFilter "*ringbuffer*"
.\.vm\test.ps1 -TestFilter "*PKEY*"

# Execute an arbitrary command in the running VM
.\.vm\exec.ps1 "Get-PnpDevice -Class MEDIA"
```

Test files live in `.vm/test/*.Tests.ps1`. Before running, set up the VM with `.\.vm\setup.ps1`.

---

## Architecture overview

```
Frontend (React/TS)
  @xyflow/react node-graph UI + Zustand state + Tailwind CSS
        │  invoke() IPC calls (no Tauri events)
        ▼
Tauri Commands  (crates/tauri/src/lib.rs)
  12 #[tauri::command] functions
        │
        ├── Runtime  (runtime.rs)
        │     Topological-order audio graph processor
        │     Spin-loop thread for sub-ms timing accuracy
        │
        ├── Nodes  (nodes/)
        │     AudioInputDevice / AudioOutputDevice  (cpal)
        │     VirtualAudioInput / VirtualAudioOutput  (driver ring buffer)
        │
        └── DriverClient  (driver_client.rs, #[cfg(windows)])
              Win32 SetupDi + DeviceIoControl → CableAudio.sys IOCTLs

Common crate  (crates/common)  — #![no_std]
  Shared ABI types between kernel driver and userspace:
  RingBufferHeader, AudioFormat, DeviceControlPayload, IOCTL codes
  C mirror: driver/Source/Inc/cable_common.h
```

**Runtime timing:** The audio processing thread uses a spin-loop with `Instant` (not `thread::sleep`) because Windows timer resolution (~15.6 ms default) causes systematic underruns at small buffer sizes.

**Virtual device rename:** Requires UAC elevation. The app re-launches itself as `cable-ui.exe --rename-endpoint <id> <name>`, performs the COM property write, and exits without opening the Tauri window. See `main.rs` and `rename_endpoint_elevated()` in `lib.rs`.

---

## Key conventions

### Rust

- **Rust edition 2024**, 2-space indentation (`rustfmt.toml`).
- All types serialized to the frontend use `#[serde(rename_all = "camelCase")]`.
- `AudioNode` enum uses `#[serde(tag = "type", content = "data")]` (adjacent tagging) to match the frontend's `{ type: "audioInputDevice", data: {...} }` JSON shape.
- `driver_client.rs` is gated behind `#[cfg(windows)]` — do not remove that guard.
- Tauri commands return `Result<T, String>`; errors are surfaced to the frontend as strings.
- `AppData` is held in a `Mutex<AppData>` Tauri `State`. Acquire the lock only for the duration of a command; audio processing runs on its own thread.

### Adding a new audio node type

1. Add a variant to the `AudioNode` enum in `crates/tauri/src/lib.rs`.
2. Implement `NodeTrait` (`id`, `init`, `dispose`, `process`) in a new file under `crates/tauri/src/nodes/`.
3. Register the module in `crates/tauri/src/nodes/mod.rs`.
4. Add a React component under `src/nodes/`.
5. Register the component in the `nodeTypes` object in `src/types.ts`.
6. Add typed `invoke()` overloads in `src/ipc.d.ts` for any new IPC commands.

### NodeTrait contract

```rust
fn process(&mut self, runtime: &Runtime, state: &RuntimeState)
    -> Result<BTreeMap<String, Vec<f32>>, String>
```

- Returns `edge_id → f32 samples` for every outgoing edge.
- Downstream nodes read from `state.edge_values` by edge ID.
- The runtime calls nodes in **topological order** each tick; cycles fall back to UI order.

### Frontend state

- All app state lives in `useAppStore` (`src/state.ts`) — a single `createWithEqualityFn` Zustand store.
- IPC calls belong in store actions, not component code.
- `ipc.d.ts` augments `@tauri-apps/api/core` `invoke` with per-command typed overloads; keep it in sync when adding new commands.

### Common crate constraints

- Must remain `#![no_std]` — no `std` imports.
- All structs shared with the driver must be `#[repr(C, packed)]` to match the C ABI in `cable_common.h`.
- Keep IOCTL code constants in sync with the kernel driver source.

---

## Key file map

```
src/
  App.tsx              ReactFlow canvas, Apply / Enable Runtime buttons
  state.ts             Zustand store — all state and IPC calls
  types.ts             Shared TS types + nodeTypes registry
  ipc.d.ts             Typed invoke() overloads
  nodes/               React node components
  components/          Menu (virtual device management), ContextMenu

crates/tauri/src/
  lib.rs               All Tauri commands + AppData + VirtualDevice management
  runtime.rs           Audio graph processor (topological order, spin-loop thread)
  driver_client.rs     Windows IOCTL client (cfg(windows) only)
  nodes/               NodeTrait implementations

crates/common/src/lib.rs   #![no_std] shared ABI types

driver/                CableAudio.sys Windows kernel driver (C++)

.vm/
  test.ps1             Pester test runner (VM-based)
  test/*.Tests.ps1     Integration test suites
  exec.ps1             Run arbitrary commands in the test VM
  setup.ps1            Prepare VM for manual testing

scripts/
  build.ps1            Unified build (driver + frontend + Tauri app)
```
