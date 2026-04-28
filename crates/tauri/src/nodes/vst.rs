/// VST3 plugin host node.
///
/// Dynamically loads the selected VST3 plugin DLL and processes audio.
/// Opens the DLL with libloading and calls IAudioProcessor via COM vtable dispatch.
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::{
  nodes::{AudioBuffer, NodeTrait},
  runtime::{Runtime, RuntimeState},
  vst3_common as vst3_com,
};

/// Single entry returned by the VST3 plugin scanner.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VstPluginInfo {
  pub name: String,
  pub path: String,
  pub vendor: String,
  pub num_inputs: u16,
  pub num_outputs: u16,
  pub num_params: u32,
}

/// VST3 parameter info (passed to the frontend).
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VstParamInfo {
  pub id: u32,
  pub title: String,
  pub value: f64,
}

// ---------------------------------------------------------------------------
// Vst3Plugin internal struct
// ---------------------------------------------------------------------------

/// Loaded VST3 plugin instance.
///
/// The IComponent / IAudioProcessor pointers remain valid as long as the DLL is alive.
/// Dropping this struct automatically releases COM interfaces and unloads the library.
struct Vst3Plugin {
  #[allow(dead_code)]
  lib: libloading::Library,
  component: *mut vst3_com::IComponent,
  processor: *mut vst3_com::IAudioProcessor,
}

// VST3 plugins guarantee thread safety per the spec.
// Audio processing is always called from the same thread (spin-loop).
unsafe impl Send for Vst3Plugin {}

impl std::fmt::Debug for Vst3Plugin {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "Vst3Plugin {{ component: {:?}, processor: {:?} }}",
      self.component, self.processor
    )
  }
}

impl Drop for Vst3Plugin {
  fn drop(&mut self) {
    unsafe {
      if !self.processor.is_null() {
        (*self.processor).set_processing(false);
        (*self.processor).release();
      }
      if !self.component.is_null() {
        (*self.component).set_active(false);
        (*self.component).terminate();
        (*self.component).release();
      }
      // lib drops last, unloading the DLL
    }
  }
}

// ---------------------------------------------------------------------------
// VstNode
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VstNode {
  /// Node ID (matches the ReactFlow node id)
  id: String,
  /// Absolute path to the selected .vst3 DLL
  plugin_path: String,
  /// Number of input buses (handles vst-in-0..N-1)
  num_inputs: u16,
  /// Number of output buses (handles vst-out-0..N-1)
  num_outputs: u16,
  /// Number of processing channels shared by inputs and outputs (typically 2 = stereo)
  channels: u16,
  /// Normalized parameter values [0.0, 1.0] in index order
  params: Vec<f64>,

  #[serde(skip)]
  plugin: Option<Vst3Plugin>,
  /// IEditController class id, populated by `extract_ctrl_cid` /
  /// `load_plugin`. Read by `openEditor` to instantiate the editor.
  #[serde(skip)]
  ctrl_cid: Option<[u8; 16]>,
  /// Latest parameter snapshot. Written by the editor thread and by
  /// `setParam`; read by `getParams`.
  #[serde(skip)]
  param_buffer: Arc<Mutex<Vec<VstParamInfo>>>,
  /// Currently-open editor handle (Windows-only).
  #[cfg(windows)]
  #[serde(skip)]
  editor: Option<VstEditorHandle>,
}

impl NodeTrait for VstNode {
  fn id(&self) -> &str {
    &self.id
  }

  fn init(&mut self, runtime: &Runtime) -> Result<(), String> {
    println!("Initializing VST node: {} ({})", self.id, self.plugin_path);

    if self.plugin_path.is_empty() {
      return Ok(());
    }

    unsafe { self.load_plugin(runtime) }
  }

  fn dispose(&mut self, _runtime: &Runtime) -> Result<(), String> {
    println!("Disposing VST node: {}", self.id);
    // Vst3Plugin::drop() handles COM release and DLL unload.
    self.plugin = None;
    Ok(())
  }

  fn process(
    &mut self,
    runtime: &Runtime,
    state: &RuntimeState,
  ) -> Result<BTreeMap<String, AudioBuffer>, String> {
    let plugin = match self.plugin.as_mut() {
      Some(p) => p,
      None => return self.passthrough(runtime, state),
    };

    unsafe {
      Self::process_with_plugin(
        plugin,
        &self.id,
        self.channels,
        self.num_inputs,
        self.num_outputs,
        runtime,
        state,
      )
    }
  }

  fn command(&mut self, data: serde_json::Value) -> Result<serde_json::Value, String> {
    let op = data
      .get("op")
      .and_then(|v| v.as_str())
      .ok_or_else(|| "missing 'op' field".to_string())?
      .to_string();

    match op.as_str() {
      "getParams" => {
        let params = self.param_buffer.lock().map_err(|e| e.to_string())?.clone();
        serde_json::to_value(params).map_err(|e| e.to_string())
      }
      "setParam" => {
        let param_id = data
          .get("paramId")
          .and_then(|v| v.as_u64())
          .ok_or_else(|| "missing 'paramId'".to_string())? as u32;
        let value = data
          .get("value")
          .and_then(|v| v.as_f64())
          .ok_or_else(|| "missing 'value'".to_string())?;
        self.do_set_param(param_id, value);
        Ok(serde_json::Value::Null)
      }
      #[cfg(windows)]
      "openEditor" => {
        let plugin_path = data
          .get("pluginPath")
          .and_then(|v| v.as_str())
          .ok_or_else(|| "missing 'pluginPath'".to_string())?
          .to_string();
        self.do_open_editor(plugin_path)?;
        Ok(serde_json::Value::Null)
      }
      #[cfg(windows)]
      "closeEditor" => {
        self.do_close_editor();
        Ok(serde_json::Value::Null)
      }
      #[cfg(not(windows))]
      "openEditor" | "closeEditor" => Err("VST editor is Windows-only".into()),
      other => Err(format!("unknown VST op: {other}")),
    }
  }
}

impl VstNode {
  /// Loads the DLL and initializes IComponent / IAudioProcessor.
  unsafe fn load_plugin(&mut self, runtime: &Runtime) -> Result<(), String> {
    let lib = libloading::Library::new(&self.plugin_path)
      .map_err(|e| format!("Failed to load VST3 DLL '{}': {}", self.plugin_path, e))?;

    // Obtain GetPluginFactory symbol
    let get_factory: libloading::Symbol<vst3_com::GetPluginFactoryFn> = lib
      .get(b"GetPluginFactory\0")
      .map_err(|e| format!("GetPluginFactory symbol not found: {}", e))?;
    let factory = get_factory();
    if factory.is_null() {
      return Err("GetPluginFactory returned null.".to_string());
    }
    let factory = &mut *factory;

    // Search for Audio Module Class CID
    let num_classes = factory.count_classes();
    let mut audio_cid: Option<[u8; 16]> = None;
    for i in 0..num_classes {
      if let Some(info) = factory.get_class_info(i) {
        let cat = vst3_com::cchar_to_string(&info.category);
        if cat.starts_with("Audio Module Class") {
          audio_cid = Some(info.cid);
          break;
        }
      }
    }
    let audio_cid = audio_cid.ok_or_else(|| "Audio Module Class not found.".to_string())?;

    // Create IComponent
    let comp_ptr = factory
      .create_instance(&audio_cid, &vst3_com::IID_ICOMPONENT)
      .ok_or_else(|| "Failed to create IComponent".to_string())?;
    let component = comp_ptr as *mut vst3_com::IComponent;
    let result = (*component).initialize(std::ptr::null_mut());
    if result != vst3_com::K_RESULT_OK {
      (*component).release();
      return Err(format!("IComponent::initialize failed: {result:#x}"));
    }

    // Query IAudioProcessor
    let proc_ptr = (*component)
      .query_interface(&vst3_com::IID_IAUDIO_PROCESSOR)
      .ok_or_else(|| "IAudioProcessor interface not found".to_string())?;
    let processor = proc_ptr as *mut vst3_com::IAudioProcessor;

    // Set bus speaker arrangements
    let arrangement = if self.channels == 1 {
      vst3_com::K_MONO
    } else {
      vst3_com::K_STEREO
    };
    let mut inputs: Vec<u64> = vec![arrangement; self.num_inputs as usize];
    let mut outputs: Vec<u64> = vec![arrangement; self.num_outputs as usize];
    (*processor).set_bus_arrangements(&mut inputs, &mut outputs);

    // Activate input and output buses
    for i in 0..(self.num_inputs as i32) {
      (*component).activate_bus(vst3_com::K_AUDIO, vst3_com::K_INPUT, i, true);
    }
    for i in 0..(self.num_outputs as i32) {
      (*component).activate_bus(vst3_com::K_AUDIO, vst3_com::K_OUTPUT, i, true);
    }

    // setupProcessing
    let setup = vst3_com::ProcessSetup::new(
      vst3_com::K_REALTIME,
      vst3_com::K_SAMPLE32,
      runtime.buffer_size as i32,
      runtime.sample_rate as f64,
    );
    let r = (*processor).setup_processing(&setup);
    if r != vst3_com::K_RESULT_OK {
      println!("VST3 setupProcessing returned: {r:#x}");
    }

    (*component).set_active(true);
    (*processor).set_processing(true);

    // Cache ctrl_cid so the editor thread can reuse it without reloading the DLL.
    let cid = (*component).get_controller_class_id();
    if cid.is_some() {
      self.ctrl_cid = cid;
    }

    self.plugin = Some(Vst3Plugin {
      lib,
      component,
      processor,
    });
    println!("VST3 plugin initialized: {}", self.plugin_path);
    Ok(())
  }

  /// Calls the actual IAudioProcessor::process() to process audio.
  unsafe fn process_with_plugin(
    plugin: &mut Vst3Plugin,
    node_id: &str,
    channels: u16,
    num_inputs: u16,
    num_outputs: u16,
    runtime: &Runtime,
    state: &RuntimeState,
  ) -> Result<BTreeMap<String, AudioBuffer>, String> {
    let ch = channels as usize;
    let frames = runtime.buffer_size as usize;

    // Collect per-bus deinterleaved input buffers
    let mut in_channel_bufs: Vec<Vec<Vec<f32>>> = Vec::new();
    let mut proto: Option<AudioBuffer> = None;

    for bus_idx in 0..num_inputs {
      let handle_id = format!("vst-in-{}", bus_idx);
      let buf = runtime
        .edges
        .iter()
        .find(|e| e.to == node_id && e.to_handle.as_deref() == Some(&handle_id))
        .and_then(|e| state.edge_values.get(&e.id));

      let samples = if let Some(b) = buf {
        if proto.is_none() {
          proto = Some(b.clone());
        }
        b.samples.clone()
      } else {
        vec![0.0f32; frames * ch]
      };

      // Deinterleave: interleaved → per-channel
      let mut chans: Vec<Vec<f32>> = vec![vec![0.0f32; frames]; ch];
      for (i, s) in samples.iter().enumerate() {
        chans[i % ch][i / ch] = *s;
      }
      in_channel_bufs.push(chans);
    }

    // Output channel buffers (zero-initialized)
    let mut out_channel_bufs: Vec<Vec<Vec<f32>>> =
      vec![vec![vec![0.0f32; frames]; ch]; num_outputs as usize];

    // Build AudioBusBuffers pointer arrays
    let mut in_ptrs: Vec<Vec<*mut f32>> = in_channel_bufs
      .iter_mut()
      .map(|bus| bus.iter_mut().map(|ch| ch.as_mut_ptr()).collect())
      .collect();
    let mut out_ptrs: Vec<Vec<*mut f32>> = out_channel_bufs
      .iter_mut()
      .map(|bus| bus.iter_mut().map(|ch| ch.as_mut_ptr()).collect())
      .collect();

    let mut in_buses: Vec<vst3_com::AudioBusBuffers> = in_ptrs
      .iter_mut()
      .map(|ptrs| vst3_com::AudioBusBuffers::new(ch as i32, 0, ptrs.as_mut_ptr()))
      .collect();
    let mut out_buses: Vec<vst3_com::AudioBusBuffers> = out_ptrs
      .iter_mut()
      .map(|ptrs| vst3_com::AudioBusBuffers::new(ch as i32, 0, ptrs.as_mut_ptr()))
      .collect();

    let mut process_data = vst3_com::ProcessData::new(
      frames as i32,
      in_buses.as_mut_ptr(),
      num_inputs as i32,
      out_buses.as_mut_ptr(),
      num_outputs as i32,
    );

    (*plugin.processor).process(&mut process_data);

    // Interleave output channels back into AudioBuffer
    let sample_rate = proto.as_ref().map_or(48000, |p| p.sample_rate);
    let bits = proto.as_ref().map_or(32, |p| p.bits_per_sample);

    let mut result = BTreeMap::new();
    for edge in &runtime.edges {
      if edge.from != node_id {
        continue;
      }
      // Determine output bus index: vst-out-N or first bus
      // TODO: support multi-bus routing — currently only bus 0 is used (see docs/known-issues.md)
      let bus_idx: usize = 0; // use single output bus
      let chans = &out_channel_bufs[bus_idx.min(out_channel_bufs.len() - 1)];
      // Per-channel → interleaved
      let mut interleaved = vec![0.0f32; frames * ch];
      for (c, chan) in chans.iter().enumerate() {
        for (f, &s) in chan.iter().enumerate() {
          interleaved[f * ch + c] = s;
        }
      }
      result.insert(
        edge.id.clone(),
        AudioBuffer::new(interleaved, channels, sample_rate, bits),
      );
    }

    Ok(result)
  }

  /// Passes input through to output when no plugin is loaded.
  fn passthrough(
    &self,
    runtime: &Runtime,
    state: &RuntimeState,
  ) -> Result<BTreeMap<String, AudioBuffer>, String> {
    let mut incoming_samples: Vec<f32> = Vec::new();
    let mut proto: Option<AudioBuffer> = None;

    for edge in &runtime.edges {
      if edge.to == self.id {
        if let Some(buf) = state.edge_values.get(&edge.id) {
          incoming_samples.extend_from_slice(&buf.samples);
          if proto.is_none() {
            proto = Some(buf.clone());
          }
        }
      }
    }

    let mut output = BTreeMap::new();
    if let Some(p) = proto {
      if !incoming_samples.is_empty() {
        let out_buf = AudioBuffer::new(
          incoming_samples,
          p.channels,
          p.sample_rate,
          p.bits_per_sample,
        );
        for edge in &runtime.edges {
          if edge.from == self.id {
            output.insert(edge.id.clone(), out_buf.clone());
          }
        }
      }
    }

    Ok(output)
  }
}

// ---------------------------------------------------------------------------
// Plugin scanning
// ---------------------------------------------------------------------------

/// Scans system VST3 plugin directories.
///
/// Calls GetPluginFactory to read the actual plugin name and vendor.
/// Falls back to filename-based info if the DLL fails to load.
pub fn scan_vst3_plugins() -> Vec<VstPluginInfo> {
  let mut results = Vec::new();

  let mut scan_dirs = vec![std::path::PathBuf::from(
    r"C:\Program Files\Common Files\VST3",
  )];
  if let Ok(local) = std::env::var("LOCALAPPDATA") {
    scan_dirs.push(
      std::path::PathBuf::from(local)
        .join("Programs")
        .join("Common")
        .join("VST3"),
    );
  }

  for dir in scan_dirs {
    if dir.exists() {
      scan_vst3_dir(&dir, &mut results);
    }
  }

  results
}

fn scan_vst3_dir(dir: &std::path::Path, results: &mut Vec<VstPluginInfo>) {
  let entries = match std::fs::read_dir(dir) {
    Ok(e) => e,
    Err(_) => return,
  };

  for entry in entries.flatten() {
    let path = entry.path();
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if !ext.eq_ignore_ascii_case("vst3") {
      continue;
    }

    let dll_path = if path.is_dir() {
      let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
      let c = path
        .join("Contents")
        .join("x86_64-win")
        .join(format!("{}.vst3", stem));
      if c.exists() {
        c
      } else {
        continue;
      }
    } else {
      path.clone()
    };

    let fallback_name = dll_path
      .file_stem()
      .and_then(|s| s.to_str())
      .unwrap_or("Unknown")
      .to_string();
    let dll_str = dll_path.to_string_lossy().into_owned();

    match scan_single_dll(&dll_str, &fallback_name) {
      Ok(info) => results.push(info),
      Err(_) => {
        results.push(VstPluginInfo {
          name: fallback_name,
          path: dll_str,
          vendor: String::new(),
          num_inputs: 1,
          num_outputs: 1,
          num_params: 0,
        });
      }
    }
  }
}

/// Loads a single DLL and reads plugin info via GetPluginFactory.
fn scan_single_dll(dll_path: &str, fallback_name: &str) -> Result<VstPluginInfo, String> {
  unsafe {
    let lib = libloading::Library::new(dll_path).map_err(|e| format!("Failed to load DLL: {e}"))?;

    let get_factory: libloading::Symbol<vst3_com::GetPluginFactoryFn> = lib
      .get(b"GetPluginFactory\0")
      .map_err(|e| format!("Symbol not found: {e}"))?;
    let factory = get_factory();
    if factory.is_null() {
      return Err("factory null".to_string());
    }
    let factory = &mut *factory;

    let vendor = factory
      .get_factory_info()
      .map(|fi| vst3_com::cchar_to_string(&fi.vendor))
      .unwrap_or_default();

    let num_classes = factory.count_classes();
    let mut plugin_name = fallback_name.to_string();
    let num_inputs: u16 = 1;
    let num_outputs: u16 = 1;
    let num_params: u32 = 0;

    for i in 0..num_classes {
      if let Some(info) = factory.get_class_info(i) {
        let cat = vst3_com::cchar_to_string(&info.category);
        if cat.starts_with("Audio Module Class") {
          let name = vst3_com::cchar_to_string(&info.name);
          if !name.is_empty() {
            plugin_name = name;
          }
          // Accurate I/O channel counts require creating IComponent;
          // keep defaults for scan performance.
          let _ = (num_inputs, num_outputs, num_params);
          break;
        }
      }
    }

    Ok(VstPluginInfo {
      name: plugin_name,
      path: dll_path.to_string(),
      vendor,
      num_inputs,
      num_outputs,
      num_params,
    })
  }
}

// ============================================================================
// Per-instance editor / parameter helpers
// ============================================================================

impl VstNode {
  /// Updates a parameter value in the shared buffer and forwards it to the
  /// editor window if one is open (Windows).
  pub(crate) fn do_set_param(&mut self, param_id: u32, value: f64) {
    if let Ok(mut params) = self.param_buffer.lock() {
      if let Some(p) = params.iter_mut().find(|p| p.id == param_id) {
        p.value = value;
      }
    }

    #[cfg(windows)]
    {
      if let Some(handle) = self.editor.as_ref() {
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
  }

  /// Opens (or focuses) the VST3 editor window for this node.
  #[cfg(windows)]
  pub(crate) fn do_open_editor(&mut self, plugin_path: String) -> Result<(), String> {
    if let Some(handle) = self.editor.as_ref() {
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
    // Stale handle (window already closed). Drop without joining; the worker
    // thread will exit on its own once Win32 cleanup finishes.
    self.editor = None;

    let ctrl_cid = self
      .ctrl_cid
      .ok_or_else(|| "ctrl_cid not found. Please press Apply first.".to_string())?;

    let hwnd_arc = Arc::new(std::sync::atomic::AtomicIsize::new(0));
    let params_arc = self.param_buffer.clone();
    let (param_tx, param_rx) = std::sync::mpsc::sync_channel::<(u32, f64)>(64);

    let hwnd_clone = hwnd_arc.clone();
    let params_clone = params_arc.clone();
    let node_id_clone = self.id.clone();

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

    self.editor = Some(VstEditorHandle {
      hwnd: hwnd_arc,
      param_tx,
      params: params_arc,
      thread: Some(thread),
    });
    Ok(())
  }

  /// Closes the editor window (best-effort) and detaches the worker thread.
  #[cfg(windows)]
  pub(crate) fn do_close_editor(&mut self) {
    if let Some(handle) = self.editor.take() {
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
      // JoinHandle dropped without join — the worker thread continues until
      // Win32/COM cleanup finishes and exits on its own.
      drop(handle);
    }
  }
}

// ============================================================================
// Static (no-node) Tauri command: VST3 plugin scan
// ============================================================================

/// Scans the system for VST3 plugins and returns the list.
pub(crate) fn scan_plugins_command() -> Result<Vec<VstPluginInfo>, String> {
  Ok(scan_vst3_plugins())
}

// WM_USER + 1 — triggers parameter channel processing in the editor WndProc
#[cfg(windows)]
const WM_VST_PARAM: u32 = windows::Win32::UI::WindowsAndMessaging::WM_USER + 1;

/// VST3 editor window handle (Windows-only).
#[cfg(windows)]
pub(crate) struct VstEditorHandle {
  hwnd: Arc<std::sync::atomic::AtomicIsize>,
  param_tx: std::sync::mpsc::SyncSender<(u32, f64)>,
  #[allow(dead_code)]
  params: Arc<std::sync::Mutex<Vec<VstParamInfo>>>,
  #[allow(dead_code)]
  thread: Option<std::thread::JoinHandle<()>>,
}

#[cfg(windows)]
impl std::fmt::Debug for VstEditorHandle {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "VstEditorHandle {{ hwnd: {} }}",
      self.hwnd.load(std::sync::atomic::Ordering::SeqCst)
    )
  }
}

/// Per-window state accessed by the editor WndProc.
#[cfg(windows)]
struct EditorWindowState {
  plug_view: *mut vst3_com::IPlugView,
  controller: *mut vst3_com::IEditController,
  param_rx: std::sync::mpsc::Receiver<(u32, f64)>,
  params_shared: Arc<std::sync::Mutex<Vec<VstParamInfo>>>,
  _lib: libloading::Library,
}

/// VST3 editor thread entry point (Windows-only).
///
/// Load DLL → create IEditController → create IPlugView → create Win32 window → message loop.
#[cfg(windows)]
pub(crate) fn run_vst_editor_thread(
  plugin_path: String,
  node_id: String,
  ctrl_cid: [u8; 16],
  hwnd_out: Arc<std::sync::atomic::AtomicIsize>,
  param_rx: std::sync::mpsc::Receiver<(u32, f64)>,
  params_shared: Arc<std::sync::Mutex<Vec<VstParamInfo>>>,
) {
  use std::sync::atomic::Ordering;
  use vst3_com::{wchar_to_string, GetPluginFactoryFn, IID_IEDIT_CONTROLLER, K_RESULT_OK};
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
      let controller = &mut *(ctrl_ptr as *mut vst3_com::IEditController);
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
      let rect = view.get_size().unwrap_or(vst3_com::ViewRect {
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
        controller: ctrl_ptr as *mut vst3_com::IEditController,
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
