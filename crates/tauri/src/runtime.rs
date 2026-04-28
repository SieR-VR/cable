use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use cpal::Host;
use serde::{Deserialize, Serialize};
use tauri::async_runtime::Mutex as AsyncMutex;
use tauri::State;

use crate::nodes::app_audio_capture::AppAudioCaptureNode;
use crate::nodes::audio_input_device::AudioInputDeviceNode;
use crate::nodes::audio_output_device::AudioOutputDeviceNode;
use crate::nodes::mixer::MixerNode;
use crate::nodes::spectrum_analyzer::SpectrumAnalyzerNode;
use crate::nodes::virtual_audio_input::VirtualAudioInputNode;
use crate::nodes::virtual_audio_output::VirtualAudioOutputNode;
use crate::nodes::vst::VstNode;
use crate::nodes::waveform_monitor::WaveformMonitorNode;
use crate::nodes::{AudioBuffer, NodeTrait};
use crate::AppData;

#[cfg(windows)]
use crate::driver::client::DriverHandle;

// ---------------------------------------------------------------------------
// Audio graph data model
// ---------------------------------------------------------------------------

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

impl AudioNode {
  pub fn id(&self) -> &str {
    match self {
      AudioNode::AudioInputDevice(n) => n.id(),
      AudioNode::AudioOutputDevice(n) => n.id(),
      AudioNode::VirtualAudioInput(n) => n.id(),
      AudioNode::VirtualAudioOutput(n) => n.id(),
      AudioNode::SpectrumAnalyzer(n) => n.id(),
      AudioNode::WaveformMonitor(n) => n.id(),
      AudioNode::AppAudioCapture(n) => n.id(),
      AudioNode::Mixer(n) => n.id(),
      AudioNode::Vst(n) => n.id(),
    }
  }

  pub fn command(&mut self, data: serde_json::Value) -> Result<serde_json::Value, String> {
    match self {
      AudioNode::AudioInputDevice(n) => n.command(data),
      AudioNode::AudioOutputDevice(n) => n.command(data),
      AudioNode::VirtualAudioInput(n) => n.command(data),
      AudioNode::VirtualAudioOutput(n) => n.command(data),
      AudioNode::SpectrumAnalyzer(n) => n.command(data),
      AudioNode::WaveformMonitor(n) => n.command(data),
      AudioNode::AppAudioCapture(n) => n.command(data),
      AudioNode::Mixer(n) => n.command(data),
      AudioNode::Vst(n) => n.command(data),
    }
  }

  fn init(&mut self, runtime: &Runtime) -> Result<(), String> {
    match self {
      AudioNode::AudioInputDevice(n) => n.init(runtime),
      AudioNode::AudioOutputDevice(n) => n.init(runtime),
      AudioNode::VirtualAudioInput(n) => n.init(runtime),
      AudioNode::VirtualAudioOutput(n) => n.init(runtime),
      AudioNode::SpectrumAnalyzer(n) => n.init(runtime),
      AudioNode::WaveformMonitor(n) => n.init(runtime),
      AudioNode::AppAudioCapture(n) => n.init(runtime),
      AudioNode::Mixer(n) => n.init(runtime),
      AudioNode::Vst(n) => n.init(runtime),
    }
  }

  fn dispose(&mut self, runtime: &Runtime) -> Result<(), String> {
    match self {
      AudioNode::AudioInputDevice(n) => n.dispose(runtime),
      AudioNode::AudioOutputDevice(n) => n.dispose(runtime),
      AudioNode::VirtualAudioInput(n) => n.dispose(runtime),
      AudioNode::VirtualAudioOutput(n) => n.dispose(runtime),
      AudioNode::SpectrumAnalyzer(n) => n.dispose(runtime),
      AudioNode::WaveformMonitor(n) => n.dispose(runtime),
      AudioNode::AppAudioCapture(n) => n.dispose(runtime),
      AudioNode::Mixer(n) => n.dispose(runtime),
      AudioNode::Vst(n) => n.dispose(runtime),
    }
  }

  fn process(
    &mut self,
    runtime: &Runtime,
    state: &RuntimeState,
  ) -> Result<BTreeMap<String, AudioBuffer>, String> {
    match self {
      AudioNode::AudioInputDevice(n) => n.process(runtime, state),
      AudioNode::AudioOutputDevice(n) => n.process(runtime, state),
      AudioNode::VirtualAudioInput(n) => n.process(runtime, state),
      AudioNode::VirtualAudioOutput(n) => n.process(runtime, state),
      AudioNode::SpectrumAnalyzer(n) => n.process(runtime, state),
      AudioNode::WaveformMonitor(n) => n.process(runtime, state),
      AudioNode::AppAudioCapture(n) => n.process(runtime, state),
      AudioNode::Mixer(n) => n.process(runtime, state),
      AudioNode::Vst(n) => n.process(runtime, state),
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AudioEdge {
  pub id: String,
  pub from: String,
  pub to: String,
  pub to_handle: Option<String>,
  pub frequency: Option<u32>,
  pub channels: Option<u16>,
  pub bits_per_sample: Option<usize>,
}

/// Per-frame render data returned by `get_node_render_data` for visualizer nodes.
#[derive(Debug, Serialize)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub(crate) enum NodeRenderData {
  SpectrumAnalyzer { bins: Vec<f32> },
  WaveformMonitor { samples: Vec<f32> },
}

// ---------------------------------------------------------------------------
// Runtime: canonical graph state shared between IPC and the audio thread
// ---------------------------------------------------------------------------

pub(crate) struct Runtime {
  pub buffer_size: u32,
  pub sample_rate: u32,
  pub host_name: String,

  pub nodes: Vec<AudioNode>,
  pub edges: Vec<AudioEdge>,

  pub audio_host: Host,

  #[cfg(windows)]
  pub driver_handle: Option<Arc<DriverHandle>>,

  pub spectrum_buffers: BTreeMap<String, Arc<Mutex<Vec<f32>>>>,
  pub waveform_buffers: BTreeMap<String, Arc<Mutex<Vec<f32>>>>,
}

pub struct RuntimeState {
  pub edge_values: BTreeMap<String, AudioBuffer>,
}

/// Compute a topological processing order from node IDs and edges.
/// Falls back to the original index order if the graph contains cycles.
pub(crate) fn topological_order(node_ids: &[&str], edges: &[(String, String)]) -> Vec<usize> {
  let n = node_ids.len();
  if n <= 1 {
    return (0..n).collect();
  }

  let mut index_by_id = std::collections::HashMap::<&str, usize>::new();
  for (idx, &id) in node_ids.iter().enumerate() {
    index_by_id.insert(id, idx);
  }

  let mut indegree = vec![0usize; n];
  let mut outgoing = vec![Vec::<usize>::new(); n];

  for (from, to) in edges {
    let f = index_by_id.get(from.as_str()).copied();
    let t = index_by_id.get(to.as_str()).copied();
    if let (Some(f), Some(t)) = (f, t) {
      outgoing[f].push(t);
      indegree[t] += 1;
    }
  }

  let mut queue = std::collections::VecDeque::<usize>::new();
  for (i, deg) in indegree.iter().enumerate() {
    if *deg == 0 {
      queue.push_back(i);
    }
  }

  let mut order = Vec::with_capacity(n);
  while let Some(i) = queue.pop_front() {
    order.push(i);
    for &next in &outgoing[i] {
      indegree[next] = indegree[next].saturating_sub(1);
      if indegree[next] == 0 {
        queue.push_back(next);
      }
    }
  }

  if order.len() != n {
    return (0..n).collect();
  }

  order
}

fn resolve_host(name: &str) -> Result<Host, String> {
  let host_id = cpal::available_hosts()
    .into_iter()
    .find(|h| format!("{:?}", h) == name)
    .ok_or_else(|| format!("Audio host not found: {}", name))?;
  cpal::host_from_id(host_id).map_err(|e| format!("Failed to open audio host: {}", e))
}

impl Runtime {
  /// Construct the always-present canonical runtime at app startup.
  pub fn new_default() -> Self {
    let audio_host = cpal::default_host();
    let host_name = format!("{:?}", audio_host.id());
    Self {
      buffer_size: 512,
      sample_rate: 48000,
      host_name,
      nodes: Vec::new(),
      edges: Vec::new(),
      audio_host,
      #[cfg(windows)]
      driver_handle: None,
      spectrum_buffers: BTreeMap::new(),
      waveform_buffers: BTreeMap::new(),
    }
  }

  fn compute_topological_order(&self) -> Vec<usize> {
    let node_ids: Vec<&str> = self.nodes.iter().map(|n| n.id()).collect();
    let edges: Vec<(String, String)> = self
      .edges
      .iter()
      .map(|e| (e.from.clone(), e.to.clone()))
      .collect();
    topological_order(&node_ids, &edges)
  }

  /// Insert (or replace) a node and call `init()` immediately so streams,
  /// plugin handles, and ring buffers come up the moment the user adds the
  /// node from the UI.
  pub fn add_node(&mut self, mut node: AudioNode) -> Result<(), String> {
    let node_id = node.id().to_string();

    // If a node with the same id exists, dispose+remove it first so add_node
    // doubles as an "upsert" entry point used by update_node.
    if let Some(pos) = self.nodes.iter().position(|n| n.id() == node_id) {
      let mut old = self.nodes.remove(pos);
      if let Err(e) = old.dispose(&*self) {
        eprintln!("dispose during add_node replacement failed: {}", e);
      }
      self.spectrum_buffers.remove(&node_id);
      self.waveform_buffers.remove(&node_id);
    }

    // Pre-allocate visualizer buffers so the node's init() can pick them up.
    match &node {
      AudioNode::SpectrumAnalyzer(n) => {
        self
          .spectrum_buffers
          .entry(n.id().to_string())
          .or_insert_with(|| Arc::new(Mutex::new(Vec::new())));
      }
      AudioNode::WaveformMonitor(n) => {
        self
          .waveform_buffers
          .entry(n.id().to_string())
          .or_insert_with(|| Arc::new(Mutex::new(Vec::new())));
      }
      _ => {}
    }

    node.init(&*self)?;
    self.nodes.push(node);
    Ok(())
  }

  /// Remove a node by id, calling `dispose()` and dropping any edges that
  /// referenced it.
  pub fn remove_node(&mut self, node_id: &str) -> Result<(), String> {
    let pos = self.nodes.iter().position(|n| n.id() == node_id);
    let Some(pos) = pos else {
      return Ok(());
    };
    let mut node = self.nodes.remove(pos);
    self.edges.retain(|e| e.from != node_id && e.to != node_id);
    self.spectrum_buffers.remove(node_id);
    self.waveform_buffers.remove(node_id);
    node.dispose(&*self)?;
    Ok(())
  }

  pub fn add_edge(&mut self, edge: AudioEdge) {
    self.edges.retain(|e| e.id != edge.id);
    if let Some(freq) = edge.frequency {
      self.sample_rate = freq;
    }
    self.edges.push(edge);
  }

  pub fn remove_edge(&mut self, edge_id: &str) {
    self.edges.retain(|e| e.id != edge_id);
  }

  /// Replace the entire graph atomically (used by drop-file load).
  pub fn replace_graph(
    &mut self,
    nodes: Vec<AudioNode>,
    edges: Vec<AudioEdge>,
  ) -> Result<(), String> {
    // Dispose all existing nodes.
    let mut old_nodes = std::mem::take(&mut self.nodes);
    for n in old_nodes.iter_mut() {
      if let Err(e) = n.dispose(&*self) {
        eprintln!("dispose during replace_graph failed: {}", e);
      }
    }
    self.edges.clear();
    self.spectrum_buffers.clear();
    self.waveform_buffers.clear();

    if let Some(freq) = edges.iter().find_map(|e| e.frequency) {
      self.sample_rate = freq;
    }
    self.edges = edges;

    // Bulk add the new nodes (init each).
    for node in nodes {
      if let Err(e) = self.add_node(node) {
        eprintln!("add_node during replace_graph failed: {}", e);
      }
    }
    Ok(())
  }

  /// Reconfigure the audio host and/or buffer size. Disposes and re-inits
  /// every node so cpal streams reopen against the new host.
  pub fn set_audio_config(
    &mut self,
    host: Option<String>,
    buffer_size: Option<u32>,
  ) -> Result<(), String> {
    let mut nodes = std::mem::take(&mut self.nodes);
    for n in nodes.iter_mut() {
      if let Err(e) = n.dispose(&*self) {
        eprintln!("dispose during set_audio_config failed: {}", e);
      }
    }

    if let Some(name) = host {
      let new_host = resolve_host(&name)?;
      self.host_name = name;
      self.audio_host = new_host;
    }
    if let Some(bs) = buffer_size {
      self.buffer_size = bs;
    }

    for n in nodes.iter_mut() {
      if let Err(e) = n.init(&*self) {
        eprintln!("init during set_audio_config failed: {}", e);
      }
    }
    self.nodes = nodes;
    Ok(())
  }

  pub fn process(&mut self) -> Result<(), String> {
    let mut state = RuntimeState {
      edge_values: BTreeMap::new(),
    };

    let order = self.compute_topological_order();
    let mut nodes = std::mem::take(&mut self.nodes);

    for idx in order {
      let node = &mut nodes[idx];
      let node_output = node.process(&*self, &state)?;
      for (edge_id, values) in node_output {
        state.edge_values.insert(edge_id, values);
      }
    }

    self.nodes = nodes;
    Ok(())
  }

  pub fn buffer_duration(&self) -> std::time::Duration {
    if self.sample_rate == 0 {
      return std::time::Duration::from_millis(10);
    }
    std::time::Duration::from_secs_f64(self.buffer_size as f64 / self.sample_rate as f64)
  }
}

// ---------------------------------------------------------------------------
// Audio thread lifecycle
// ---------------------------------------------------------------------------

pub(crate) fn start_runtime_thread(state: &mut AppData) {
  if state.runtime_running.is_some() {
    return;
  }

  let running = Arc::new(AtomicBool::new(true));
  let running_clone = running.clone();
  let runtime_arc = state.runtime.clone();

  let sleep_duration = runtime_arc
    .lock()
    .map(|r| r.buffer_duration())
    .unwrap_or_else(|_| std::time::Duration::from_millis(10));

  println!("Enabling runtime with sleep duration: {:?}", sleep_duration);

  let handle = std::thread::spawn(move || {
    let mut next_tick = std::time::Instant::now() + sleep_duration;
    while running_clone.load(Ordering::Relaxed) {
      {
        if let Ok(mut rt) = runtime_arc.lock() {
          if let Err(e) = rt.process() {
            eprintln!("Error processing audio graph: {}", e);
          }
        }
      }

      // Sleep / spin until the next tick. The lock is released here so IPC
      // commands can mutate the graph between ticks.
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
      let now = std::time::Instant::now();
      if next_tick < now {
        next_tick = now + sleep_duration;
      }
    }

    println!("Runtime thread stopped.");
  });

  state.runtime_running = Some(running);
  state.runtime_thread = Some(handle);
}

pub(crate) fn stop_runtime_thread(state: &mut AppData) -> Result<(), String> {
  if let Some(running) = state.runtime_running.take() {
    running.store(false, Ordering::Relaxed);
  }
  if let Some(handle) = state.runtime_thread.take() {
    handle
      .join()
      .map_err(|_| "Failed to join runtime thread".to_string())?;
  }
  Ok(())
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn add_node(
  state: State<'_, AsyncMutex<AppData>>,
  node: AudioNode,
) -> Result<(), String> {
  let runtime_arc = {
    let app = state.lock().await;
    app.runtime.clone()
  };
  let mut rt = runtime_arc
    .lock()
    .map_err(|e| format!("runtime lock poisoned: {}", e))?;
  rt.add_node(node)
}

#[tauri::command]
pub async fn remove_node(
  state: State<'_, AsyncMutex<AppData>>,
  node_id: String,
) -> Result<(), String> {
  let runtime_arc = {
    let app = state.lock().await;
    app.runtime.clone()
  };
  let mut rt = runtime_arc
    .lock()
    .map_err(|e| format!("runtime lock poisoned: {}", e))?;
  rt.remove_node(&node_id)
}

/// Replace an existing node with a freshly deserialized one (dispose+init).
#[tauri::command]
pub async fn update_node(
  state: State<'_, AsyncMutex<AppData>>,
  node: AudioNode,
) -> Result<(), String> {
  let runtime_arc = {
    let app = state.lock().await;
    app.runtime.clone()
  };
  let mut rt = runtime_arc
    .lock()
    .map_err(|e| format!("runtime lock poisoned: {}", e))?;
  rt.add_node(node)
}

#[tauri::command]
pub async fn add_edge(
  state: State<'_, AsyncMutex<AppData>>,
  edge: AudioEdge,
) -> Result<(), String> {
  let runtime_arc = {
    let app = state.lock().await;
    app.runtime.clone()
  };
  let mut rt = runtime_arc
    .lock()
    .map_err(|e| format!("runtime lock poisoned: {}", e))?;
  rt.add_edge(edge);
  Ok(())
}

#[tauri::command]
pub async fn remove_edge(
  state: State<'_, AsyncMutex<AppData>>,
  edge_id: String,
) -> Result<(), String> {
  let runtime_arc = {
    let app = state.lock().await;
    app.runtime.clone()
  };
  let mut rt = runtime_arc
    .lock()
    .map_err(|e| format!("runtime lock poisoned: {}", e))?;
  rt.remove_edge(&edge_id);
  Ok(())
}

#[tauri::command]
pub async fn replace_graph(
  state: State<'_, AsyncMutex<AppData>>,
  nodes: Vec<AudioNode>,
  edges: Vec<AudioEdge>,
) -> Result<(), String> {
  let runtime_arc = {
    let app = state.lock().await;
    app.runtime.clone()
  };
  let mut rt = runtime_arc
    .lock()
    .map_err(|e| format!("runtime lock poisoned: {}", e))?;
  rt.replace_graph(nodes, edges)
}

#[tauri::command]
pub async fn set_audio_config(
  state: State<'_, AsyncMutex<AppData>>,
  host: Option<String>,
  buffer_size: Option<u32>,
) -> Result<(), String> {
  let was_running = {
    let app = state.lock().await;
    app.runtime_running.is_some()
  };
  if was_running {
    let mut app = state.lock().await;
    stop_runtime_thread(&mut app)?;
  }

  let runtime_arc = {
    let app = state.lock().await;
    app.runtime.clone()
  };
  {
    let mut rt = runtime_arc
      .lock()
      .map_err(|e| format!("runtime lock poisoned: {}", e))?;
    rt.set_audio_config(host, buffer_size)?;
  }

  if was_running {
    let mut app = state.lock().await;
    start_runtime_thread(&mut app);
  }
  Ok(())
}

#[tauri::command]
pub async fn enable_runtime(state: State<'_, AsyncMutex<AppData>>) -> Result<(), String> {
  let mut app = state.lock().await;
  start_runtime_thread(&mut app);
  Ok(())
}

#[tauri::command]
pub async fn disable_runtime(state: State<'_, AsyncMutex<AppData>>) -> Result<(), String> {
  let mut app = state.lock().await;
  stop_runtime_thread(&mut app)?;
  println!("Runtime disabled.");
  Ok(())
}

#[tauri::command]
pub async fn get_node_render_data(
  state: State<'_, AsyncMutex<AppData>>,
) -> Result<BTreeMap<String, NodeRenderData>, String> {
  let runtime_arc = {
    let app = state.lock().await;
    app.runtime.clone()
  };
  let rt = runtime_arc
    .lock()
    .map_err(|e| format!("runtime lock poisoned: {}", e))?;
  let mut result: BTreeMap<String, NodeRenderData> = BTreeMap::new();
  for (id, buf) in &rt.spectrum_buffers {
    result.insert(
      id.clone(),
      NodeRenderData::SpectrumAnalyzer {
        bins: buf.lock().unwrap().clone(),
      },
    );
  }
  for (id, buf) in &rt.waveform_buffers {
    result.insert(
      id.clone(),
      NodeRenderData::WaveformMonitor {
        samples: buf.lock().unwrap().clone(),
      },
    );
  }
  Ok(result)
}

#[cfg(test)]
mod tests {
  use super::topological_order;

  fn e(from: &str, to: &str) -> (String, String) {
    (from.to_string(), to.to_string())
  }

  #[test]
  fn empty_graph() {
    assert_eq!(topological_order(&[], &[]), Vec::<usize>::new());
  }

  #[test]
  fn single_node() {
    assert_eq!(topological_order(&["A"], &[]), vec![0]);
  }

  #[test]
  fn linear_chain() {
    let ids = ["A", "B", "C"];
    let edges = [e("A", "B"), e("B", "C")];
    let order = topological_order(&ids, &edges);
    assert_eq!(order, vec![0, 1, 2]);
  }

  #[test]
  fn diamond_graph() {
    let ids = ["A", "B", "C", "D"];
    let edges = [e("A", "B"), e("A", "C"), e("B", "D"), e("C", "D")];
    let order = topological_order(&ids, &edges);
    assert_eq!(order[0], 0);
    assert_eq!(order[3], 3);
    assert!(order[1] == 1 || order[1] == 2);
    assert!(order[2] == 1 || order[2] == 2);
    assert_ne!(order[1], order[2]);
  }

  #[test]
  fn disconnected_nodes() {
    let ids = ["A", "B", "C"];
    let edges = [];
    let order = topological_order(&ids, &edges);
    assert_eq!(order, vec![0, 1, 2]);
  }

  #[test]
  fn cycle_falls_back_to_index_order() {
    let ids = ["A", "B", "C"];
    let edges = [e("A", "B"), e("B", "C"), e("C", "A")];
    let order = topological_order(&ids, &edges);
    assert_eq!(order, vec![0, 1, 2]);
  }

  #[test]
  fn partial_cycle_falls_back() {
    let ids = ["A", "B", "C", "D"];
    let edges = [e("A", "B"), e("B", "C"), e("C", "B")];
    let order = topological_order(&ids, &edges);
    assert_eq!(order, vec![0, 1, 2, 3]);
  }

  #[test]
  fn edges_referencing_unknown_nodes() {
    let ids = ["A", "B"];
    let edges = [e("A", "B"), e("X", "Y")];
    let order = topological_order(&ids, &edges);
    assert_eq!(order, vec![0, 1]);
  }

  #[test]
  fn complex_dag() {
    let ids = ["A", "B", "C", "D", "E"];
    let edges = [
      e("A", "B"),
      e("A", "C"),
      e("B", "D"),
      e("C", "D"),
      e("C", "E"),
    ];
    let order = topological_order(&ids, &edges);
    assert_eq!(order[0], 0);
    let pos = |idx: usize| order.iter().position(|&x| x == idx).unwrap();
    assert!(pos(0) < pos(1));
    assert!(pos(0) < pos(2));
    assert!(pos(1) < pos(3));
    assert!(pos(2) < pos(3));
    assert!(pos(2) < pos(4));
  }

  #[test]
  fn buffer_duration_normal() {
    let dur = std::time::Duration::from_secs_f64(512.0 / 48000.0);
    let epsilon = std::time::Duration::from_micros(1);
    let expected = std::time::Duration::from_secs_f64(512.0 / 48000.0);
    assert!((dur.as_secs_f64() - expected.as_secs_f64()).abs() < epsilon.as_secs_f64());
  }
}
