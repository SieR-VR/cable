//! Bluetooth identity and (Phase B) battery telemetry for audio devices.
//!
//! Phase A surfaces a `BluetoothInfo` record for any cpal/MM audio endpoint
//! that is backed by a Bluetooth device. The link is established via
//! `PKEY_Device_ContainerId` which is identical between the audio endpoint
//! and the underlying Bluetooth PnP node.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BluetoothInfo {
  /// Container GUID shared between the audio endpoint and the BT PnP nodes,
  /// formatted as `{XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX}`.
  pub container_id: String,
  /// Bluetooth Classic MAC address as `AA:BB:CC:DD:EE:FF`. May be missing
  /// for BLE-only peripherals.
  pub address: Option<String>,
  /// Bluetooth Vendor ID (e.g. `0x004C` for Apple).
  pub vendor_id: Option<u16>,
  /// Bluetooth Product ID.
  pub product_id: Option<u16>,
  /// Top-level container category, e.g. "Audio.Headphone".
  pub category: Option<String>,
  /// True when at least one PnP node in the container uses a Bluetooth enumerator.
  pub is_bluetooth: bool,
}

pub mod airpods;

#[cfg(windows)]
mod watcher;
#[cfg(windows)]
mod win;

#[cfg(windows)]
pub use win::resolve_bluetooth_info;

#[cfg(windows)]
pub fn start_battery_watcher(app: tauri::AppHandle) -> Result<(), String> {
  watcher::start(app)
}
#[cfg(windows)]
pub fn stop_battery_watcher() -> Result<(), String> {
  watcher::stop()
}

#[cfg(not(windows))]
pub fn resolve_bluetooth_info(_audio_device_id: &str) -> Option<BluetoothInfo> {
  None
}
#[cfg(not(windows))]
pub fn start_battery_watcher(_app: tauri::AppHandle) -> Result<(), String> {
  Err("Bluetooth battery watcher is only supported on Windows".into())
}
#[cfg(not(windows))]
pub fn stop_battery_watcher() -> Result<(), String> {
  Ok(())
}
