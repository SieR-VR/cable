use std::collections::BTreeMap;
use std::sync::{
  atomic::{AtomicBool, Ordering},
  Arc,
};

use cpal::traits::{DeviceTrait, HostTrait};
use serde::{Deserialize, Serialize};
use tauri::{async_runtime::Mutex, Builder, State};

pub(crate) mod driver;
pub(crate) mod nodes;
mod runtime;
pub(crate) mod vst3_common;

#[cfg(windows)]
pub use driver::endpoint::rename_endpoint_elevated;

use nodes::app_audio_capture::AppAudioCaptureNode;
use nodes::audio_input_device::AudioInputDeviceNode;
use nodes::audio_output_device::AudioOutputDeviceNode;
use nodes::mixer::MixerNode;
use nodes::spectrum_analyzer::SpectrumAnalyzerNode;
use nodes::virtual_audio_input::VirtualAudioInputNode;
use nodes::virtual_audio_output::VirtualAudioOutputNode;
use nodes::vst_node::{VstNode, VstParamInfo, VstPluginInfo};
use nodes::waveform_monitor::WaveformMonitorNode;
use nodes::NodeTrait;

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
  vst_editors: BTreeMap<String, VstEditorHandle>,
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

    println!("Runtime thread stopped.");
    runtime
  });

  state.runtime_running = Some(running);
  state.runtime_thread = Some(handle);
}

fn stop_runtime_thread(state: &mut AppData) -> Result<(), String> {
  if let Some(running) = state.runtime_running.take() {
    running.store(false, Ordering::Relaxed);
  }

  if let Some(handle) = state.runtime_thread.take() {
    let runtime = handle
      .join()
      .map_err(|_| "Failed to join runtime thread".to_string())?;
    // Restore the runtime so enable_runtime can restart it without
    // requiring a full setup_runtime call.
    state.runtime = Some(runtime);
  }

  Ok(())
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

#[derive(Debug, Serialize, Deserialize)]
struct AudioGraph {
  nodes: Vec<AudioNode>,
  edges: Vec<AudioEdge>,
}

/// Per-frame render data returned by `get_node_render_data` for visualizer nodes.
#[derive(Debug, Serialize)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub(crate) enum NodeRenderData {
  SpectrumAnalyzer { bins: Vec<f32> },
  WaveformMonitor { samples: Vec<f32> },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub(crate) enum AudioNode {
  AudioInputDevice(AudioInputDeviceNode),
  AudioOutputDevice(AudioOutputDeviceNode),
  VirtualAudioInput(VirtualAudioInputNode),
  VirtualAudioOutput(VirtualAudioOutputNode),
  SpectrumAnalyzer(SpectrumAnalyzerNode),
  WaveformMonitor(WaveformMonitorNode),
  AppAudioCapture(AppAudioCaptureNode),
  Mixer(MixerNode),
  Vst(VstNode),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AudioEdge {
  id: String,

  from: String,
  to: String,
  to_handle: Option<String>,

  frequency: Option<u32>,
  channels: Option<u16>,
  bits_per_sample: Option<usize>,
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

  let mut app_state = state.lock().await;

  let was_running = app_state.runtime_running.is_some();
  if was_running {
    stop_runtime_thread(&mut app_state)?;
  }

  // Dispose the previous runtime (restored by stop_runtime_thread, or idle).
  if let Some(mut old_runtime) = app_state.runtime.take() {
    if let Err(e) = old_runtime.dispose_nodes() {
      eprintln!("Error disposing previous runtime nodes: {}", e);
    }
  }

  #[cfg(windows)]
  let driver_handle = app_state.driver_handle.clone();
  #[cfg(not(windows))]
  let driver_handle: Option<()> = None;

  // Build spectrum buffers for any SpectrumAnalyzer nodes in the new graph.
  let mut spectrum_buffers: BTreeMap<String, Arc<std::sync::Mutex<Vec<f32>>>> = BTreeMap::new();
  for node in &graph.nodes {
    if let AudioNode::SpectrumAnalyzer(n) = node {
      spectrum_buffers.insert(
        n.id().to_string(),
        Arc::new(std::sync::Mutex::new(Vec::new())),
      );
    }
  }
  app_state.spectrum_buffers = spectrum_buffers.clone();

  // Build waveform buffers for any WaveformMonitor nodes in the new graph.
  let mut waveform_buffers: BTreeMap<String, Arc<std::sync::Mutex<Vec<f32>>>> = BTreeMap::new();
  for node in &graph.nodes {
    if let AudioNode::WaveformMonitor(n) = node {
      waveform_buffers.insert(
        n.id().to_string(),
        Arc::new(std::sync::Mutex::new(Vec::new())),
      );
    }
  }
  app_state.waveform_buffers = waveform_buffers.clone();

  drop(app_state);

  let mut runtime = runtime::Runtime::new(
    buffer_size,
    sample_rate,
    graph.nodes,
    graph.edges,
    audio_host,
    driver_handle,
    spectrum_buffers,
    waveform_buffers,
  );

  runtime.init_nodes()?;

  // Extract ctrl_cid from VST nodes and store in AppData.
  let mut vst_ctrl_cids: BTreeMap<String, [u8; 16]> = BTreeMap::new();
  for node in &runtime.nodes {
    if let AudioNode::Vst(n) = node {
      if let Some(cid) = n.ctrl_cid {
        vst_ctrl_cids.insert(n.id().to_string(), cid);
      }
    }
  }

  let mut state = state.lock().await;
  state.vst_ctrl_cids = vst_ctrl_cids;
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

/// Return per-frame render data for all active visualizer nodes.
/// Returns a map of node_id → NodeRenderData covering every visualizer buffer.
/// A single call fetches data for all visualizer nodes in the current graph.
#[tauri::command]
async fn get_node_render_data(
  state: State<'_, Mutex<AppData>>,
) -> Result<BTreeMap<String, NodeRenderData>, String> {
  let app = state.lock().await;
  let mut result: BTreeMap<String, NodeRenderData> = BTreeMap::new();
  for (id, buf) in &app.spectrum_buffers {
    result.insert(
      id.clone(),
      NodeRenderData::SpectrumAnalyzer {
        bins: buf.lock().unwrap().clone(),
      },
    );
  }
  for (id, buf) in &app.waveform_buffers {
    result.insert(
      id.clone(),
      NodeRenderData::WaveformMonitor {
        samples: buf.lock().unwrap().clone(),
      },
    );
  }
  Ok(result)
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

/// Scans the system for VST3 plugins and returns the list.
/// The result is cached in AppData.
#[tauri::command]
async fn scan_vst3_plugins(state: State<'_, Mutex<AppData>>) -> Result<Vec<VstPluginInfo>, String> {
  let plugins = nodes::vst_node::scan_vst3_plugins();
  let mut app = state.lock().await;
  app.vst_plugin_list = plugins.clone();
  Ok(plugins)
}

/// Returns the parameter list for a plugin.
/// Returns cached values when the editor is open.
#[tauri::command]
async fn get_vst_params(
  state: State<'_, Mutex<AppData>>,
  node_id: String,
) -> Result<Vec<VstParamInfo>, String> {
  let app = state.lock().await;
  if let Some(buf) = app.vst_param_buffers.get(&node_id) {
    Ok(buf.lock().map_err(|e| e.to_string())?.clone())
  } else {
    Ok(Vec::new())
  }
}

/// Sets a single parameter value from the editor window.
#[tauri::command]
async fn set_vst_param(
  state: State<'_, Mutex<AppData>>,
  node_id: String,
  param_id: u32,
  value: f64,
) -> Result<(), String> {
  let app = state.lock().await;

  // Update shared parameter buffer
  if let Some(buf) = app.vst_param_buffers.get(&node_id) {
    if let Ok(mut params) = buf.lock() {
      if let Some(p) = params.iter_mut().find(|p| p.id == param_id) {
        p.value = value;
      }
    }
  }

  // Forward parameter to editor thread
  #[cfg(windows)]
  {
    if let Some(handle) = app.vst_editors.get(&node_id) {
      let hwnd_val = handle.hwnd.load(std::sync::atomic::Ordering::SeqCst);
      if hwnd_val != 0 {
        let _ = handle.param_tx.try_send((param_id, value));
        unsafe {
          use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
          use windows::Win32::UI::WindowsAndMessaging::PostMessageW;
          let _ = PostMessageW(
            Some(HWND(hwnd_val as *mut _)),
            WM_VST_PARAM,
            WPARAM(0),
            LPARAM(0),
          );
        }
      }
    }
  }

  Ok(())
}

// WM_USER + 1 — triggers parameter channel processing in the editor WndProc
#[cfg(windows)]
const WM_VST_PARAM: u32 = windows::Win32::UI::WindowsAndMessaging::WM_USER + 1;

/// Opens a VST3 editor window.
/// If already open, brings the window to the foreground.
#[cfg(windows)]
#[tauri::command]
async fn open_vst_editor(
  state: State<'_, Mutex<AppData>>,
  node_id: String,
  plugin_path: String,
) -> Result<(), String> {
  let mut app = state.lock().await;

  // If the editor is already open, focus it; if hwnd == 0 the window was closed — remove stale entry
  if let Some(handle) = app.vst_editors.get(&node_id) {
    let hwnd_val = handle.hwnd.load(std::sync::atomic::Ordering::SeqCst);
    if hwnd_val != 0 {
      unsafe {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{
          SetForegroundWindow, ShowWindow, SW_RESTORE,
        };
        let hwnd = HWND(hwnd_val as *mut _);
        let _ = ShowWindow(hwnd, SW_RESTORE);
        let _ = SetForegroundWindow(hwnd);
      }
      return Ok(());
    }
  }
  // hwnd == 0 or no entry: remove stale handle and create a new one
  if let Some(mut stale) = app.vst_editors.remove(&node_id) {
    if let Some(t) = stale.thread.take() {
      drop(app);
      let _ = t.join();
      app = state.lock().await;
    }
  }

  let hwnd_arc = Arc::new(std::sync::atomic::AtomicIsize::new(0));
  let params_arc: Arc<std::sync::Mutex<Vec<VstParamInfo>>> =
    Arc::new(std::sync::Mutex::new(Vec::new()));
  let (param_tx, param_rx) = std::sync::mpsc::sync_channel::<(u32, f64)>(64);

  let ctrl_cid = app
    .vst_ctrl_cids
    .get(&node_id)
    .copied()
    .ok_or_else(|| format!("ctrl_cid not found. Please press Apply first."))?;

  let hwnd_clone = hwnd_arc.clone();
  let params_clone = params_arc.clone();
  let node_id_clone = node_id.clone();

  let thread = std::thread::spawn(move || {
    run_vst_editor_thread(
      plugin_path,
      node_id_clone,
      ctrl_cid,
      hwnd_clone,
      param_rx,
      params_clone,
    );
  });

  // Register Arc in param_buffers so get_vst_params returns an empty list even
  // before the editor thread has populated it.
  app
    .vst_param_buffers
    .insert(node_id.clone(), params_arc.clone());

  app.vst_editors.insert(
    node_id,
    VstEditorHandle {
      hwnd: hwnd_arc,
      param_tx,
      params: params_arc,
      thread: Some(thread),
    },
  );
  Ok(())
}

/// Closes a VST3 editor window.
#[cfg(windows)]
#[tauri::command]
async fn close_vst_editor(state: State<'_, Mutex<AppData>>, node_id: String) -> Result<(), String> {
  let mut app = state.lock().await;
  if let Some(handle) = app.vst_editors.get(&node_id) {
    let hwnd_val = handle.hwnd.load(std::sync::atomic::Ordering::SeqCst);
    if hwnd_val != 0 {
      unsafe {
        use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
        use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_CLOSE};
        let _ = PostMessageW(
          Some(HWND(hwnd_val as *mut _)),
          WM_CLOSE,
          WPARAM(0),
          LPARAM(0),
        );
      }
    }
  }
  if let Some(mut handle) = app.vst_editors.remove(&node_id) {
    if let Some(t) = handle.thread.take() {
      drop(app); // Release Mutex before join to avoid deadlock
      let _ = t.join();
    }
  }
  Ok(())
}

/// VST3 editor window handle (Windows-only).
#[cfg(windows)]
struct VstEditorHandle {
  hwnd: Arc<std::sync::atomic::AtomicIsize>,
  param_tx: std::sync::mpsc::SyncSender<(u32, f64)>,
  #[allow(dead_code)]
  params: Arc<std::sync::Mutex<Vec<VstParamInfo>>>,
  thread: Option<std::thread::JoinHandle<()>>,
}

/// Per-window state accessed by the editor WndProc.
#[cfg(windows)]
struct EditorWindowState {
  plug_view: *mut vst3_common::IPlugView,
  controller: *mut vst3_common::IEditController,
  param_rx: std::sync::mpsc::Receiver<(u32, f64)>,
  params_shared: Arc<std::sync::Mutex<Vec<VstParamInfo>>>,
  _lib: libloading::Library,
}

/// VST3 editor thread entry point (Windows-only).
///
/// Load DLL → create IEditController → create IPlugView → create Win32 window → message loop.
#[cfg(windows)]
fn run_vst_editor_thread(
  plugin_path: String,
  node_id: String,
  ctrl_cid: [u8; 16],
  hwnd_out: Arc<std::sync::atomic::AtomicIsize>,
  param_rx: std::sync::mpsc::Receiver<(u32, f64)>,
  params_shared: Arc<std::sync::Mutex<Vec<VstParamInfo>>>,
) {
  use std::sync::atomic::Ordering;
  use vst3_common::{wchar_to_string, GetPluginFactoryFn, IID_IEDIT_CONTROLLER, K_RESULT_OK};
  use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
  use windows::Win32::System::LibraryLoader::GetModuleHandleW;
  use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DispatchMessageW, GetMessageW, RegisterClassExW, SetWindowLongPtrW,
    ShowWindow, TranslateMessage, CS_HREDRAW, CS_VREDRAW, GWLP_USERDATA, MSG, SW_SHOW, WNDCLASSEXW,
    WS_CAPTION, WS_SYSMENU,
  };

  unsafe {
    let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

    let result = (|| -> Result<(), String> {
      let lib =
        libloading::Library::new(&plugin_path).map_err(|e| format!("Failed to load DLL: {e}"))?;

      let get_factory: libloading::Symbol<GetPluginFactoryFn> = lib
        .get(b"GetPluginFactory\0")
        .map_err(|e| format!("GetPluginFactory symbol not found: {e}"))?;
      let factory = get_factory();
      if factory.is_null() {
        return Err("factory null".into());
      }
      let factory = &mut *factory;

      // // Create IEditController
      let ctrl_ptr = factory
        .create_instance(&ctrl_cid, &IID_IEDIT_CONTROLLER)
        .ok_or("Failed to create IEditController")?;
      let controller = &mut *(ctrl_ptr as *mut vst3_common::IEditController);
      let init_result = controller.initialize(std::ptr::null_mut());
      if init_result != K_RESULT_OK {
        controller.release();
        return Err(format!(
          "IEditController::initialize failed: {init_result:#x}"
        ));
      }

      // // Read parameters
      let count = controller.get_parameter_count();
      let mut param_list: Vec<VstParamInfo> = Vec::new();
      for i in 0..count {
        if let Some(info) = controller.get_parameter_info(i) {
          let value = controller.get_param_normalized(info.id);
          param_list.push(VstParamInfo {
            id: info.id,
            title: wchar_to_string(&info.title),
            value,
          });
        }
      }
      *params_shared.lock().map_err(|e| e.to_string())? = param_list;

      // Create IPlugView
      let view_ptr = controller
        .create_view()
        .ok_or("Failed to create IPlugView")?;
      let view = &mut *view_ptr;
      let rect = view.get_size().unwrap_or(vst3_common::ViewRect {
        left: 0,
        top: 0,
        right: 800,
        bottom: 600,
      });
      let w = rect.width().max(200) as i32;
      let h = rect.height().max(100) as i32;

      // Register and create Win32 window
      let hinstance = GetModuleHandleW(None).unwrap_or_default();
      let class_name: Vec<u16> = format!("VstEditor_{}", node_id)
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
      let window_title: Vec<u16> = format!("VST Editor — {}", node_id)
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

      let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(vst_editor_wnd_proc),
        hInstance: windows::Win32::Foundation::HINSTANCE(hinstance.0),
        lpszClassName: windows::core::PCWSTR(class_name.as_ptr()),
        ..Default::default()
      };
      RegisterClassExW(&wc);

      let hwnd = CreateWindowExW(
        windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
        windows::core::PCWSTR(class_name.as_ptr()),
        windows::core::PCWSTR(window_title.as_ptr()),
        WS_CAPTION | WS_SYSMENU,
        windows::Win32::UI::WindowsAndMessaging::CW_USEDEFAULT,
        windows::Win32::UI::WindowsAndMessaging::CW_USEDEFAULT,
        w,
        h,
        None,
        None,
        Some(windows::Win32::Foundation::HINSTANCE(hinstance.0)),
        None,
      )
      .map_err(|e| format!("CreateWindowExW failed: {e}"))?;

      // Set up EditorWindowState
      let state = Box::new(EditorWindowState {
        plug_view: view_ptr,
        controller: ctrl_ptr as *mut vst3_common::IEditController,
        param_rx,
        params_shared: params_shared.clone(),
        _lib: lib,
      });
      SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);

      // Attach plugin UI
      view.attached(hwnd.0 as *mut _, b"HWND\0");

      // Store HWND and show window
      hwnd_out.store(hwnd.0 as isize, Ordering::SeqCst);
      let _ = ShowWindow(hwnd, SW_SHOW);

      // Message loop
      let mut msg = MSG::default();
      while GetMessageW(&mut msg, None, 0, 0).as_bool() {
        let _ = TranslateMessage(&msg);
        DispatchMessageW(&msg);
      }

      // Window closed; reset hwnd to 0 for re-open detection
      hwnd_out.store(0, Ordering::SeqCst);

      Ok(())
    })();

    if let Err(e) = result {
      eprintln!("VST editor thread error: {e}");
      hwnd_out.store(0, Ordering::SeqCst);
    }

    CoUninitialize();
  }
}

/// VST editor window WndProc.
#[cfg(windows)]
unsafe extern "system" fn vst_editor_wnd_proc(
  hwnd: windows::Win32::Foundation::HWND,
  msg: u32,
  wparam: windows::Win32::Foundation::WPARAM,
  lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
  use windows::Win32::Foundation::LRESULT;
  use windows::Win32::UI::WindowsAndMessaging::{
    DefWindowProcW, DestroyWindow, GetWindowLongPtrW, PostQuitMessage, SetWindowLongPtrW,
    GWLP_USERDATA, WM_CLOSE, WM_DESTROY,
  };

  let user_data = GetWindowLongPtrW(hwnd, GWLP_USERDATA);

  match msg {
    WM_CLOSE => {
      if user_data != 0 {
        let state = &mut *(user_data as *mut EditorWindowState);
        if !state.plug_view.is_null() {
          (*state.plug_view).removed();
          (*state.plug_view).release();
          state.plug_view = std::ptr::null_mut();
        }
        if !state.controller.is_null() {
          (*state.controller).terminate();
          (*state.controller).release();
          state.controller = std::ptr::null_mut();
        }
      }
      let _ = DestroyWindow(hwnd);
      LRESULT(0)
    }
    WM_DESTROY => {
      if user_data != 0 {
        drop(Box::from_raw(user_data as *mut EditorWindowState));
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
      }
      PostQuitMessage(0);
      LRESULT(0)
    }
    WM_VST_PARAM => {
      if user_data != 0 {
        let state = &mut *(user_data as *mut EditorWindowState);
        while let Ok((id, value)) = state.param_rx.try_recv() {
          if !state.controller.is_null() {
            (*state.controller).set_param_normalized(id, value);
          }
          if let Ok(mut params) = state.params_shared.try_lock() {
            if let Some(p) = params.iter_mut().find(|p| p.id == id) {
              p.value = value;
            }
          }
        }
      }
      LRESULT(0)
    }
    _ => DefWindowProcW(hwnd, msg, wparam, lparam),
  }
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
      setup_runtime,
      enable_runtime,
      disable_runtime,
      open_devtools,
      get_node_render_data,
      save_graph,
      read_text_file,
      create_node,
      scan_vst3_plugins,
      get_vst_params,
      set_vst_param,
      #[cfg(windows)]
      open_vst_editor,
      #[cfg(windows)]
      close_vst_editor,
    ])
    .run(tauri::generate_context!())
    .unwrap();
}
