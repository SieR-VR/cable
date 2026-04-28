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

use nodes::vst_node::{VstParamInfo, VstPluginInfo};
use nodes::NodeTrait;
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
  /// Cached VST3 plugin scan results.
  vst_plugin_list: Vec<VstPluginInfo>,
  /// VST3 parameter buffer (node_id → parameter list).
  /// Populated from IEditController when the editor is opened.
  vst_param_buffers: BTreeMap<String, Arc<std::sync::Mutex<Vec<VstParamInfo>>>>,
  /// VST3 IEditController CID cache (node_id → 16-byte CID).
  /// Populated after setup_runtime completes; read by open_vst_editor.
  vst_ctrl_cids: BTreeMap<String, [u8; 16]>,
  /// Open VST3 editor windows (node_id → handle). Windows-only.
  #[cfg(windows)]
  vst_editors: BTreeMap<String, crate::nodes::vst_node::VstEditorHandle>,
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

/// Called at node creation time to run NodeTrait::create().
/// For VST nodes, temporarily loads the DLL to extract ctrl_cid,
/// enabling the editor to be opened without pressing Apply first.
#[tauri::command]
async fn create_node(state: State<'_, Mutex<AppData>>, node: AudioNode) -> Result<(), String> {
  let mut node = node;
  if let AudioNode::Vst(ref mut n) = node {
    use crate::nodes::NodeTrait;
    n.create()?;
    if let Some(cid) = n.ctrl_cid {
      let mut app = state.lock().await;
      app.vst_ctrl_cids.insert(n.id().to_string(), cid);
    }
  }
  Ok(())
}

/// Unified IPC entry point for all node-specific subcommands.
///
/// Dispatches by `node_type` to the appropriate per-type handler. For nodes
/// whose state lives on the node instance itself, this would call into
/// `NodeTrait::command`; for nodes whose state lives in `AppData` (like VST),
/// it delegates to a free function in the node module.
#[tauri::command]
async fn node_command(
  state: State<'_, Mutex<AppData>>,
  node_type: String,
  node_id: String,
  data: serde_json::Value,
) -> Result<serde_json::Value, String> {
  match node_type.as_str() {
    "vst" => nodes::vst_node::handle_command(state, node_id, data).await,
    other => Err(format!("node_command: unknown node type '{other}'")),
  }
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
      vst_param_buffers: BTreeMap::new(),
      vst_ctrl_cids: BTreeMap::new(),
      #[cfg(windows)]
      vst_editors: BTreeMap::new(),
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
