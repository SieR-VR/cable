use std::collections::BTreeMap;
use std::sync::{atomic::AtomicBool, Arc, Mutex as StdMutex};

use cpal::traits::{DeviceTrait, HostTrait};
use serde::{Deserialize, Serialize};
use tauri::{async_runtime::Mutex, Builder, State};

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

/// Unified IPC entry point for all node-specific subcommands.
///
/// For static (no-instance) operations such as `vst:scan`, dispatches based
/// on `node_type` only. For per-instance operations, looks up the node in
/// the runtime graph and forwards to `NodeTrait::command`.
#[tauri::command]
async fn node_command(
  state: State<'_, Mutex<AppData>>,
  node_type: String,
  node_id: String,
  data: serde_json::Value,
) -> Result<serde_json::Value, String> {
  // Static (no node id) ops first.
  if node_id.is_empty() {
    return match (node_type.as_str(), data.get("op").and_then(|v| v.as_str())) {
      ("vst", Some("scan")) => {
        let plugins = nodes::vst::scan_plugins_command()?;
        serde_json::to_value(plugins).map_err(|e| e.to_string())
      }
      (t, op) => Err(format!(
        "node_command: unsupported static op '{}' for type '{}'",
        op.unwrap_or(""),
        t
      )),
    };
  }

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  Builder::default()
    .plugin(tauri_plugin_opener::init())
    .manage(Mutex::new(AppData {
      runtime: Arc::new(StdMutex::new(runtime::Runtime::new_default())),
      runtime_thread: None,
      runtime_running: None,
      #[cfg(windows)]
      driver_handle: None,
      virtual_devices: BTreeMap::new(),
    }))
    .invoke_handler(tauri::generate_handler![
      get_window_list,
      get_audio_hosts,
      get_audio_devices,
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
      node_command,
    ])
    .run(tauri::generate_context!())
    .unwrap();
}
