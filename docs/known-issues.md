# Known Issues and Limitations

This document tracks known limitations, incomplete features, and potential
vulnerabilities identified in the Cable codebase.

---

## VST3 Node (`crates/tauri/src/nodes/vst_node.rs`)

### Single output bus is always used
**Location:** `process_with_plugin`, line `let bus_idx: usize = 0; // use single output bus`

**Description:** The VST3 processing path currently uses only output bus index 0
regardless of `num_outputs`. Plugins with multiple output buses will only have their
first bus routed to the graph output. The handle identifiers `vst-out-N` suggest
multi-bus routing was intended but is not yet implemented.

**Impact:** Multi-bus output VST3 plugins (e.g., stems-capable instruments) silently
drop audio from bus indices 1+.

**Affected area:** `VstNode::process_with_plugin`

---

### VST3 plugin I/O channel counts are not read during scanning
**Location:** `scan_single_dll` — `let num_inputs: u16 = 1; let num_outputs: u16 = 1;`

**Description:** The scanner keeps hard-coded defaults (1 in / 1 out) because
reading accurate channel counts would require creating an `IComponent` instance per
DLL, which is too expensive for a bulk scan.

**Impact:** `VstPluginInfo.num_inputs` / `num_outputs` are always `1` after a scan.
Accurate values are only available after `apply_graph` runs `init()` on the node.
The frontend reads these values to render handles, so the initial handle count after
scanning will be incorrect for most plugins.

**Mitigation needed:** Perform a lightweight I/O query inside `scan_single_dll` using
`IComponent::getBusInfo`, or load the accurate count lazily in `NodeTrait::create`.

---

### `unsafe` blocks throughout VST3 COM dispatch have no null-pointer guards after the vtable call
**Location:** `vst_node.rs`, `vst3_com.rs`

**Description:** Raw COM pointers (`*mut IComponent`, `*mut IAudioProcessor`, etc.)
are dereferenced after querying the vtable, but intermediate results (e.g., the
result of `create_instance`) are checked only once. If a misbehaving plugin returns
a non-null pointer that points to invalid memory the application will crash with an
access violation rather than returning a clean Rust error.

**Impact:** Crash (SIGSEGV / access violation) when loading a corrupt or
incompatible VST3 plugin.

---

## VST3 Editor (`crates/tauri/src/lib.rs`)

### `open_vst_editor` requires `apply_graph` to be called first on some code paths
**Location:** `open_vst_editor` command, `vst_ctrl_cids` lookup

**Description:** If a VST node was never passed through `create_node` (i.e., the
user restored a saved graph and opened the editor without pressing Apply),
`ctrl_cid` will not be present in `vst_ctrl_cids` and the command returns an error
message.

**Error message:** `"ctrl_cid not found. Please press Apply first."`

**Mitigation:** On graph load, proactively call `create_node` for every VST node to
pre-populate `vst_ctrl_cids`.

---

### `EditorWindowState` is leaked on abnormal WndProc exit
**Location:** `run_vst_editor_thread` — `SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize)`

**Description:** The `EditorWindowState` box is recovered and dropped inside
`WM_DESTROY`. If the message loop terminates without delivering `WM_DESTROY` (e.g.,
due to a plugin crash during `attached()`) the `Box` will be leaked, holding a live
`libloading::Library` and COM pointers.

**Impact:** DLL cannot be unloaded for the lifetime of the process; memory leak.

---

## Audio Device Nodes (`crates/tauri/src/nodes/audio_output_device.rs`, `audio_input_device.rs`)

### `unwrap()` in device enumeration panics if cpal returns an error
**Location:** `get_audio_devices` command in `lib.rs` (lines 273–299)

**Description:** Several calls in the device-enumeration block use `.unwrap()` on
`DeviceTrait` methods (`description()`, `id()`, `default_input_config()`). If a
device disappears between enumeration start and the property read (hot-unplug), the
host process will panic.

**Impact:** Application crash on audio device disconnect during initialization.

**Fix:** Replace `.unwrap()` with `.ok()` / `if let` to skip inaccessible devices
gracefully.

---

## AppAudioCapture (`crates/tauri/src/nodes/app_audio_capture.rs`)

### Non-Windows platforms always return an error
**Location:** `NodeTrait::init` — `#[cfg(not(windows))]` branch returns
`Err("AppAudioCapture requires Windows".to_string())`

**Description:** The node compiles on all platforms but is not functional outside
Windows. The error is propagated to the user as a runtime failure when `apply_graph`
is called.

**Impact:** Non-Windows users adding an `AppAudioCapture` node will see an obscure
error on Apply. The UI should hide or disable this node type on non-Windows targets.

---

### Process loopback uses a 5-second hard timeout
**Location:** `wasapi_process_loopback_inner` — `let deadline = ... + Duration::from_secs(5)`

**Description:** `ActivateAudioInterfaceAsync` is waited on with a 5-second
condvar timeout. If the Windows audio stack is under heavy load the timeout may
fire prematurely, returning a failure even though the activation would have
succeeded shortly after.

**Impact:** Intermittent `AppAudioCapture` initialization failures on slow or
heavily loaded systems.

---

## Ring Buffer / Audio Graph (`crates/tauri/src/nodes/audio_output_device.rs`)

### Overflow samples are silently dropped
**Location:** `process` — `if pushed < buf.samples.len() { " [OVERFLOW: samples dropped]" }`

**Description:** When the ring buffer from the audio graph to the cpal output
stream is full, excess samples are discarded. This is logged every 200 ticks but
produces an audible glitch (dropout).

**Impact:** Audible dropouts under high CPU load or when the buffer size is
mismatched between the graph tick rate and the cpal callback rate.

**Mitigation:** Tune the ring-buffer headroom multiplier (`* 16`) or add an
adaptive back-pressure mechanism.

---

## Driver Client (`crates/tauri/src/driver_client.rs`)

### Windows-only; no fallback for virtual devices on other platforms
**Location:** `#[cfg(windows)]` gating throughout `driver_client.rs` and related
Tauri commands

**Description:** Virtual device creation, removal, and rename all require the
CableAudio.sys kernel driver and only compile/run on Windows. On non-Windows
targets the commands are stubbed to return errors or are compiled out.

**Impact:** The driver-dependent features (virtual audio devices) are unavailable
on macOS/Linux. The UI currently does not communicate this limitation clearly.

---

## Frontend (`src/nodes/VstNode.tsx`)

### VST3 plugin handle counts display incorrect values after scan
**Description:** Because `scan_vst3_plugins` returns `num_inputs: 1, num_outputs: 1`
for all plugins (see VST3 scan limitation above), the initial handle layout in the
VstNode component always shows 1 input and 1 output. The correct counts are only
reflected after `apply_graph` updates the node data.

**Impact:** Users may wire up incorrect connections before pressing Apply.

---

## General

### Extensive use of `unwrap()` on `Mutex::lock()`
**Locations:** Throughout `lib.rs`, `nodes/vst3_com.rs`, `nodes/app_audio_capture.rs`

**Description:** Shared state is protected by `std::sync::Mutex`. If any thread
panics while holding the lock, subsequent `lock().unwrap()` calls from other threads
will panic on the poisoned mutex, bringing down the entire Tauri process.

**Impact:** A panic in the audio processing thread or the VST editor thread can
cascade into a full application crash.

**Fix:** Replace `lock().unwrap()` with `lock().unwrap_or_else(|p| p.into_inner())`
where recovery is possible, or restructure error handling to avoid propagating
panics across thread boundaries.
