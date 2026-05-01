//! Tauri commands for the CableAudio kernel-driver interface and virtual
//! device lifecycle.

use std::sync::Arc;
use tauri::{async_runtime::Mutex, State};

use super::client;
use crate::driver::endpoint::{
  elevated_set_endpoint_device_desc, elevated_set_endpoint_device_format, endpoint_exists,
  find_new_endpoint_id, snapshot_endpoint_ids,
};
use crate::{AppData, VirtualDevice};
/// Try to open a handle to the CableAudio kernel driver.
/// Returns true if the driver is available, false otherwise.
#[tauri::command]
pub async fn connect_driver(state: State<'_, Mutex<AppData>>) -> Result<bool, String> {
  #[cfg(windows)]
  {
    let mut state = state.lock().await;
    match client::DriverHandle::open() {
      Ok(handle) => {
        println!("CableAudio driver connected successfully");
        let arc = Arc::new(handle);
        state.driver_handle = Some(arc.clone());
        if let Ok(mut rt) = state.runtime.lock() {
          rt.driver_handle = Some(arc);
        }
        Ok(true)
      }
      Err(e) => {
        println!("CableAudio driver not available: {}", e);
        state.driver_handle = None;
        if let Ok(mut rt) = state.runtime.lock() {
          rt.driver_handle = None;
        }
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
pub async fn is_driver_connected(state: State<'_, Mutex<AppData>>) -> Result<bool, String> {
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
pub async fn list_virtual_devices(
  state: State<'_, Mutex<AppData>>,
) -> Result<Vec<VirtualDevice>, String> {
  let state = state.lock().await;
  Ok(state.virtual_devices.values().cloned().collect())
}

/// Create a new virtual audio device via the driver.
/// Returns the created VirtualDevice with its driver-assigned ID.
#[tauri::command]
pub async fn create_virtual_device(
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
        "render" => crate::driver::types::DeviceType::Render,
        "capture" => crate::driver::types::DeviceType::Capture,
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
      channels: 2,
      sample_rate: 48000,
      bits_per_sample: 32,
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
pub async fn remove_virtual_device(
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
pub async fn rename_virtual_device(
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
pub(crate) fn hex_to_device_id(hex: &str) -> Result<crate::driver::types::DeviceId, String> {
  let bytes = hex::decode(hex).map_err(|e| format!("Invalid device ID hex: {}", e))?;
  if bytes.len() != 16 {
    return Err(format!("Device ID must be 16 bytes, got {}", bytes.len()));
  }
  let mut id = [0u8; 16];
  id.copy_from_slice(&bytes);
  Ok(id)
}

/// Restore a list of virtual devices into AppData from persisted frontend state.
///
/// Called on app startup after `connect_driver` succeeds to repopulate the
/// in-memory device map from the frontend's persisted `settings.json`. Any
/// device already present in AppData (by ID) is left untouched.
#[tauri::command]
pub async fn restore_virtual_devices(
  state: State<'_, Mutex<AppData>>,
  devices: Vec<VirtualDevice>,
) -> Result<(), String> {
  let mut app = state.lock().await;
  for device in devices {
    let id = device.id.clone();
    app.virtual_devices.entry(id).or_insert(device);
  }
  Ok(())
}

/// Update the format preset for a virtual device.
///
/// Stores the preferred audio format (channels, sample rate, bits per sample)
/// alongside the device entry so the graph validation engine can check that
/// connected audio edges match the device's expected format.
///
/// If the device has a known Windows MM endpoint ID the format change is also
/// applied to the endpoint's `PKEY_AudioEngine_DeviceFormat` property via an
/// elevated sub-process, so Windows Audio Engine uses the new format the next
/// time the device is opened by a client application.
#[tauri::command]
pub async fn set_virtual_device_format(
  state: State<'_, Mutex<AppData>>,
  device_id: String,
  channels: u32,
  sample_rate: u32,
  bits_per_sample: u32,
) -> Result<(), String> {
  // Validate parameters early.
  if channels == 0 || channels > 8 {
    return Err(format!("Unsupported channel count: {}", channels));
  }
  if bits_per_sample != 16 && bits_per_sample != 24 && bits_per_sample != 32 {
    return Err(format!(
      "Unsupported bits_per_sample: {}. Must be 16, 24, or 32.",
      bits_per_sample
    ));
  }

  // Retrieve the endpoint_id before releasing the lock, then update metadata.
  let endpoint_id = {
    let mut app = state.lock().await;
    let device = app
      .virtual_devices
      .get_mut(&device_id)
      .ok_or_else(|| format!("Device {} not found", device_id))?;
    device.channels = channels;
    device.sample_rate = sample_rate;
    device.bits_per_sample = bits_per_sample;
    device.endpoint_id.clone()
  };

  // Attempt to apply the format to the Windows MM endpoint via an elevated
  // subprocess (requires UAC consent). This is a best-effort operation:
  // if it fails (e.g. no endpoint yet, UAC cancelled) the metadata update
  // above still takes effect for graph validation.
  #[cfg(windows)]
  if !endpoint_id.is_empty() {
    let ep = endpoint_id.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
      elevated_set_endpoint_device_format(&ep, sample_rate, channels as u16, bits_per_sample as u16)
    })
    .await;

    match result {
      Ok(Ok(())) => {
        println!(
          "set_virtual_device_format: applied {} Hz / {} ch / {}-bit to endpoint '{}'",
          sample_rate, channels, bits_per_sample, endpoint_id
        );
      }
      Ok(Err(e)) => {
        eprintln!(
          "set_virtual_device_format: elevated format change failed (non-fatal): {}",
          e
        );
        return Err(format!("Format applied locally but endpoint update failed: {}", e));
      }
      Err(e) => {
        eprintln!("set_virtual_device_format: spawn_blocking error: {}", e);
      }
    }
  }

  Ok(())
}
