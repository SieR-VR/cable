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

/// Result type sent from the editor thread's Phase 1 back to `init()`.
///
/// Contains the fully-initialized `Vst3Plugin` (IComponent + IAudioProcessor),
/// the audio_cid, and the optional ctrl_cid extracted from the plugin factory.
#[cfg(windows)]
type VstInitResult = Result<(Vst3Plugin, [u8; 16], Option<[u8; 16]>), String>;

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
  /// Audio Module Class CID, stored during load_plugin for the editor thread.
  #[serde(skip)]
  audio_cid: Option<[u8; 16]>,
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

    #[cfg(windows)]
    return self.init_with_editor_thread(runtime);

    #[cfg(not(windows))]
    unsafe {
      self.load_plugin(runtime)
    }
  }

  fn dispose(&mut self, _runtime: &Runtime) -> Result<(), String> {
    println!("Disposing VST node: {}", self.id);
    #[cfg(windows)]
    {
      // Close any open editor window, then drop the handle.
      // Dropping VstEditorHandle drops open_editor_tx, which causes the editor
      // thread's Phase 2 recv() to return Err and the thread to exit cleanly.
      self.do_close_editor();
      self.editor = None;
    }
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

// ---------------------------------------------------------------------------
// Windows: editor-thread-based initialization
// Non-Windows: direct DLL load
// ---------------------------------------------------------------------------

impl VstNode {
  /// Windows path: spawns the editor thread immediately so that
  /// `IComponent::initialize()` runs on that thread. JUCE's MessageManager
  /// singleton registers the calling thread as its "message thread"; by
  /// making the editor thread call initialize() first we ensure JUCE
  /// dispatches callbacks where `GetMessageW` is running, eliminating the
  /// deadlock in `IPlugView::attached()`.
  #[cfg(windows)]
  fn init_with_editor_thread(&mut self, runtime: &Runtime) -> Result<(), String> {
    let (init_tx, init_rx) = std::sync::mpsc::sync_channel::<VstInitResult>(1);
    let (open_editor_tx, open_editor_rx) = std::sync::mpsc::sync_channel::<()>(1);
    let (param_tx, param_rx) = std::sync::mpsc::sync_channel::<(u32, f64)>(64);

    let hwnd_arc = Arc::new(std::sync::atomic::AtomicIsize::new(0));
    let params_arc = self.param_buffer.clone();

    let plugin_path = self.plugin_path.clone();
    let node_id = self.id.clone();
    let channels = self.channels;
    let num_inputs = self.num_inputs;
    let num_outputs = self.num_outputs;
    let buffer_size = runtime.buffer_size as i32;
    let sample_rate = runtime.sample_rate as f64;

    let hwnd_clone = hwnd_arc.clone();
    let params_clone = params_arc.clone();
    // Wrap param_rx in Arc<Mutex<…>> so the editor thread can pass it to
    // multiple successive open_editor_once calls without recreating the channel.
    let param_rx_arc = Arc::new(std::sync::Mutex::new(param_rx));
    let param_rx_clone = param_rx_arc.clone();

    let thread = std::thread::spawn(move || {
      run_vst_editor_thread(
        plugin_path,
        node_id,
        hwnd_clone,
        param_rx_clone,
        params_clone,
        init_tx,
        open_editor_rx,
        channels,
        num_inputs,
        num_outputs,
        buffer_size,
        sample_rate,
      );
    });

    // Block until Phase 1 completes on the editor thread.
    let (plugin, audio_cid, ctrl_cid) = match init_rx.recv() {
      Ok(Ok(v)) => v,
      Ok(Err(e)) => {
        drop(open_editor_tx);
        let _ = thread.join();
        return Err(e);
      }
      Err(_) => {
        drop(open_editor_tx);
        let _ = thread.join();
        return Err(format!(
          "VST3 editor thread died during init for '{}'",
          self.plugin_path
        ));
      }
    };

    self.audio_cid = Some(audio_cid);
    self.ctrl_cid = ctrl_cid;
    self.plugin = Some(plugin);
    self.editor = Some(VstEditorHandle {
      hwnd: hwnd_arc,
      param_tx,
      params: params_arc,
      thread: Some(thread),
      open_editor_tx,
    });

    println!("VST3 plugin initialized: {}", self.plugin_path);
    Ok(())
  }

  /// Loads the DLL and initializes IComponent / IAudioProcessor.
  /// Used on non-Windows where there is no editor thread.
  #[cfg_attr(windows, allow(dead_code))]
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
    self.audio_cid = Some(audio_cid);

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

    // Do NOT probe for editor availability here. Calling create_instance on the
    // current (Tauri command) thread causes JUCE to register this thread as its
    // "message thread". Subsequent IPlugView::attached() calls then deadlock
    // waiting for a Win32 message loop that never runs on this thread.
    //
    // Instead leave has_editor = None and attempt to open the editor lazily in
    // run_vst_editor_thread. If createView returns null there, the editor thread
    // exits cleanly and the command returns an error to the frontend.

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
  ///
  /// The editor thread is already running (started by `init_with_editor_thread`);
  /// this simply signals it to open the UI window (Phase 2 trigger).
  #[cfg(windows)]
  pub(crate) fn do_open_editor(&mut self, _plugin_path: String) -> Result<(), String> {
    let handle = self
      .editor
      .as_ref()
      .ok_or_else(|| "VST node not initialized. Press Apply first.".to_string())?;

    let hwnd_val = handle.hwnd.load(std::sync::atomic::Ordering::SeqCst);
    if hwnd_val != 0 {
      // Window already open — bring to front.
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

    // Signal the editor thread to open the UI (Phase 2 trigger).
    handle
      .open_editor_tx
      .try_send(())
      .map_err(|_| "Editor thread is not ready or has already exited".to_string())?;
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
// Plugin (no-instance) command dispatch for `plugin_command` IPC
// ============================================================================

/// Dispatches plugin-level (no node instance) commands for the VST plugin.
/// Currently supports `op: "scan"`.
pub(crate) fn plugin_command(data: serde_json::Value) -> Result<serde_json::Value, String> {
  let op = data.get("op").and_then(|v| v.as_str()).unwrap_or("");
  match op {
    "scan" => {
      let plugins = scan_vst3_plugins();
      serde_json::to_value(plugins).map_err(|e| e.to_string())
    }
    other => Err(format!("vst plugin_command: unknown op '{}'", other)),
  }
}

// WM_USER + 1 — triggers parameter channel processing in the editor WndProc
#[cfg(windows)]
const WM_VST_PARAM: u32 = windows::Win32::UI::WindowsAndMessaging::WM_USER + 1;

// WM_USER + 2 — WndProc spawns a helper thread that calls IPlugView::attached()
// while the message loop keeps pumping.
#[cfg(windows)]
const WM_VST_ATTACH: u32 = windows::Win32::UI::WindowsAndMessaging::WM_USER + 2;

// WM_USER + 3 — posted by the helper thread after attached() returns.
// WndProc re-queries the plugin size, resizes the window, and shows it.
#[cfg(windows)]
const WM_VST_AFTER_ATTACH: u32 = windows::Win32::UI::WindowsAndMessaging::WM_USER + 3;

// WM_USER + 4 — IPlugFrame::resizeView() posts this to request a window resize
// from the editor thread (safe to call SetWindowPos on the owning thread).
// WPARAM = desired client width (i32 as usize), LPARAM = desired client height.
#[cfg(windows)]
const WM_VST_RESIZE_VIEW: u32 = windows::Win32::UI::WindowsAndMessaging::WM_USER + 4;

/// VST3 editor window handle (Windows-only).
#[cfg(windows)]
pub(crate) struct VstEditorHandle {
  hwnd: Arc<std::sync::atomic::AtomicIsize>,
  param_tx: std::sync::mpsc::SyncSender<(u32, f64)>,
  #[allow(dead_code)]
  params: Arc<std::sync::Mutex<Vec<VstParamInfo>>>,
  #[allow(dead_code)]
  thread: Option<std::thread::JoinHandle<()>>,
  /// Triggers the editor thread's Phase 2 (open UI window).
  /// Dropping this without sending causes the thread to exit cleanly.
  open_editor_tx: std::sync::mpsc::SyncSender<()>,
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
  /// Host-side IPlugFrame implementation allocated on the heap.
  /// Released when the window closes (WM_CLOSE).
  plug_frame: *mut HostPlugFrame,
  /// Single-component plugins: the IComponent that backs the controller (same object, extra ref).
  /// WM_CLOSE must NOT call terminate() on the controller when this is Some.
  component_holder: Option<*mut vst3_com::IComponent>,
  /// Separated model: the IComponent kept alive so ctrl's juceCompo pointer remains valid.
  separated_comp: Option<*mut vst3_com::IComponent>,
  /// IConnectionPoint obtained from IComponent (separated model); released on close.
  comp_cp: Option<*mut vst3_com::IConnectionPoint>,
  /// IConnectionPoint obtained from IEditController (separated model); released on close.
  ctrl_cp: Option<*mut vst3_com::IConnectionPoint>,
  /// Shared with the editor thread so param_rx survives across open/close cycles.
  param_rx: Arc<std::sync::Mutex<std::sync::mpsc::Receiver<(u32, f64)>>>,
  params_shared: Arc<std::sync::Mutex<Vec<VstParamInfo>>>,
  _lib: libloading::Library,
}

/// VST3 editor thread entry point (Windows-only).
///
/// Phase 1: loads the DLL and calls IComponent::initialize() on this thread so
/// JUCE's MessageManager singleton registers *this* thread as its "message
/// thread". The resulting Vst3Plugin is sent back to init() via `init_tx`.
///
/// Phase 2: waits for open_editor_rx signals and opens a Win32+VST3 editor
/// window for each one. The GetMessageW loop on this thread processes JUCE's
/// internal PostMessage callbacks, preventing the deadlock that occurs when
/// attached() runs on a thread with no message pump.
#[cfg(windows)]
fn run_vst_editor_thread(
  plugin_path: String,
  node_id: String,
  hwnd_out: Arc<std::sync::atomic::AtomicIsize>,
  param_rx: Arc<std::sync::Mutex<std::sync::mpsc::Receiver<(u32, f64)>>>,
  params_shared: Arc<std::sync::Mutex<Vec<VstParamInfo>>>,
  init_tx: std::sync::mpsc::SyncSender<VstInitResult>,
  open_editor_rx: std::sync::mpsc::Receiver<()>,
  channels: u16,
  num_inputs: u16,
  num_outputs: u16,
  buffer_size: i32,
  sample_rate: f64,
) {
  use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};

  unsafe {
    let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

    // --- Phase 1: IComponent initialization on this thread ---
    // Running IComponent::initialize() here causes JUCE's MessageManager to
    // register this thread as its "message thread". All subsequent JUCE
    // callFunctionOnMessageThread calls will post messages to this thread's
    // GetMessageW loop, which prevents the attached() deadlock.
    let phase1 = vst_init_phase1(
      &plugin_path,
      channels,
      num_inputs,
      num_outputs,
      buffer_size,
      sample_rate,
    );

    let (audio_cid, ctrl_cid) = match &phase1 {
      Ok((_, a, c)) => (*a, *c),
      Err(_) => {
        let _ = init_tx.send(phase1);
        CoUninitialize();
        return;
      }
    };
    let _ = init_tx.send(phase1);
    // init_tx is dropped here; the blocking recv() in init() unblocks.

    // --- Phase 2: Open editor window on demand ---
    while let Ok(()) = open_editor_rx.recv() {
      hwnd_out.store(0, std::sync::atomic::Ordering::SeqCst);
      open_editor_once(
        &plugin_path,
        &node_id,
        ctrl_cid,
        audio_cid,
        &hwnd_out,
        &param_rx,
        &params_shared,
      );
    }

    CoUninitialize();
  }
}

/// Phase 1 helper: loads the plugin DLL and sets up IComponent + IAudioProcessor.
///
/// Must be called on the editor thread so JUCE MessageManager is owned by that
/// thread. The returned Vst3Plugin is sent to the main thread via a channel.
#[cfg(windows)]
unsafe fn vst_init_phase1(
  plugin_path: &str,
  channels: u16,
  num_inputs: u16,
  num_outputs: u16,
  buffer_size: i32,
  sample_rate: f64,
) -> VstInitResult {
  use vst3_com::{GetPluginFactoryFn, IID_ICOMPONENT, K_RESULT_OK};

  let lib = libloading::Library::new(plugin_path)
    .map_err(|e| format!("Failed to load VST3 DLL '{}': {}", plugin_path, e))?;

  let get_factory: libloading::Symbol<GetPluginFactoryFn> = lib
    .get(b"GetPluginFactory\0")
    .map_err(|e| format!("GetPluginFactory symbol not found: {e}"))?;
  let factory = get_factory();
  if factory.is_null() {
    return Err("GetPluginFactory returned null".into());
  }
  let factory = &mut *factory;

  // Find the Audio Module Class CID
  let num_classes = factory.count_classes();
  let mut audio_cid: Option<[u8; 16]> = None;
  for i in 0..num_classes {
    if let Some(info) = factory.get_class_info(i) {
      if vst3_com::cchar_to_string(&info.category).starts_with("Audio Module Class") {
        audio_cid = Some(info.cid);
        break;
      }
    }
  }
  let audio_cid = audio_cid.ok_or("Audio Module Class not found")?;

  let comp_ptr = factory
    .create_instance(&audio_cid, &IID_ICOMPONENT)
    .ok_or("Failed to create IComponent")?;
  let component = comp_ptr as *mut vst3_com::IComponent;

  // IComponent::initialize runs on this (editor) thread — JUCE MessageManager
  // registers this thread as its message thread right here.
  let result = (*component).initialize(std::ptr::null_mut());
  if result != K_RESULT_OK {
    (*component).release();
    return Err(format!("IComponent::initialize failed: {result:#x}"));
  }

  let proc_ptr = (*component)
    .query_interface(&vst3_com::IID_IAUDIO_PROCESSOR)
    .ok_or("IAudioProcessor interface not found")?;
  let processor = proc_ptr as *mut vst3_com::IAudioProcessor;

  let arrangement = if channels == 1 {
    vst3_com::K_MONO
  } else {
    vst3_com::K_STEREO
  };
  let mut inputs: Vec<u64> = vec![arrangement; num_inputs as usize];
  let mut outputs: Vec<u64> = vec![arrangement; num_outputs as usize];
  (*processor).set_bus_arrangements(&mut inputs, &mut outputs);

  for i in 0..(num_inputs as i32) {
    (*component).activate_bus(vst3_com::K_AUDIO, vst3_com::K_INPUT, i, true);
  }
  for i in 0..(num_outputs as i32) {
    (*component).activate_bus(vst3_com::K_AUDIO, vst3_com::K_OUTPUT, i, true);
  }

  let setup = vst3_com::ProcessSetup::new(
    vst3_com::K_REALTIME,
    vst3_com::K_SAMPLE32,
    buffer_size,
    sample_rate,
  );
  let r = (*processor).setup_processing(&setup);
  if r != K_RESULT_OK {
    println!("VST3 setupProcessing returned: {r:#x}");
  }
  (*component).set_active(true);
  (*processor).set_processing(true);

  let ctrl_cid = (*component).get_controller_class_id();

  Ok((
    Vst3Plugin {
      lib,
      component,
      processor,
    },
    audio_cid,
    ctrl_cid,
  ))
}

/// Phase 2 helper: opens a single editor window for the plugin and runs the
/// Win32 message loop until the window is closed.
///
/// Called on the editor thread so GetMessageW processes JUCE's internal
/// PostMessage callbacks dispatched from the attached() helper thread.
#[cfg(windows)]
unsafe fn open_editor_once(
  plugin_path: &str,
  node_id: &str,
  ctrl_cid: Option<[u8; 16]>,
  audio_cid: [u8; 16],
  hwnd_out: &Arc<std::sync::atomic::AtomicIsize>,
  param_rx: &Arc<std::sync::Mutex<std::sync::mpsc::Receiver<(u32, f64)>>>,
  params_shared: &Arc<std::sync::Mutex<Vec<VstParamInfo>>>,
) {
  use std::sync::atomic::Ordering;
  use vst3_com::{
    wchar_to_string, GetPluginFactoryFn, IID_ICOMPONENT, IID_IEDIT_CONTROLLER, K_RESULT_OK,
  };
  use windows::Win32::System::LibraryLoader::GetModuleHandleW;
  use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DispatchMessageW, GetMessageW, RegisterClassExW, SetWindowLongPtrW,
    TranslateMessage, CS_HREDRAW, CS_VREDRAW, GWLP_USERDATA, MSG, WNDCLASSEXW, WS_CAPTION,
    WS_SYSMENU,
  };

  let result = (|| -> Result<(), String> {
    // Load a second handle to the same DLL. On Windows, LoadLibrary only
    // increments the ref count for an already-loaded DLL, so JUCE globals
    // (including MessageManager) are not re-initialized.
    let lib2 = libloading::Library::new(plugin_path)
      .map_err(|e| format!("Failed to load DLL (editor): {e}"))?;

    let get_factory: libloading::Symbol<GetPluginFactoryFn> = lib2
      .get(b"GetPluginFactory\0")
      .map_err(|e| format!("GetPluginFactory not found: {e}"))?;
    let factory = get_factory();
    if factory.is_null() {
      return Err("factory null".into());
    }
    let factory = &mut *factory;

    let is_separated = match ctrl_cid {
      Some(cid) if cid != audio_cid => true,
      _ => false,
    };

    let (ctrl_ptr, component_holder, separated_comp, comp_cp, ctrl_cp): (
      *mut vst3_com::IEditController,
      Option<*mut vst3_com::IComponent>,
      Option<*mut vst3_com::IComponent>,
      Option<*mut vst3_com::IConnectionPoint>,
      Option<*mut vst3_com::IConnectionPoint>,
    ) = if is_separated {
      let cid = ctrl_cid.unwrap();

      let sep_comp_ptr = factory
        .create_instance(&audio_cid, &IID_ICOMPONENT)
        .ok_or("Failed to create IComponent for connection (separated)")?;
      let sep_component = &mut *(sep_comp_ptr as *mut vst3_com::IComponent);
      let r = sep_component.initialize(std::ptr::null_mut());
      if r != K_RESULT_OK {
        sep_component.release();
        return Err(format!(
          "IComponent::initialize failed (separated, connection): {r:#x}"
        ));
      }

      let ptr = factory
        .create_instance(&cid, &IID_IEDIT_CONTROLLER)
        .ok_or("Failed to create IEditController (separated)")?;
      let controller = &mut *(ptr as *mut vst3_com::IEditController);
      let r = controller.initialize(std::ptr::null_mut());
      if r != K_RESULT_OK {
        controller.release();
        sep_component.terminate();
        sep_component.release();
        return Err(format!("IEditController::initialize failed: {r:#x}"));
      }

      let comp_cp_raw = (*(sep_comp_ptr as *mut vst3_com::FUnknown))
        .query_interface(&vst3_com::IID_ICONNECTION_POINT);
      let ctrl_cp_raw = controller.query_interface(&vst3_com::IID_ICONNECTION_POINT);
      if let (Some(cp_raw), Some(ccp_raw)) = (comp_cp_raw, ctrl_cp_raw) {
        let comp_cp = &mut *(cp_raw as *mut vst3_com::IConnectionPoint);
        let ctrl_cp = &mut *(ccp_raw as *mut vst3_com::IConnectionPoint);
        let r1 = comp_cp.connect(ptr); // comp -> ctrl
        let r2 = ctrl_cp.connect(sep_comp_ptr); // ctrl -> comp
        println!("VST3 editor thread: IConnectionPoint::connect r1={r1:#x} r2={r2:#x}");
        (
          ptr as *mut vst3_com::IEditController,
          None,
          Some(sep_comp_ptr as *mut vst3_com::IComponent),
          Some(cp_raw as *mut vst3_com::IConnectionPoint),
          Some(ccp_raw as *mut vst3_com::IConnectionPoint),
        )
      } else {
        (
          ptr as *mut vst3_com::IEditController,
          None,
          Some(sep_comp_ptr as *mut vst3_com::IComponent),
          None,
          None,
        )
      }
    } else {
      let cid = ctrl_cid.unwrap_or(audio_cid);
      let comp_ptr = factory
        .create_instance(&cid, &IID_ICOMPONENT)
        .ok_or("Failed to create IComponent (single-component)")?;
      let component = &mut *(comp_ptr as *mut vst3_com::IComponent);
      let r = component.initialize(std::ptr::null_mut());
      if r != K_RESULT_OK {
        component.release();
        return Err(format!(
          "IComponent::initialize failed (single-component): {r:#x}"
        ));
      }
      let ctrl_ptr = component
        .query_interface(&IID_IEDIT_CONTROLLER)
        .ok_or("IEditController not found (single-component)")?;
      (
        ctrl_ptr as *mut vst3_com::IEditController,
        Some(comp_ptr as *mut vst3_com::IComponent),
        None,
        None,
        None,
      )
    };

    let controller = &mut *ctrl_ptr;

    // Populate parameter list
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

    println!(
      "VST3 editor thread: calling createView for '{}'",
      plugin_path
    );
    let view_ptr = match controller.create_view() {
      Some(v) => {
        println!("VST3 editor thread: createView succeeded ({:?})", v);
        v
      }
      None => {
        println!(
          "VST3 plugin '{}': createView returned null — no graphical editor.",
          plugin_path
        );
        return Ok(());
      }
    };
    println!("VST3 editor thread: createView OK, creating Win32 window");

    // Query the plugin's preferred size before creating the window so the client
    // area matches exactly when attached() runs. Fall back to 800×600 if getSize
    // is unavailable (e.g., before attached() some plugins return zeroes).
    let (initial_client_w, initial_client_h) = unsafe {
      let v = &mut *view_ptr;
      match v.get_size() {
        Some(r) if r.width() > 0 && r.height() > 0 => {
          let w = r.width() as i32;
          let h = r.height() as i32;
          println!("VST3 editor thread: pre-attach getSize = {w}x{h}");
          (w, h)
        }
        _ => (800_i32, 600_i32),
      }
    };

    // Convert client area to window size so the plugin has exactly the right
    // client area when attached() is called with our HWND.
    let (w, h) = unsafe {
      use windows::Win32::Foundation::RECT;
      use windows::Win32::UI::WindowsAndMessaging::{
        AdjustWindowRectEx, WINDOW_EX_STYLE, WS_CAPTION, WS_SYSMENU,
      };
      let mut rect = RECT {
        left: 0,
        top: 0,
        right: initial_client_w,
        bottom: initial_client_h,
      };
      let _ = AdjustWindowRectEx(
        &mut rect,
        WS_CAPTION | WS_SYSMENU,
        false,
        WINDOW_EX_STYLE(0),
      );
      (rect.right - rect.left, rect.bottom - rect.top)
    };

    let hinstance = GetModuleHandleW(None).unwrap_or_default();
    let class_name: Vec<u16> = format!("VstEditor_{node_id}")
      .encode_utf16()
      .chain(std::iter::once(0))
      .collect();
    let window_title: Vec<u16> = format!("VST Editor -- {node_id}")
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
    println!("VST3 editor thread: calling RegisterClassExW");
    RegisterClassExW(&wc);
    println!("VST3 editor thread: calling CreateWindowExW w={w} h={h}");

    let hwnd = CreateWindowExW(
      windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
      windows::core::PCWSTR(class_name.as_ptr()),
      windows::core::PCWSTR(window_title.as_ptr()),
      // WS_CLIPCHILDREN: prevents the host window from painting over the plugin's
      // child window area during WM_PAINT, which causes the blank white screen seen
      // with non-JUCE plugins.
      WS_CAPTION | WS_SYSMENU | windows::Win32::UI::WindowsAndMessaging::WS_CLIPCHILDREN,
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

    println!("VST3 editor thread: CreateWindowExW ok, hwnd={:?}", hwnd);

    // Allocate a HostPlugFrame so plugins can call resizeView() during attached().
    let frame = HostPlugFrame::new(hwnd);
    let frame_ptr = Box::into_raw(frame) as *mut HostPlugFrame;

    let state = Box::new(EditorWindowState {
      plug_view: view_ptr,
      controller: ctrl_ptr,
      plug_frame: frame_ptr,
      component_holder,
      separated_comp,
      comp_cp,
      ctrl_cp,
      param_rx: param_rx.clone(),
      params_shared: params_shared.clone(),
      _lib: lib2,
    });
    SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);

    // Store HWND before PostMessage so the frontend can see it.
    hwnd_out.store(hwnd.0 as isize, Ordering::SeqCst);

    {
      use windows::Win32::Foundation::{LPARAM, WPARAM};
      use windows::Win32::UI::WindowsAndMessaging::PostMessageW;
      let post_result = PostMessageW(Some(hwnd), WM_VST_ATTACH, WPARAM(0), LPARAM(0));
      println!("VST3 editor thread: PostMessageW(WM_VST_ATTACH) result={post_result:?}");
    }

    println!("VST3 editor thread: entering message loop");
    let mut msg = MSG::default();
    while GetMessageW(&mut msg, None, 0, 0).as_bool() {
      let _ = TranslateMessage(&msg);
      DispatchMessageW(&msg);
    }
    println!("VST3 editor thread: message loop exited");

    hwnd_out.store(0, Ordering::SeqCst);
    Ok(())
  })();

  if let Err(e) = result {
    println!("VST editor open_editor_once error: {e}");
    hwnd_out.store(0, std::sync::atomic::Ordering::SeqCst);
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
    WM_VST_ATTACH => {
      println!("VST3 WndProc: WM_VST_ATTACH received, user_data={user_data:#x}");
      // Call setFrame() then spawn a helper thread to call IPlugView::attached().
      //
      // setFrame must be called before attached() per VST3 spec. Doing it here
      // (on the editor thread, before spawning) keeps the frame valid for the
      // entire lifetime of the window.
      //
      // attached() is dispatched from a helper thread so the GetMessageW loop
      // on THIS thread keeps pumping while JUCE's MessageManager sends internal
      // PostMessage callbacks back to this thread (deadlock prevention).
      if user_data != 0 {
        let state = &*(user_data as *const EditorWindowState);
        if !state.plug_view.is_null() {
          // Call setFrame on the editor thread before spawning attached().
          if !state.plug_frame.is_null() {
            (*state.plug_view).set_frame(state.plug_frame as *mut _);
          }
          // Show the window before calling attached() so non-JUCE plugins that
          // render synchronously inside attached() have a visible parent to paint
          // into. For JUCE plugins this is harmless (JUCE defers rendering).
          {
            use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_SHOWNA};
            // SW_SHOWNA: show without stealing focus or changing activation
            let _ = ShowWindow(hwnd, SW_SHOWNA);
          }
          let view_addr = state.plug_view as usize;
          let hwnd_isize = hwnd.0 as isize;
          std::thread::spawn(move || unsafe {
            use windows::Win32::Foundation::HWND;
            let view = &mut *(view_addr as *mut vst3_com::IPlugView);
            let hwnd_copy = HWND(hwnd_isize as *mut _);
            println!("VST3 attach thread: calling IPlugView::attached()");
            let result = view.attached(hwnd_copy.0 as *mut _, b"HWND\0");
            println!("VST3 attach thread: IPlugView::attached returned {result:#x}");
            use windows::Win32::Foundation::{LPARAM, WPARAM};
            use windows::Win32::UI::WindowsAndMessaging::PostMessageW;
            let _ = PostMessageW(
              Some(hwnd_copy),
              WM_VST_AFTER_ATTACH,
              WPARAM(result as usize),
              LPARAM(0),
            );
          });
        }
      }
      LRESULT(0)
    }
    WM_VST_AFTER_ATTACH => {
      // attached() finished on the helper thread. Re-query the real plugin size,
      // convert client size to window size (AdjustWindowRectEx), resize the
      // container window, and make it visible.
      if user_data != 0 {
        use windows::Win32::Foundation::RECT;
        use windows::Win32::Graphics::Gdi::{InvalidateRect, UpdateWindow};
        use windows::Win32::UI::WindowsAndMessaging::{
          AdjustWindowRectEx, SetWindowPos, ShowWindow, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOZORDER,
          SW_SHOW, WINDOW_EX_STYLE, WS_CAPTION, WS_SYSMENU,
        };
        let state = &mut *(user_data as *mut EditorWindowState);
        if !state.plug_view.is_null() {
          let view = &mut *state.plug_view;
          if let Some(r) = view.get_size() {
            let client_w = r.width().max(200) as i32;
            let client_h = r.height().max(100) as i32;
            println!("VST3 WndProc: post-attach content size = {client_w}x{client_h}");
            // getSize() returns client area dimensions; inflate to window size.
            let mut rect = RECT {
              left: 0,
              top: 0,
              right: client_w,
              bottom: client_h,
            };
            let _ = AdjustWindowRectEx(
              &mut rect,
              WS_CAPTION | WS_SYSMENU,
              false,
              WINDOW_EX_STYLE(0),
            );
            let window_w = rect.right - rect.left;
            let window_h = rect.bottom - rect.top;
            println!("VST3 WndProc: adjusted window size = {window_w}x{window_h}");
            let _ = SetWindowPos(
              hwnd,
              None,
              0,
              0,
              window_w,
              window_h,
              SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
            // Notify the plugin of the final client size so it can re-render.
            // This is necessary when the plugin renders at creation time (before
            // the window was resized) or when resizeView() was not called.
            let new_rect = vst3_com::ViewRect {
              left: 0,
              top: 0,
              right: client_w,
              bottom: client_h,
            };
            view.on_size(&new_rect);
          }
          let _ = ShowWindow(hwnd, SW_SHOW);
          // Force an immediate repaint.
          let _ = InvalidateRect(Some(hwnd), None, false);
          let _ = UpdateWindow(hwnd);
        }
      }
      LRESULT(0)
    }
    WM_VST_RESIZE_VIEW => {
      // IPlugFrame::resizeView() was called by the plugin — resize the window to
      // match the requested client area. Only valid after attached().
      let client_w = wparam.0 as i32;
      let client_h = lparam.0 as i32;
      if client_w > 0 && client_h > 0 {
        use windows::Win32::Foundation::RECT;
        use windows::Win32::Graphics::Gdi::{InvalidateRect, UpdateWindow};
        use windows::Win32::UI::WindowsAndMessaging::{
          AdjustWindowRectEx, SetWindowPos, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOZORDER,
          WINDOW_EX_STYLE, WS_CAPTION, WS_SYSMENU,
        };
        let mut rect = RECT {
          left: 0,
          top: 0,
          right: client_w,
          bottom: client_h,
        };
        let _ = AdjustWindowRectEx(
          &mut rect,
          WS_CAPTION | WS_SYSMENU,
          false,
          WINDOW_EX_STYLE(0),
        );
        let _ = SetWindowPos(
          hwnd,
          None,
          0,
          0,
          rect.right - rect.left,
          rect.bottom - rect.top,
          SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
        let _ = InvalidateRect(Some(hwnd), None, false);
        let _ = UpdateWindow(hwnd);
      }
      LRESULT(0)
    }
    WM_CLOSE => {
      if user_data != 0 {
        let state = &mut *(user_data as *mut EditorWindowState);
        if !state.plug_view.is_null() {
          // Clear the frame reference before removing the view.
          (*state.plug_view).set_frame(std::ptr::null_mut());
          (*state.plug_view).removed();
          (*state.plug_view).release();
          state.plug_view = std::ptr::null_mut();
        }
        // Release the HostPlugFrame after the view no longer holds a reference to it.
        if !state.plug_frame.is_null() {
          let frame = &mut *(state.plug_frame as *mut vst3_com::IPlugFrame);
          frame.release();
          state.plug_frame = std::ptr::null_mut();
        }
        if !state.controller.is_null() {
          // Disconnect IConnectionPoint before releasing (separated model).
          // Peek at pointers without taking to ensure ordering: disconnect first,
          // then release the IConnectionPoint refs, then release the interfaces.
          let ctrl_raw = state.controller;
          let sep_comp_raw = state.separated_comp;
          if let Some(comp_cp) = state.comp_cp.take() {
            (*comp_cp).disconnect(ctrl_raw as *mut _);
            (*comp_cp).release();
          }
          if let Some(ctrl_cp) = state.ctrl_cp.take() {
            if let Some(sep_comp) = sep_comp_raw {
              (*ctrl_cp).disconnect(sep_comp as *mut _);
            }
            (*ctrl_cp).release();
          }
          // Single-component: controller IS the component — only release the extra ref
          // from queryInterface; terminate/release via component_holder below.
          // Separated: do a full terminate + release on the controller.
          if state.component_holder.is_none() {
            (*state.controller).terminate();
          }
          (*state.controller).release();
          state.controller = std::ptr::null_mut();
        }
        if let Some(comp) = state.component_holder.take() {
          if !comp.is_null() {
            (*comp).terminate();
            (*comp).release();
          }
        }
        if let Some(sep_comp) = state.separated_comp.take() {
          if !sep_comp.is_null() {
            (*sep_comp).terminate();
            (*sep_comp).release();
          }
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
        if let Ok(rx) = state.param_rx.try_lock() {
          while let Ok((id, value)) = rx.try_recv() {
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
      }
      LRESULT(0)
    }
    _ => DefWindowProcW(hwnd, msg, wparam, lparam),
  }
}

// ---------------------------------------------------------------------------
// HostPlugFrame — IPlugFrame implementation
//
// The host allocates one HostPlugFrame per editor window. The plugin calls
// resizeView() to request the host to resize the window; the host posts
// WM_VST_RESIZE_VIEW so that SetWindowPos is called on the owning thread.
// ---------------------------------------------------------------------------

#[cfg(windows)]
#[repr(C)]
pub(crate) struct HostPlugFrame {
  /// Must be first field: the COM vtable pointer.
  vtable: *const vst3_com::IPlugFrameVtbl,
  /// HWND of the editor window, stored as isize so the struct is Send.
  hwnd: isize,
  ref_count: std::sync::atomic::AtomicU32,
}

#[cfg(windows)]
unsafe impl Send for HostPlugFrame {}
#[cfg(windows)]
unsafe impl Sync for HostPlugFrame {}

#[cfg(windows)]
static HOST_PLUG_FRAME_VTBL: vst3_com::IPlugFrameVtbl = vst3_com::IPlugFrameVtbl {
  query_interface: host_plug_frame_query_interface,
  add_ref: host_plug_frame_add_ref,
  release: host_plug_frame_release,
  resize_view: host_plug_frame_resize_view,
};

#[cfg(windows)]
unsafe extern "system" fn host_plug_frame_query_interface(
  this: *mut vst3_com::IPlugFrame,
  iid: *const u8,
  out: *mut *mut std::ffi::c_void,
) -> i32 {
  use std::slice;
  let iid_bytes = slice::from_raw_parts(iid, 16);
  if iid_bytes == vst3_com::IID_IPLUG_FRAME || iid_bytes == vst3_com::IID_FUNKNOWN {
    host_plug_frame_add_ref(this);
    *out = this as *mut _;
    vst3_com::K_RESULT_OK
  } else {
    *out = std::ptr::null_mut();
    vst3_com::K_NO_INTERFACE
  }
}

#[cfg(windows)]
unsafe extern "system" fn host_plug_frame_add_ref(this: *mut vst3_com::IPlugFrame) -> u32 {
  let frame = &*(this as *const HostPlugFrame);
  frame
    .ref_count
    .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    + 1
}

#[cfg(windows)]
unsafe extern "system" fn host_plug_frame_release(this: *mut vst3_com::IPlugFrame) -> u32 {
  let frame = &*(this as *const HostPlugFrame);
  let prev = frame
    .ref_count
    .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
  if prev == 1 {
    // Reconstruct the Box to drop it.
    drop(Box::from_raw(this as *mut HostPlugFrame));
    0
  } else {
    prev - 1
  }
}

#[cfg(windows)]
unsafe extern "system" fn host_plug_frame_resize_view(
  this: *mut vst3_com::IPlugFrame,
  view: *mut vst3_com::IPlugView,
  new_rect: *const vst3_com::ViewRect,
) -> i32 {
  use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
  use windows::Win32::UI::WindowsAndMessaging::PostMessageW;

  let frame = &*(this as *const HostPlugFrame);
  let hwnd = HWND(frame.hwnd as *mut _);
  if hwnd.0.is_null() || new_rect.is_null() {
    return vst3_com::K_INVALID_ARGUMENT;
  }
  let r = &*new_rect;
  let w = r.width().max(1) as usize;
  let h = r.height().max(1) as usize;
  println!("IPlugFrame::resizeView requested {w}x{h}");

  // Notify the view that the size was accepted before posting the resize,
  // so the plugin doesn't retry.
  if !view.is_null() {
    (*view).on_size(new_rect);
  }

  // Hand off to the editor thread (owner of the HWND) to call SetWindowPos.
  let _ = PostMessageW(
    Some(hwnd),
    WM_VST_RESIZE_VIEW,
    WPARAM(w),
    LPARAM(h as isize),
  );
  vst3_com::K_RESULT_OK
}

#[cfg(windows)]
impl HostPlugFrame {
  pub fn new(hwnd: windows::Win32::Foundation::HWND) -> Box<Self> {
    Box::new(HostPlugFrame {
      vtable: &HOST_PLUG_FRAME_VTBL,
      hwnd: hwnd.0 as isize,
      ref_count: std::sync::atomic::AtomicU32::new(1),
    })
  }
}
