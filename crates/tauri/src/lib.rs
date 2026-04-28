use std::collections::BTreeMap;
use std::sync::{atomic::AtomicBool, Arc};

use cpal::traits::{DeviceTrait, HostTrait};
use serde::{Deserialize, Serialize};
use tauri::{async_runtime::Mutex, Builder, State};

pub(crate) mod driver;
pub(crate) mod nodes;
mod runtime;
pub(crate) mod vst3_common;

#[cfg(windows)]
pub use driver::endpoint::rename_endpoint_elevated;

use nodes::vst_node::VstPluginInfo;
use nodes::NodeSharedStore;
use runtime::AudioNode;

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
  runtime_thread: Option<std::thread::JoinHandle<runtime::Runtime>>,
  runtime_running: Option<Arc<AtomicBool>>,
  #[cfg(windows)]
  driver_handle: Option<Arc<crate::driver::client::DriverHandle>>,
  /// Virtual devices created via the menu panel (device_hex_id -> VirtualDevice).
  virtual_devices: BTreeMap<String, VirtualDevice>,
  /// Spectrum buffers for SpectrumAnalyzer nodes (node_id -> magnitude bins).
  spectrum_buffers: BTreeMap<String, Arc<std::sync::Mutex<Vec<f32>>>>,
  /// Waveform buffers for WaveformMonitor nodes (node_id -> rolling sample window).
  waveform_buffers: BTreeMap<String, Arc<std::sync::Mutex<Vec<f32>>>>,
  /// Cached VST3 plugin scan results (global, not per-node).
  vst_plugin_list: Vec<VstPluginInfo>,
  /// Type-erased per-node-instance shared state. Each node type stores its
  /// own state struct (e.g. `VstNodeShared`) keyed by node id. Plugin nodes
  /// added later can use the same mechanism.
  node_shared_store: Arc<NodeSharedStore>,
  /// Persistent node instances created via `create_node`. Used as the
  /// dispatch target for `node_command` so that node-specific IPC works
  /// regardless of whether the runtime is currently running.
  nodes: BTreeMap<String, AudioNode>,
}

/// A visible top-level window enumerated via `EnumWindows`.
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

/// Return the list of visible top-level windows with non-empty titles.
/// Used by the AppAudioCapture node to let the user pick a target application.
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

/// Open the WebView developer tools (browser devtools).
/// Works in debug builds; in release builds requires the `devtools` Cargo feature.
#[tauri::command]
fn open_devtools(window: tauri::WebviewWindow) {
  #[cfg(debug_assertions)]
  window.open_devtools();
  #[cfg(not(debug_assertions))]
  let _ = window;
}

/// Show a native Save As dialog and write the graph JSON to the chosen file.
/// Returns `true` if saved, `false` if the user cancelled.
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

/// Read the text content of a file at the given path.
/// Used to load a dropped graph JSON file.
#[tauri::command]
async fn read_text_file(path: String) -> Result<String, String> {
  std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// VST3 commands
// ---------------------------------------------------------------------------

/// Called at node creation time to run NodeTrait::create() and register the
/// instance into AppData. The persisted instance is the dispatch target for
/// `node_command` and shares its mutable state with the runtime instance via
/// `NodeSharedStore`.
#[tauri::command]
async fn create_node(state: State<'_, Mutex<AppData>>, node: AudioNode) -> Result<(), String> {
  let mut node = node;
  let mut app = state.lock().await;
  let store = app.node_shared_store.clone();

  // Inject shared store into node-types that need it.
  if let AudioNode::Vst(ref mut n) = node {
    n.shared_store = Some(store.clone());
  }

  // Run NodeTrait::create() which may populate shared state (e.g. VST
  // ctrl_cid) before the node is used.
  node.create_node()?;

  let id = node.id().to_string();
  app.nodes.insert(id, node);
  Ok(())
}

/// Unified IPC entry point for all node-specific subcommands.
///
/// For static (no-instance) operations such as `vst:scan`, dispatches based
/// on `node_type` only. For per-instance operations, looks up the node in
/// `AppData.nodes` and forwards to `NodeTrait::command`.
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
        let plugins = nodes::vst_node::scan_plugins_command(state).await?;
        serde_json::to_value(plugins).map_err(|e| e.to_string())
      }
      (t, op) => Err(format!(
        "node_command: unsupported static op '{}' for type '{}'",
        op.unwrap_or(""),
        t
      )),
    };
  }

  let mut app = state.lock().await;
  let node = app
    .nodes
    .get_mut(&node_id)
    .ok_or_else(|| format!("node_command: node '{node_id}' not found"))?;
  node.command(data)
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
      spectrum_buffers: BTreeMap::new(),
      waveform_buffers: BTreeMap::new(),
      vst_plugin_list: Vec::new(),
      node_shared_store: Arc::new(NodeSharedStore::new()),
      nodes: BTreeMap::new(),
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
      runtime::setup_runtime,
      runtime::enable_runtime,
      runtime::disable_runtime,
      open_devtools,
      runtime::get_node_render_data,
      save_graph,
      read_text_file,
      create_node,
      node_command,
    ])
    .run(tauri::generate_context!())
    .unwrap();
}
