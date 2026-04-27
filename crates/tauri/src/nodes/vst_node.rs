/// VST3 plugin host node.
///
/// Dynamically loads the selected VST3 plugin DLL and processes audio.
/// Opens the DLL with libloading and calls IAudioProcessor via COM vtable dispatch.
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
  nodes::{vst3_com, AudioBuffer, NodeTrait},
  runtime::{Runtime, RuntimeState},
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
  lib: libloading::Library,
  component: *mut vst3_com::IComponent,
  processor: *mut vst3_com::IAudioProcessor,
}

// VST3 plugins guarantee thread safety per the spec.
// Audio processing is always called from the same thread (spin-loop).
unsafe impl Send for Vst3Plugin {}

impl std::fmt::Debug for Vst3Plugin {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "Vst3Plugin {{ component: {:?}, processor: {:?} }}", self.component, self.processor)
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
  /// CID obtained from IComponent::getControllerClassId(). Set after load_plugin.
  #[serde(skip)]
  pub ctrl_cid: Option<[u8; 16]>,
}

impl NodeTrait for VstNode {
  fn id(&self) -> &str {
    &self.id
  }

  /// Temporarily loads the DLL to extract only the IEditController CID.
  /// Can be called without a Runtime; executed immediately on plugin selection.
  fn create(&mut self) -> Result<(), String> {
    if self.plugin_path.is_empty() {
      return Ok(());
    }
    unsafe { self.extract_ctrl_cid() }
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

  fn process(&mut self, runtime: &Runtime,
             state: &RuntimeState)
             -> Result<BTreeMap<String, AudioBuffer>, String> {
    let plugin = match self.plugin.as_mut() {
      Some(p) => p,
      None => return self.passthrough(runtime, state),
    };

    unsafe { Self::process_with_plugin(plugin, &self.id, self.channels, self.num_inputs,
                                       self.num_outputs, runtime, state) }
  }
}

impl VstNode {
  /// Temporarily loads the DLL to extract only the IEditController CID.
  /// IComponent is released immediately after creation; lib is unloaded when the scope ends.
  unsafe fn extract_ctrl_cid(&mut self) -> Result<(), String> {
    let lib = libloading::Library::new(&self.plugin_path)
      .map_err(|e| format!("Failed to load VST3 DLL: {e}"))?;

    let get_factory: libloading::Symbol<vst3_com::GetPluginFactoryFn> =
      lib.get(b"GetPluginFactory\0")
         .map_err(|e| format!("GetPluginFactory symbol not found: {e}"))?;
    let factory = get_factory();
    if factory.is_null() {
      return Err("factory null".to_string());
    }
    let factory = &mut *factory;

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
    let audio_cid =
      audio_cid.ok_or_else(|| "Audio Module Class not found.".to_string())?;

    if let Some(comp_ptr) = factory.create_instance(&audio_cid, &vst3_com::IID_ICOMPONENT) {
      let component = comp_ptr as *mut vst3_com::IComponent;
      if (*component).initialize(std::ptr::null_mut()) == vst3_com::K_RESULT_OK {
        self.ctrl_cid = (*component).get_controller_class_id();
        (*component).terminate();
      }
      (*component).release();
    }
    // lib drops → DLL is unloaded
    Ok(())
  }

  /// Loads the DLL and initializes IComponent / IAudioProcessor.
  unsafe fn load_plugin(&mut self, runtime: &Runtime) -> Result<(), String> {
    let lib = libloading::Library::new(&self.plugin_path)
      .map_err(|e| format!("Failed to load VST3 DLL '{}': {}", self.plugin_path, e))?;

    // Obtain GetPluginFactory symbol
    let get_factory: libloading::Symbol<vst3_com::GetPluginFactoryFn> =
      lib.get(b"GetPluginFactory\0")
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
    let comp_ptr = factory.create_instance(&audio_cid, &vst3_com::IID_ICOMPONENT)
                          .ok_or_else(|| "Failed to create IComponent".to_string())?;
    let component = comp_ptr as *mut vst3_com::IComponent;
    let result = (*component).initialize(std::ptr::null_mut());
    if result != vst3_com::K_RESULT_OK {
      (*component).release();
      return Err(format!("IComponent::initialize failed: {result:#x}"));
    }

    // Query IAudioProcessor
    let proc_ptr = (*component).query_interface(&vst3_com::IID_IAUDIO_PROCESSOR)
                               .ok_or_else(|| "IAudioProcessor interface not found".to_string())?;
    let processor = proc_ptr as *mut vst3_com::IAudioProcessor;

    // Set bus speaker arrangements
    let arrangement = if self.channels == 1 { vst3_com::K_MONO } else { vst3_com::K_STEREO };
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
    let setup = vst3_com::ProcessSetup::new(vst3_com::K_REALTIME,
                                            vst3_com::K_SAMPLE32,
                                            runtime.buffer_size as i32,
                                            runtime.sample_rate as f64);
    let r = (*processor).setup_processing(&setup);
    if r != vst3_com::K_RESULT_OK {
      println!("VST3 setupProcessing returned: {r:#x}");
    }

    (*component).set_active(true);
    (*processor).set_processing(true);

    // Store ctrl_cid so the editor thread can reuse it without reloading the DLL.
    self.ctrl_cid = (*component).get_controller_class_id();

    self.plugin = Some(Vst3Plugin { lib, component, processor });
    println!("VST3 plugin initialized: {}", self.plugin_path);
    Ok(())
  }

  /// Calls the actual IAudioProcessor::process() to process audio.
  unsafe fn process_with_plugin(plugin: &mut Vst3Plugin, node_id: &str, channels: u16,
                                 num_inputs: u16, num_outputs: u16, runtime: &Runtime,
                                 state: &RuntimeState)
                                 -> Result<BTreeMap<String, AudioBuffer>, String> {
    let ch = channels as usize;
    let frames = runtime.buffer_size as usize;

    // Collect per-bus deinterleaved input buffers
    let mut in_channel_bufs: Vec<Vec<Vec<f32>>> = Vec::new();
    let mut proto: Option<AudioBuffer> = None;

    for bus_idx in 0..num_inputs {
      let handle_id = format!("vst-in-{}", bus_idx);
      let buf = runtime.edges.iter().find(|e| {
        e.to == node_id && e.to_handle.as_deref() == Some(&handle_id)
      }).and_then(|e| state.edge_values.get(&e.id));

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
    let mut in_ptrs: Vec<Vec<*mut f32>> = in_channel_bufs.iter_mut()
                                                          .map(|bus| {
                                                            bus.iter_mut()
                                                               .map(|ch| ch.as_mut_ptr())
                                                               .collect()
                                                          })
                                                          .collect();
    let mut out_ptrs: Vec<Vec<*mut f32>> = out_channel_bufs.iter_mut()
                                                            .map(|bus| {
                                                              bus.iter_mut()
                                                                 .map(|ch| ch.as_mut_ptr())
                                                                 .collect()
                                                            })
                                                            .collect();

    let mut in_buses: Vec<vst3_com::AudioBusBuffers> =
      in_ptrs.iter_mut()
             .map(|ptrs| vst3_com::AudioBusBuffers::new(ch as i32, 0, ptrs.as_mut_ptr()))
             .collect();
    let mut out_buses: Vec<vst3_com::AudioBusBuffers> =
      out_ptrs.iter_mut()
              .map(|ptrs| vst3_com::AudioBusBuffers::new(ch as i32, 0, ptrs.as_mut_ptr()))
              .collect();

    let mut process_data =
      vst3_com::ProcessData::new(frames as i32,
                                 in_buses.as_mut_ptr(),
                                 num_inputs as i32,
                                 out_buses.as_mut_ptr(),
                                 num_outputs as i32);

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
      let bus_idx: usize = 0; // use single output bus
      let chans = &out_channel_bufs[bus_idx.min(out_channel_bufs.len() - 1)];
      // Per-channel → interleaved
      let mut interleaved = vec![0.0f32; frames * ch];      for (c, chan) in chans.iter().enumerate() {
        for (f, &s) in chan.iter().enumerate() {
          interleaved[f * ch + c] = s;
        }
      }
      result.insert(edge.id.clone(), AudioBuffer::new(interleaved, channels, sample_rate, bits));
    }

    Ok(result)
  }

  /// Passes input through to output when no plugin is loaded.
  fn passthrough(&self, runtime: &Runtime,
                 state: &RuntimeState)
                 -> Result<BTreeMap<String, AudioBuffer>, String> {
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
        let out_buf =
          AudioBuffer::new(incoming_samples, p.channels, p.sample_rate, p.bits_per_sample);
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

  let mut scan_dirs = vec![std::path::PathBuf::from(r"C:\Program Files\Common Files\VST3")];
  if let Ok(local) = std::env::var("LOCALAPPDATA") {
    scan_dirs.push(std::path::PathBuf::from(local).join("Programs")
                                                   .join("Common")
                                                   .join("VST3"));
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
      let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
      let c = path.join("Contents").join("x86_64-win").join(format!("{}.vst3", stem));
      if c.exists() { c } else { continue }
    } else {
      path.clone()
    };

    let fallback_name = dll_path.file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("Unknown")
                                .to_string();
    let dll_str = dll_path.to_string_lossy().into_owned();

    match scan_single_dll(&dll_str, &fallback_name) {
      Ok(info) => results.push(info),
      Err(_) => {
        results.push(VstPluginInfo { name: fallback_name,
                                     path: dll_str,
                                     vendor: String::new(),
                                     num_inputs: 1,
                                     num_outputs: 1,
                                     num_params: 0 });
      }
    }
  }
}

/// Loads a single DLL and reads plugin info via GetPluginFactory.
fn scan_single_dll(dll_path: &str, fallback_name: &str) -> Result<VstPluginInfo, String> {
  unsafe {
    let lib = libloading::Library::new(dll_path)
      .map_err(|e| format!("Failed to load DLL: {e}"))?;

    let get_factory: libloading::Symbol<vst3_com::GetPluginFactoryFn> =
      lib.get(b"GetPluginFactory\0")
         .map_err(|e| format!("Symbol not found: {e}"))?;
    let factory = get_factory();
    if factory.is_null() {
      return Err("factory null".to_string());
    }
    let factory = &mut *factory;

    let vendor = factory.get_factory_info()
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

    Ok(VstPluginInfo { name: plugin_name,
                       path: dll_path.to_string(),
                       vendor,
                       num_inputs,
                       num_outputs,
                       num_params })
  }
}
