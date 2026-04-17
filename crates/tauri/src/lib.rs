use std::collections::BTreeMap;
use std::sync::{
  atomic::{AtomicBool, Ordering},
  Arc,
};

use cpal::traits::{DeviceTrait, HostTrait};
use serde::{Deserialize, Serialize};
use tauri::{async_runtime::Mutex, Builder, State};

#[cfg(windows)]
pub(crate) mod driver_client;
pub(crate) mod nodes;
mod runtime;

use nodes::audio_input_device::AudioInputDeviceNode;
use nodes::audio_output_device::AudioOutputDeviceNode;
use nodes::virtual_audio_input::VirtualAudioInputNode;
use nodes::virtual_audio_output::VirtualAudioOutputNode;

/// A virtual audio device managed by the driver, independent of the audio graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VirtualDevice {
  /// Hex-encoded 16-byte device ID from the driver.
  pub id: String,
  /// User-chosen friendly name.
  pub name: String,
  /// "render" or "capture".
  pub device_type: String,
  /// Windows MM endpoint ID string (e.g. `{0.0.0.00000000}.{guid}`).
  /// Cached at creation time for fast IMMDevice lookup during rename.
  /// Internal field — not sent to the frontend.
  #[serde(skip)]
  pub endpoint_id: String,
}

struct AppData {
  runtime: Option<runtime::Runtime>,
  runtime_thread: Option<std::thread::JoinHandle<()>>,
  runtime_running: Option<Arc<AtomicBool>>,
  #[cfg(windows)]
  driver_handle: Option<Arc<driver_client::DriverHandle>>,
  /// Virtual devices created via the menu panel (device_hex_id -> VirtualDevice).
  virtual_devices: BTreeMap<String, VirtualDevice>,
}

fn start_runtime_thread(state: &mut AppData, mut runtime: runtime::Runtime) {
  let running = Arc::new(AtomicBool::new(true));
  let running_clone = running.clone();

  let sleep_duration = runtime.buffer_duration();
  println!("Enabling runtime with sleep duration: {:?}", sleep_duration);

  let handle = std::thread::spawn(move || {
    // Use a spin-loop with Instant for precise audio timing.
    // std::thread::sleep on Windows has ~15.6ms granularity by default,
    // which causes systematic underruns when sleep_duration < 15.6ms.
    let mut next_tick = std::time::Instant::now() + sleep_duration;
    while running_clone.load(Ordering::Relaxed) {
      if let Err(e) = runtime.process() {
        eprintln!("Error processing audio graph: {}", e);
      }

      // Spin-wait until the next tick for sub-millisecond accuracy.
      // Yield to the OS when we're more than 2ms away to reduce CPU usage,
      // then spin for the final stretch.
      loop {
        let now = std::time::Instant::now();
        if now >= next_tick {
          break;
        }
        let remaining = next_tick - now;
        if remaining > std::time::Duration::from_millis(2) {
          std::thread::sleep(std::time::Duration::from_millis(1));
        } else {
          std::hint::spin_loop();
        }
      }
      next_tick += sleep_duration;

      // If we fell behind (e.g. system stall), snap forward to avoid
      // a burst of catch-up iterations.
      let now = std::time::Instant::now();
      if next_tick < now {
        next_tick = now + sleep_duration;
      }
    }

    if let Err(e) = runtime.dispose_nodes() {
      eprintln!("Error disposing nodes: {}", e);
    }
    println!("Runtime thread stopped.");
  });

  state.runtime_running = Some(running);
  state.runtime_thread = Some(handle);
}

fn stop_runtime_thread(state: &mut AppData) -> Result<(), String> {
  if let Some(running) = state.runtime_running.take() {
    running.store(false, Ordering::Relaxed);
  }

  if let Some(handle) = state.runtime_thread.take() {
    handle
      .join()
      .map_err(|_| "Failed to join runtime thread".to_string())?;
  }

  Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AudioDevice {
  id: String,
  readable_name: String,

  frequency: u32,
  channels: u16,
  bits_per_sample: usize,

  descriptions: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AudioGraph {
  nodes: Vec<AudioNode>,
  edges: Vec<AudioEdge>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub(crate) enum AudioNode {
  AudioInputDevice(AudioInputDeviceNode),
  AudioOutputDevice(AudioOutputDeviceNode),
  VirtualAudioInput(VirtualAudioInputNode),
  VirtualAudioOutput(VirtualAudioOutputNode),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AudioEdge {
  id: String,

  from: String,
  to: String,

  frequency: Option<u32>,
  channels: Option<u16>,
  bits_per_sample: Option<usize>,
}

#[tauri::command]
fn get_audio_hosts() -> Vec<String> {
  let available_hosts = cpal::available_hosts();
  println!("Available audio hosts: {:?}", available_hosts);

  available_hosts.iter().map(|h| format!("{:?}", h)).collect()
}

#[tauri::command]
fn get_audio_devices(host: String) -> (Vec<AudioDevice>, Vec<AudioDevice>) {
  let host_id = match cpal::available_hosts()
    .into_iter()
    .find(|h| format!("{:?}", h) == host)
  {
    Some(h) => h,
    None => return (Vec::new(), Vec::new()),
  };

  let host = cpal::host_from_id(host_id).unwrap();
  let input_devices = host
    .input_devices()
    .unwrap()
    .map(|d| {
      let description = d.description().unwrap();
      let interface_type = d.default_input_config().unwrap();

      AudioDevice {
        id: d.id().unwrap().to_string(),
        readable_name: description.name().to_string(),
        descriptions: description.extended().to_vec(),
        frequency: interface_type.sample_rate(),
        channels: interface_type.channels(),
        bits_per_sample: interface_type.sample_format().sample_size() * 8,
      }
    })
    .collect();
  let output_devices = host
    .output_devices()
    .unwrap()
    .map(|d| {
      let description = d.description().unwrap();
      let interface_type = d.default_output_config().unwrap();

      AudioDevice {
        id: d.id().unwrap().to_string(),
        readable_name: description.name().to_string(),
        descriptions: description.extended().to_vec(),
        frequency: interface_type.sample_rate(),
        channels: interface_type.channels(),
        bits_per_sample: interface_type.sample_format().sample_size() * 8,
      }
    })
    .collect();

  println!(
    "Input devices: {:?}, Output devices: {:?}",
    input_devices, output_devices
  );
  (input_devices, output_devices)
}

/// Try to open a handle to the CableAudio kernel driver.
/// Returns true if the driver is available, false otherwise.
#[tauri::command]
async fn connect_driver(state: State<'_, Mutex<AppData>>) -> Result<bool, String> {
  #[cfg(windows)]
  {
    let mut state = state.lock().await;
    match driver_client::DriverHandle::open() {
      Ok(handle) => {
        println!("CableAudio driver connected successfully");
        state.driver_handle = Some(Arc::new(handle));
        Ok(true)
      }
      Err(e) => {
        println!("CableAudio driver not available: {}", e);
        state.driver_handle = None;
        Ok(false)
      }
    }
  }
  #[cfg(not(windows))]
  {
    let _ = state;
    Ok(false)
  }
}

/// Check if the driver is currently connected.
#[tauri::command]
async fn is_driver_connected(state: State<'_, Mutex<AppData>>) -> Result<bool, String> {
  #[cfg(windows)]
  {
    let state = state.lock().await;
    Ok(state.driver_handle.is_some())
  }
  #[cfg(not(windows))]
  {
    let _ = state;
    Ok(false)
  }
}

/// List all currently created virtual devices.
#[tauri::command]
async fn list_virtual_devices(
  state: State<'_, Mutex<AppData>>,
) -> Result<Vec<VirtualDevice>, String> {
  let state = state.lock().await;
  Ok(state.virtual_devices.values().cloned().collect())
}

/// Create a new virtual audio device via the driver.
/// Returns the created VirtualDevice with its driver-assigned ID.
#[tauri::command]
async fn create_virtual_device(
  state: State<'_, Mutex<AppData>>,
  name: String,
  device_type: String,
) -> Result<VirtualDevice, String> {
  #[cfg(windows)]
  {
    // Take a snapshot of existing endpoint IDs *before* creating the device so
    // we can detect the new one by diff (avoids unreliable PnP tree traversal).
    let pre_snapshot = tauri::async_runtime::spawn_blocking(snapshot_endpoint_ids)
      .await
      .unwrap_or_else(|_| std::collections::HashSet::new());

    // Acquire lock only long enough to clone the driver handle and issue the IOCTL.
    // We release the lock before the long COM enumeration so other commands can proceed.
    let hex_id = {
      let app = state.lock().await;
      let driver = app
        .driver_handle
        .as_ref()
        .ok_or_else(|| "Driver not connected".to_string())?
        .clone();
      // Lock is still held during the IOCTL call, but the IOCTL is fast (kernel
      // synchronous); the long async wait is the COM enumeration below.
      drop(app); // release lock before blocking IOCTL

      let dt = match device_type.as_str() {
        "render" => common::DeviceType::Render,
        "capture" => common::DeviceType::Capture,
        _ => return Err(format!("Invalid device type: {}", device_type)),
      };

      let created = driver.create_virtual_device(&name, dt)?;
      let hex_id = hex::encode(created.id);

      println!(
        "Created virtual {} device '{}' -> {}",
        device_type, name, hex_id
      );

      hex_id
    }; // ← mutex is already released (dropped above)

    // Attempt to find the Windows MM endpoint ID for this device by polling for
    // a newly-appeared endpoint (diff against pre_snapshot).
    // We do this on a blocking thread to keep COM calls off the async executor.
    let name_for_creation = name.clone();
    let endpoint_id = tauri::async_runtime::spawn_blocking(move || {
      let ep_id = find_new_endpoint_id(&pre_snapshot, 15, 300)?;
      eprintln!("create_virtual_device: found endpoint_id='{}'", ep_id);
      // If we found the endpoint, immediately stamp the user's chosen name.
      // The driver sets interface-level properties, but PKEY_Device_DeviceDesc
      // on the MM endpoint (pid=2) is what Windows Audio exposes as FriendlyName.
      if !ep_id.is_empty() {
        if let Err(e) = elevated_set_endpoint_device_desc(&ep_id, &name_for_creation) {
          eprintln!(
            "elevated_set_endpoint_device_desc at creation failed: {}",
            e
          );
          // Non-fatal: device works, just shows generic name until next rename.
        }
      }
      Ok(ep_id)
    })
    .await
    .unwrap_or_else(|e: tauri::Error| {
      eprintln!("spawn_blocking error finding endpoint_id: {}", e);
      Ok::<String, String>(String::new())
    })
    .unwrap_or_else(|e: String| {
      eprintln!("find_new_endpoint_id error: {}", e);
      String::new()
    });

    println!("  endpoint_id for {}: '{}'", hex_id, endpoint_id);

    let vd = VirtualDevice {
      id: hex_id.clone(),
      name,
      device_type,
      endpoint_id,
    };
    // Re-acquire the lock to persist the new device entry.
    let mut app = state.lock().await;
    app.virtual_devices.insert(hex_id, vd.clone());
    Ok(vd)
  }
  #[cfg(not(windows))]
  {
    let _ = (state, name, device_type);
    Err("Virtual devices require Windows".to_string())
  }
}

/// Remove an existing virtual audio device via the driver.
#[tauri::command]
async fn remove_virtual_device(
  state: State<'_, Mutex<AppData>>,
  device_id: String,
) -> Result<(), String> {
  #[cfg(windows)]
  {
    let mut app = state.lock().await;

    // Reject removal while the audio runtime is actively using ring buffers.
    if app
      .runtime_running
      .as_ref()
      .is_some_and(|r| r.load(std::sync::atomic::Ordering::Relaxed))
    {
      return Err("Cannot remove a virtual device while the runtime is running".to_string());
    }

    let driver = app
      .driver_handle
      .as_ref()
      .ok_or_else(|| "Driver not connected".to_string())?
      .clone();

    let id_bytes = hex_to_device_id(&device_id)?;
    driver.remove_virtual_device(&id_bytes)?;
    app.virtual_devices.remove(&device_id);

    println!("Removed virtual device {}", device_id);
    Ok(())
  }
  #[cfg(not(windows))]
  {
    let _ = (state, device_id);
    Err("Virtual devices require Windows".to_string())
  }
}

/// Rename a virtual audio device.
///
/// Uses COM IPropertyStore to write PKEY_Device_DeviceDesc (pid=2) on the
/// MM endpoint, which causes Windows to reflect the new name as FriendlyName.
/// Also updates the in-memory AppData entry.
#[tauri::command]
async fn rename_virtual_device(
  state: State<'_, Mutex<AppData>>,
  device_id: String,
  new_name: String,
) -> Result<(), String> {
  #[cfg(windows)]
  {
    // Fetch cached endpoint_id; recover it opportunistically if missing.
    let mut endpoint_id = {
      let app = state.lock().await;
      app
        .virtual_devices
        .get(&device_id)
        .ok_or_else(|| format!("Device {} not found", device_id))?
        .endpoint_id
        .clone()
    };

    if endpoint_id.is_empty() {
      let known_ids = {
        let app = state.lock().await;
        app
          .virtual_devices
          .values()
          .filter_map(|v| {
            if v.id != device_id && !v.endpoint_id.is_empty() {
              Some(v.endpoint_id.clone())
            } else {
              None
            }
          })
          .collect::<std::collections::HashSet<_>>()
      };

      endpoint_id = tauri::async_runtime::spawn_blocking(move || {
        let all = snapshot_endpoint_ids();
        for ep in all {
          if !known_ids.contains(&ep) && endpoint_exists(&ep) {
            return ep;
          }
        }
        String::new()
      })
      .await
      .map_err(|e| format!("spawn_blocking error recovering endpoint_id: {}", e))?;

      if !endpoint_id.is_empty() {
        let mut app = state.lock().await;
        if let Some(vd) = app.virtual_devices.get_mut(&device_id) {
          vd.endpoint_id = endpoint_id.clone();
        }
      }

      if endpoint_id.is_empty() {
        return Err(format!(
          "Device {} has no cached endpoint_id; cannot rename",
          device_id
        ));
      }
    }

    let name_for_com = new_name.clone();
    let ep_id = endpoint_id.clone();
    tauri::async_runtime::spawn_blocking(move || {
      elevated_set_endpoint_device_desc(&ep_id, &name_for_com)
    })
    .await
    .map_err(|e| format!("spawn_blocking error: {}", e))??;

    // Update local state.
    let mut app = state.lock().await;
    if let Some(vd) = app.virtual_devices.get_mut(&device_id) {
      vd.name = new_name;
    }
    Ok(())
  }
  #[cfg(not(windows))]
  {
    let _ = (state, device_id, new_name);
    Err("Virtual devices require Windows".to_string())
  }
}

/// Convert hex string to 16-byte DeviceId.
#[cfg(windows)]
fn hex_to_device_id(hex: &str) -> Result<common::DeviceId, String> {
  let bytes = hex::decode(hex).map_err(|e| format!("Invalid device ID hex: {}", e))?;
  if bytes.len() != 16 {
    return Err(format!("Device ID must be 16 bytes, got {}", bytes.len()));
  }
  let mut id = [0u8; 16];
  id.copy_from_slice(&bytes);
  Ok(id)
}

// ---------------------------------------------------------------------------
// COM helpers for audio endpoint discovery and rename
// ---------------------------------------------------------------------------

/// Collect the IDs of all currently active MM audio endpoints into a HashSet.
///
/// Called on a blocking thread before creating a virtual device so we can
/// identify the new endpoint by set-difference after creation.
#[cfg(windows)]
fn snapshot_endpoint_ids() -> std::collections::HashSet<String> {
  use windows::Win32::Media::Audio::{eAll, IMMDeviceEnumerator, MMDeviceEnumerator, DEVICE_STATE};
  use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
  };

  let mut ids = std::collections::HashSet::new();

  unsafe {
    let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
    if hr.is_err() && hr != windows::Win32::Foundation::S_FALSE {
      eprintln!("snapshot_endpoint_ids: CoInitializeEx failed: {:?}", hr);
      return ids;
    }
    let _guard = CoUninitGuard;

    let enumerator: IMMDeviceEnumerator =
      match CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER) {
        Ok(e) => e,
        Err(e) => {
          eprintln!("snapshot_endpoint_ids: CoCreateInstance failed: {}", e);
          return ids;
        }
      };

    // Enumerate all endpoints (active/disabled/not-present/unplugged).
    let collection = match enumerator.EnumAudioEndpoints(eAll, DEVICE_STATE(0xF)) {
      Ok(c) => c,
      Err(e) => {
        eprintln!("snapshot_endpoint_ids: EnumAudioEndpoints failed: {}", e);
        return ids;
      }
    };

    let count = match collection.GetCount() {
      Ok(n) => n,
      Err(_) => return ids,
    };

    for i in 0..count {
      let device = match collection.Item(i) {
        Ok(d) => d,
        Err(_) => continue,
      };
      let id_pwstr = match device.GetId() {
        Ok(p) => p,
        Err(_) => continue,
      };
      let id_str = id_pwstr.to_string().unwrap_or_default();
      windows::Win32::System::Com::CoTaskMemFree(Some(id_pwstr.as_ptr() as *const _));
      ids.insert(id_str);
    }
  }

  ids
}

/// Poll MM audio endpoints until one appears that is NOT in `pre_snapshot`.
///
/// Returns the new endpoint's ID string, e.g. `{0.0.0.00000000}.{guid}`.
/// Returns an empty string if no new endpoint appears within the retry window.
///
/// This avoids all PnP-tree traversal (CM_Get_Parent, SetupDi) — the new
/// endpoint is simply the one that wasn't there before the IOCTL.
#[cfg(windows)]
fn find_new_endpoint_id(
  pre_snapshot: &std::collections::HashSet<String>,
  max_retries: u32,
  retry_delay_ms: u64,
) -> Result<String, String> {
  use windows::Win32::Media::Audio::{eAll, IMMDeviceEnumerator, MMDeviceEnumerator, DEVICE_STATE};
  use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
  };

  unsafe {
    let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
    if hr.is_err() && hr != windows::Win32::Foundation::S_FALSE {
      return Err(format!("CoInitializeEx failed: {:?}", hr));
    }
    let _coinit_guard = CoUninitGuard;

    let enumerator: IMMDeviceEnumerator =
      CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER)
        .map_err(|e| format!("CoCreateInstance(MMDeviceEnumerator) failed: {}", e))?;

    for attempt in 0..=max_retries {
      if attempt > 0 {
        std::thread::sleep(std::time::Duration::from_millis(retry_delay_ms));
        eprintln!("find_new_endpoint_id: retry {}/{}", attempt, max_retries);
      }

      let collection = match enumerator.EnumAudioEndpoints(eAll, DEVICE_STATE(0xF)) {
        Ok(c) => c,
        Err(e) => return Err(format!("EnumAudioEndpoints failed: {}", e)),
      };

      let count = match collection.GetCount() {
        Ok(n) => n,
        Err(e) => return Err(format!("GetCount failed: {}", e)),
      };

      for i in 0..count {
        let device = match collection.Item(i) {
          Ok(d) => d,
          Err(_) => continue,
        };
        let id_pwstr = match device.GetId() {
          Ok(p) => p,
          Err(_) => continue,
        };
        let id_str = id_pwstr.to_string().unwrap_or_default();
        windows::Win32::System::Com::CoTaskMemFree(Some(id_pwstr.as_ptr() as *const _));

        if !pre_snapshot.contains(&id_str) {
          eprintln!(
            "find_new_endpoint_id: found new endpoint '{}' on attempt {}",
            id_str, attempt
          );
          return Ok(id_str);
        }
      }
    }

    eprintln!(
      "find_new_endpoint_id: no new endpoint appeared after {} retries",
      max_retries
    );
    Ok(String::new())
  }
}

#[cfg(windows)]
fn endpoint_exists(endpoint_id: &str) -> bool {
  use windows::Win32::Media::Audio::{IMMDeviceEnumerator, MMDeviceEnumerator};
  use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
  };

  unsafe {
    let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
    if hr.is_err() && hr != windows::Win32::Foundation::S_FALSE {
      return false;
    }
    let _coinit_guard = CoUninitGuard;

    let enumerator: IMMDeviceEnumerator =
      match CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER) {
        Ok(e) => e,
        Err(_) => return false,
      };

    let ep_wide: Vec<u16> = endpoint_id
      .encode_utf16()
      .chain(std::iter::once(0))
      .collect();

    enumerator
      .GetDevice(windows::core::PCWSTR(ep_wide.as_ptr()))
      .is_ok()
  }
}

/// Write PKEY_Device_DeviceDesc (pid=2) on the MM endpoint identified by
/// `endpoint_id`. This changes the first component of the FriendlyName that
/// Windows Audio shows in the Sound control panel and GetFriendlyName().
#[cfg(windows)]
fn set_endpoint_device_desc(endpoint_id: &str, new_name: &str) -> Result<(), String> {
  use windows::Win32::Foundation::PROPERTYKEY;
  use windows::Win32::Media::Audio::{IMMDeviceEnumerator, MMDeviceEnumerator};
  use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
  use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemAlloc, CoTaskMemFree, CLSCTX_INPROC_SERVER,
    COINIT_MULTITHREADED, STGM,
  };
  use windows::Win32::System::Variant::VT_LPWSTR;
  use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;

  // PKEY_Device_DeviceDesc = {A45C254E-DF1C-4EFD-8020-67D146A850E0}, pid=2
  const PKEY_DEVICE_DESC_FMTID: windows::core::GUID = windows::core::GUID::from_values(
    0xA45C254E,
    0xDF1C,
    0x4EFD,
    [0x80, 0x20, 0x67, 0xD1, 0x46, 0xA8, 0x50, 0xE0],
  );

  let ep_wide: Vec<u16> = endpoint_id
    .encode_utf16()
    .chain(std::iter::once(0))
    .collect();
  let name_wide: Vec<u16> = new_name.encode_utf16().chain(std::iter::once(0)).collect();

  unsafe {
    let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
    if hr.is_err() && hr != windows::Win32::Foundation::S_FALSE {
      return Err(format!("CoInitializeEx failed: {:?}", hr));
    }
    let _coinit_guard = CoUninitGuard;

    let enumerator: IMMDeviceEnumerator =
      CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER)
        .map_err(|e| format!("CoCreateInstance(MMDeviceEnumerator) failed: {}", e))?;

    let device = enumerator
      .GetDevice(windows::core::PCWSTR(ep_wide.as_ptr()))
      .map_err(|e| format!("GetDevice('{}') failed: {}", endpoint_id, e))?;

    // STGM_READWRITE = 2
    let props: IPropertyStore = device
      .OpenPropertyStore(STGM(2))
      .map_err(|e| format!("OpenPropertyStore(READWRITE) failed: {}", e))?;

    // Build a VT_LPWSTR PROPVARIANT for the new name.
    // PROPVARIANT layout: vt (u16 at offset 0) + padding (6 bytes) + union (8 bytes).
    // We allocate a CoTaskMem buffer for the string and store the pointer at offset 8.
    let mut pv = PROPVARIANT::default();
    let byte_len = name_wide.len() * 2; // includes null terminator
    let buf = CoTaskMemAlloc(byte_len) as *mut u16;
    if buf.is_null() {
      return Err("CoTaskMemAlloc failed".to_string());
    }
    std::ptr::copy_nonoverlapping(name_wide.as_ptr(), buf, name_wide.len());

    let pv_ptr = &mut pv as *mut PROPVARIANT as *mut u8;
    *(pv_ptr as *mut u16) = VT_LPWSTR.0 as u16; // vt at offset 0
    *(pv_ptr.add(8) as *mut *mut u16) = buf; // pwszVal at offset 8

    let key = PROPERTYKEY {
      fmtid: PKEY_DEVICE_DESC_FMTID,
      pid: 2,
    };

    let set_result = props.SetValue(&key, &pv);

    // Always free the buffer we allocated.
    CoTaskMemFree(Some(buf as *const _));
    // Zero the pointer in pv so it isn't double-freed by accident.
    *(pv_ptr.add(8) as *mut *mut u16) = std::ptr::null_mut();

    set_result.map_err(|e| format!("IPropertyStore::SetValue(DeviceDesc) failed: {}", e))?;

    props
      .Commit()
      .map_err(|e| format!("IPropertyStore::Commit failed: {}", e))?;

    println!(
      "IPropertyStore::SetValue(DeviceDesc) OK for endpoint '{}'",
      endpoint_id
    );
    Ok(())
  }
}

/// RAII guard that calls CoUninitialize when dropped.
#[cfg(windows)]
struct CoUninitGuard;

#[cfg(windows)]
impl Drop for CoUninitGuard {
  fn drop(&mut self) {
    unsafe {
      windows::Win32::System::Com::CoUninitialize();
    }
  }
}

// ---------------------------------------------------------------------------
// Elevated rename helpers
// ---------------------------------------------------------------------------

/// Public entry point called from `main.rs` when the process is re-launched
/// with `--rename-endpoint <endpoint_id> <name>`.
///
/// This runs in an elevated (admin) context and writes PKEY_Device_DeviceDesc
/// via IPropertyStore, then returns.
#[cfg(windows)]
pub fn rename_endpoint_elevated(endpoint_id: &str, new_name: &str) -> Result<(), String> {
  set_endpoint_device_desc(endpoint_id, new_name)
}

/// Re-launch the current executable with verb "runas" so that Windows shows a
/// UAC prompt and the child process runs elevated.  The child is invoked with
/// `--rename-endpoint <endpoint_id> <new_name>` arguments.
///
/// This function blocks until the elevated child exits and returns an error if
/// the user cancels the UAC prompt or the child exits with a non-zero code.
#[cfg(windows)]
fn elevated_set_endpoint_device_desc(endpoint_id: &str, new_name: &str) -> Result<(), String> {
  use windows::core::PCWSTR;
  use windows::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
  use windows::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject, INFINITE};
  use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};

  // Build the argument string: --rename-endpoint <endpoint_id> <new_name>
  // We quote the name component to preserve spaces.
  // The endpoint_id is a Windows audio endpoint path — it can contain braces
  // and dots but not spaces, so no quoting needed.
  let args_str = format!(
    "--rename-endpoint {} {}",
    endpoint_id,
    shell_quote(new_name)
  );

  // Get the path of the current executable.
  let exe_path = std::env::current_exe().map_err(|e| format!("current_exe() failed: {}", e))?;
  let exe_str = exe_path
    .to_str()
    .ok_or_else(|| "exe path is not valid UTF-8".to_string())?;

  // Encode as wide strings with null terminator.
  let verb: Vec<u16> = "runas\0".encode_utf16().collect();
  let exe_wide: Vec<u16> = exe_str.encode_utf16().chain(std::iter::once(0)).collect();
  let args_wide: Vec<u16> = args_str.encode_utf16().chain(std::iter::once(0)).collect();

  unsafe {
    let mut sei = SHELLEXECUTEINFOW {
      cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
      fMask: SEE_MASK_NOCLOSEPROCESS,
      hwnd: windows::Win32::Foundation::HWND(std::ptr::null_mut()),
      lpVerb: PCWSTR(verb.as_ptr()),
      lpFile: PCWSTR(exe_wide.as_ptr()),
      lpParameters: PCWSTR(args_wide.as_ptr()),
      lpDirectory: PCWSTR(std::ptr::null()),
      nShow: 0, // SW_HIDE
      ..Default::default()
    };

    ShellExecuteExW(&mut sei).map_err(|e| {
      format!(
        "ShellExecuteExW(runas) failed: {} — user may have cancelled UAC",
        e
      )
    })?;

    // hProcess is only valid when SEE_MASK_NOCLOSEPROCESS is set and the
    // operation succeeded.
    let hproc = sei.hProcess;
    if hproc.is_invalid() {
      return Err("ShellExecuteExW returned invalid process handle".to_string());
    }

    // Wait for the elevated child to complete.
    let wait_result = WaitForSingleObject(hproc, INFINITE);
    if wait_result != WAIT_OBJECT_0 {
      let _ = CloseHandle(hproc);
      return Err(format!(
        "WaitForSingleObject failed (result={:?})",
        wait_result
      ));
    }

    let mut exit_code: u32 = 0;
    let _ = GetExitCodeProcess(hproc, &mut exit_code);
    let _ = CloseHandle(hproc);

    if exit_code != 0 {
      return Err(format!(
        "Elevated rename process exited with code {} (rename failed)",
        exit_code
      ));
    }

    Ok(())
  }
}

/// Minimally quote a string for use as a single shell token:
/// wraps in double-quotes if the string contains spaces, escaping embedded
/// double-quotes with backslash.
#[cfg(windows)]
fn shell_quote(s: &str) -> String {
  if s.contains(' ') || s.contains('"') {
    let escaped = s.replace('"', "\\\"");
    format!("\"{}\"", escaped)
  } else {
    s.to_string()
  }
}

#[tauri::command]
async fn setup_runtime(
  state: State<'_, Mutex<AppData>>,
  graph: AudioGraph,
  host: String,
  buffer_size: u32,
) -> Result<(), String> {
  println!("Setting up audio graph: {:?}", graph);
  let host_id = match cpal::available_hosts()
    .into_iter()
    .find(|h| format!("{:?}", h) == host)
  {
    Some(h) => h,
    None => return Err(format!("Audio host not found: {}", host)),
  };
  let audio_host = cpal::host_from_id(host_id).unwrap();

  let sample_rate = graph
    .edges
    .first()
    .and_then(|e| e.frequency)
    .unwrap_or(48000);

  println!(
    "Creating runtime with buffer size: {}, sample_rate: {}, host: {:?}",
    buffer_size, sample_rate, host_id
  );

  let mut app_state = state.lock().await;

  let was_running = app_state.runtime_running.is_some();
  if was_running {
    stop_runtime_thread(&mut app_state)?;
  }

  #[cfg(windows)]
  let driver_handle = app_state.driver_handle.clone();
  #[cfg(not(windows))]
  let driver_handle: Option<()> = None;

  drop(app_state);

  let mut runtime = runtime::Runtime::new(
    buffer_size,
    sample_rate,
    graph.nodes,
    graph.edges,
    audio_host,
    driver_handle,
  );

  runtime.init_nodes()?;

  let mut state = state.lock().await;
  state.runtime = Some(runtime);

  // Always start (or restart) runtime after applying graph so users
  // immediately hear the route without requiring a separate enable step.
  if let Some(runtime_to_start) = state.runtime.take() {
    start_runtime_thread(&mut state, runtime_to_start);
  }

  Ok(())
}

#[tauri::command]
async fn enable_runtime(state: State<'_, Mutex<AppData>>) -> Result<(), String> {
  let mut state = state.lock().await;
  if let Some(runtime) = state.runtime.take() {
    start_runtime_thread(&mut state, runtime);
  }
  Ok(())
}

/// Open the WebView developer tools (browser devtools).
/// Works in debug builds; in release builds requires the `devtools` Cargo feature.
#[tauri::command]
fn open_devtools(window: tauri::WebviewWindow) {
  #[cfg(debug_assertions)]
  window.open_devtools();
  #[cfg(not(debug_assertions))]
  let _ = window;
}

#[tauri::command]
async fn disable_runtime(state: State<'_, Mutex<AppData>>) -> Result<(), String> {
  let mut state = state.lock().await;
  stop_runtime_thread(&mut state)?;

  println!("Runtime disabled.");
  Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  Builder::default()
    .plugin(tauri_plugin_opener::init())
    .manage(Mutex::new(AppData {
      runtime: None,
      runtime_thread: None,
      runtime_running: None,
      #[cfg(windows)]
      driver_handle: None,
      virtual_devices: BTreeMap::new(),
    }))
    .invoke_handler(tauri::generate_handler![
      get_audio_hosts,
      get_audio_devices,
      connect_driver,
      is_driver_connected,
      list_virtual_devices,
      create_virtual_device,
      remove_virtual_device,
      rename_virtual_device,
      setup_runtime,
      enable_runtime,
      disable_runtime,
      open_devtools,
    ])
    .run(tauri::generate_context!())
    .unwrap();
}
