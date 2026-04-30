use std::collections::BTreeMap;
use std::sync::{atomic::AtomicBool, Arc, Mutex as StdMutex};

use cpal::traits::{DeviceTrait, HostTrait};
use serde::{Deserialize, Serialize};
use tauri::{
  async_runtime::Mutex,
  menu::{Menu, MenuItem},
  tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
  Builder, Manager, State, WindowEvent,
};
use tauri_plugin_store::StoreExt;

pub(crate) mod bluetooth;
pub(crate) mod driver;
pub(crate) mod nodes;
mod runtime;
pub(crate) mod vst3_common;

#[cfg(windows)]
pub use driver::endpoint::rename_endpoint_elevated;

/// A virtual audio device managed by the driver, independent of the audio graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VirtualDevice {
  pub id: String,
  pub name: String,
  pub device_type: String,
  #[serde(skip)]
  pub endpoint_id: String,
}

pub(crate) struct AppData {
  /// Canonical audio graph state. Always present from app startup; shared
  /// with the audio thread via Arc clone when the runtime is enabled.
  pub runtime: Arc<StdMutex<runtime::Runtime>>,
  pub runtime_thread: Option<std::thread::JoinHandle<()>>,
  pub runtime_running: Option<Arc<AtomicBool>>,
  #[cfg(windows)]
  pub driver_handle: Option<Arc<crate::driver::client::DriverHandle>>,
  pub virtual_devices: BTreeMap<String, VirtualDevice>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WindowInfo {
  pub process_id: u32,
  pub title: String,
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

#[tauri::command]
fn get_window_list() -> Vec<WindowInfo> {
  #[cfg(windows)]
  {
    use windows::core::BOOL;
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::EnumWindows;

    let mut result: Vec<WindowInfo> = Vec::new();

    unsafe extern "system" fn enum_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
      use windows::core::BOOL;
      use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
      };

      let list = &mut *(lparam.0 as *mut Vec<WindowInfo>);

      if !IsWindowVisible(hwnd).as_bool() {
        return BOOL(1);
      }

      let len = GetWindowTextLengthW(hwnd);
      if len <= 0 {
        return BOOL(1);
      }

      let mut buf = vec![0u16; (len + 1) as usize];
      let written = GetWindowTextW(hwnd, &mut buf);
      if written <= 0 {
        return BOOL(1);
      }

      let title = String::from_utf16_lossy(&buf[..written as usize]);

      let mut process_id: u32 = 0;
      GetWindowThreadProcessId(hwnd, Some(&mut process_id));

      list.push(WindowInfo { process_id, title });

      BOOL(1)
    }

    unsafe {
      let ptr = &mut result as *mut Vec<WindowInfo>;
      let _ = EnumWindows(Some(enum_callback), LPARAM(ptr as isize));
    }

    result
  }
  #[cfg(not(windows))]
  {
    vec![]
  }
}

#[tauri::command]
fn get_audio_hosts() -> Vec<String> {
  let available_hosts = cpal::available_hosts();
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

  (input_devices, output_devices)
}

#[tauri::command]
fn get_audio_device_bluetooth(device_id: String) -> Option<bluetooth::BluetoothInfo> {
  bluetooth::resolve_bluetooth_info(&device_id)
}

#[tauri::command]
fn start_bluetooth_battery_watcher(app: tauri::AppHandle) -> Result<(), String> {
  bluetooth::start_battery_watcher(app)
}

#[tauri::command]
fn stop_bluetooth_battery_watcher() -> Result<(), String> {
  bluetooth::stop_battery_watcher()
}

#[tauri::command]
fn open_devtools(window: tauri::WebviewWindow) {
  #[cfg(debug_assertions)]
  window.open_devtools();
  #[cfg(not(debug_assertions))]
  let _ = window;
}

#[tauri::command]
async fn save_graph(content: String) -> Result<bool, String> {
  let handle = rfd::AsyncFileDialog::new()
    .set_file_name("cable-graph.json")
    .add_filter("Cable Graph", &["json"])
    .save_file()
    .await;

  match handle {
    Some(file) => {
      std::fs::write(file.path(), content.as_bytes()).map_err(|e| e.to_string())?;
      Ok(true)
    }
    None => Ok(false),
  }
}

#[tauri::command]
async fn read_text_file(path: String) -> Result<String, String> {
  std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

/// Dispatches plugin-level (no instance) commands.
///
/// For each plugin type, forwards to the plugin's `plugin_command` handler.
/// Used for operations that exist outside any node instance (e.g. scanning
/// the system for available VST3 plugins).
#[tauri::command]
async fn plugin_command(
  plugin_type: String,
  data: serde_json::Value,
) -> Result<serde_json::Value, String> {
  match plugin_type.as_str() {
    "vst" => nodes::vst::plugin_command(data),
    other => Err(format!("plugin_command: unknown plugin type '{}'", other)),
  }
}

/// Per-instance node command dispatch. Looks up the node in the runtime
/// graph and forwards to `NodeTrait::command`.
#[tauri::command]
async fn node_command(
  state: State<'_, Mutex<AppData>>,
  node_id: String,
  data: serde_json::Value,
) -> Result<serde_json::Value, String> {
  let runtime_arc = {
    let app = state.lock().await;
    app.runtime.clone()
  };
  let mut rt = runtime_arc
    .lock()
    .map_err(|e| format!("runtime lock poisoned: {}", e))?;
  let node = rt
    .nodes
    .iter_mut()
    .find(|n| n.id() == node_id)
    .ok_or_else(|| format!("node_command: node '{node_id}' not found"))?;
  node.command(data)
}

/// Filename of the persisted settings store (relative to app data dir).
const SETTINGS_STORE_FILE: &str = "settings.json";
/// Settings key controlling whether closing the window minimizes the app
/// to the system tray instead of exiting. Defaults to `false`.
const KEY_MINIMIZE_TO_TRAY: &str = "minimizeToTrayEnabled";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  Builder::default()
    .plugin(tauri_plugin_store::Builder::default().build())
    .plugin(tauri_plugin_opener::init())
    .manage(Mutex::new(AppData {
      runtime: Arc::new(StdMutex::new(runtime::Runtime::new_default())),
      runtime_thread: None,
      runtime_running: None,
      #[cfg(windows)]
      driver_handle: None,
      virtual_devices: BTreeMap::new(),
    }))
    .setup(|app| {
      // System tray: lets the user restore the window after it has been
      // hidden by the "minimize to tray" setting, or quit the app entirely.
      let show_item = MenuItem::with_id(app, "show", "Show Cable", true, None::<&str>)?;
      let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
      let tray_menu = Menu::with_items(app, &[&show_item, &quit_item])?;

      let _tray = TrayIconBuilder::with_id("main")
        .icon(
          app
            .default_window_icon()
            .cloned()
            .ok_or("no default window icon")?,
        )
        .tooltip("Cable")
        .menu(&tray_menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
          "show" => show_main_window(app),
          "quit" => app.exit(0),
          _ => {}
        })
        .on_tray_icon_event(|tray, event| {
          if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
          } = event
          {
            show_main_window(tray.app_handle());
          }
        })
        .build(app)?;

      Ok(())
    })
    .on_window_event(|window, event| {
      if let WindowEvent::CloseRequested { api, .. } = event {
        if window.label() != "main" {
          return;
        }
        let app = window.app_handle();
        let minimize_to_tray = app
          .store(SETTINGS_STORE_FILE)
          .ok()
          .and_then(|store| store.get(KEY_MINIMIZE_TO_TRAY))
          .and_then(|v| v.as_bool())
          .unwrap_or(false);
        if minimize_to_tray {
          api.prevent_close();
          let _ = window.hide();
        }
      }
    })
    .invoke_handler(tauri::generate_handler![
      get_window_list,
      get_audio_hosts,
      get_audio_devices,
      get_audio_device_bluetooth,
      start_bluetooth_battery_watcher,
      stop_bluetooth_battery_watcher,
      driver::commands::connect_driver,
      driver::commands::is_driver_connected,
      driver::commands::list_virtual_devices,
      driver::commands::create_virtual_device,
      driver::commands::remove_virtual_device,
      driver::commands::rename_virtual_device,
      runtime::add_node,
      runtime::remove_node,
      runtime::update_node,
      runtime::add_edge,
      runtime::remove_edge,
      runtime::replace_graph,
      runtime::set_audio_config,
      runtime::enable_runtime,
      runtime::disable_runtime,
      runtime::get_node_render_data,
      open_devtools,
      save_graph,
      read_text_file,
      plugin_command,
      node_command,
    ])
    .run(tauri::generate_context!())
    .unwrap();
}

/// Bring the main webview window back to the foreground after it has been
/// hidden (e.g. by the "minimize to tray on close" setting).
fn show_main_window<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
  if let Some(window) = app.get_webview_window("main") {
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
  }
}
