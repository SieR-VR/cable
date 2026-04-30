//! BLE advertisement watcher that listens for AppleCP "Proximity Pairing"
//! packets to surface AirPods battery levels. Lifecycle is controlled from
//! the frontend through `start_bluetooth_battery_watcher` /
//! `stop_bluetooth_battery_watcher` Tauri commands.

#![cfg(windows)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use windows::Devices::Bluetooth::Advertisement::{
  BluetoothLEAdvertisementReceivedEventArgs, BluetoothLEAdvertisementWatcher,
  BluetoothLEScanningMode,
};
use windows::Foundation::TypedEventHandler;
use windows::Storage::Streams::DataReader;

use super::airpods::{parse_apple_continuity, ApplePayload};
use super::win::enumerate_bluetooth_containers;

const APPLE_COMPANY_ID: u16 = 0x004C;
const EVENT_NAME: &str = "bluetooth-battery-update";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BatteryEvent {
  pub container_id: String,
  pub model_id: u16,
  pub left: Option<u8>,
  pub right: Option<u8>,
  #[serde(rename = "case")]
  pub case_: Option<u8>,
  pub charging_left: bool,
  pub charging_right: bool,
  pub charging_case: bool,
}

struct WatcherState {
  watcher: BluetoothLEAdvertisementWatcher,
  received_token: i64,
}

// SAFETY: BluetoothLEAdvertisementWatcher is `Send` per windows-rs binding,
// and we serialise all access through a Mutex.
unsafe impl Send for WatcherState {}

fn state() -> &'static Mutex<Option<WatcherState>> {
  static S: OnceLock<Mutex<Option<WatcherState>>> = OnceLock::new();
  S.get_or_init(|| Mutex::new(None))
}

/// Start the watcher. Idempotent: returns Ok if already running.
pub fn start(app: AppHandle) -> Result<(), String> {
  let mut guard = state().lock().map_err(|e| e.to_string())?;
  if guard.is_some() {
    return Ok(());
  }

  // Build model_id -> container_id map. Apple devices expose their model id
  // in DEVPKEY_Bluetooth_DeviceVID (e.g. 0x2024 for AirPods Pro 2 USB-C).
  let containers = enumerate_bluetooth_containers();
  let mut model_map: HashMap<u16, String> = HashMap::new();
  for c in containers {
    if let Some(vid) = c.vendor_id {
      model_map.entry(vid).or_insert(c.container_id);
    }
  }
  let model_map = Arc::new(model_map);

  // Cache last emitted state per container so we don't spam the frontend.
  let last_emit: Arc<Mutex<HashMap<String, BatteryEvent>>> = Arc::new(Mutex::new(HashMap::new()));

  let watcher = BluetoothLEAdvertisementWatcher::new()
    .map_err(|e| format!("BluetoothLEAdvertisementWatcher::new failed: {}", e))?;
  watcher
    .SetScanningMode(BluetoothLEScanningMode::Active)
    .map_err(|e| format!("SetScanningMode failed: {}", e))?;

  let handler_app = app.clone();
  let handler_map = Arc::clone(&model_map);
  let handler_last = Arc::clone(&last_emit);
  let handler = TypedEventHandler::<
    BluetoothLEAdvertisementWatcher,
    BluetoothLEAdvertisementReceivedEventArgs,
  >::new(move |_sender, args| {
    if let Some(args) = args.as_ref() {
      if let Err(e) = on_received(args, &handler_app, &handler_map, &handler_last) {
        eprintln!("bluetooth watcher: handler error: {}", e);
      }
    }
    Ok(())
  });

  let token = watcher
    .Received(&handler)
    .map_err(|e| format!("Watcher::Received subscription failed: {}", e))?;
  watcher
    .Start()
    .map_err(|e| format!("Watcher::Start failed: {}", e))?;

  *guard = Some(WatcherState {
    watcher,
    received_token: token,
  });
  Ok(())
}

/// Stop the watcher. Idempotent: returns Ok if not running.
pub fn stop() -> Result<(), String> {
  let mut guard = state().lock().map_err(|e| e.to_string())?;
  if let Some(s) = guard.take() {
    let _ = s.watcher.RemoveReceived(s.received_token);
    s.watcher
      .Stop()
      .map_err(|e| format!("Watcher::Stop failed: {}", e))?;
  }
  Ok(())
}

fn on_received(
  args: &BluetoothLEAdvertisementReceivedEventArgs,
  app: &AppHandle,
  model_map: &HashMap<u16, String>,
  last_emit: &Mutex<HashMap<String, BatteryEvent>>,
) -> Result<(), String> {
  let mfg_view = args
    .Advertisement()
    .map_err(|e| e.to_string())?
    .GetManufacturerDataByCompanyId(APPLE_COMPANY_ID)
    .map_err(|e| e.to_string())?;
  let len = mfg_view.Size().map_err(|e| e.to_string())?;
  for i in 0..len {
    let entry = mfg_view.GetAt(i).map_err(|e| e.to_string())?;
    let buffer = entry.Data().map_err(|e| e.to_string())?;
    let data = ibuffer_to_vec(&buffer)?;
    let payload = match parse_apple_continuity(&data) {
      Some(p) => p,
      None => continue,
    };
    let container_id = match model_map.get(&payload.model_id) {
      Some(c) => c.clone(),
      None => continue,
    };
    let event = make_event(container_id, payload);
    let mut last = last_emit.lock().map_err(|e| e.to_string())?;
    if last.get(&event.container_id) == Some(&event) {
      continue;
    }
    last.insert(event.container_id.clone(), event.clone());
    drop(last);
    if let Err(e) = app.emit(EVENT_NAME, &event) {
      eprintln!("bluetooth watcher: emit failed: {}", e);
    }
  }
  Ok(())
}

fn make_event(container_id: String, p: ApplePayload) -> BatteryEvent {
  BatteryEvent {
    container_id,
    model_id: p.model_id,
    left: p.left,
    right: p.right,
    case_: p.case_,
    charging_left: p.charging_left,
    charging_right: p.charging_right,
    charging_case: p.charging_case,
  }
}

fn ibuffer_to_vec(buffer: &windows::Storage::Streams::IBuffer) -> Result<Vec<u8>, String> {
  let reader = DataReader::FromBuffer(buffer).map_err(|e| e.to_string())?;
  let len = reader.UnconsumedBufferLength().map_err(|e| e.to_string())? as usize;
  let mut out = vec![0u8; len];
  reader.ReadBytes(&mut out).map_err(|e| e.to_string())?;
  Ok(out)
}
