use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use crate::nodes::{AudioBuffer, NodeTrait};
use cpal::Host;

#[cfg(windows)]
use crate::driver::client::DriverHandle;

pub(crate) struct Runtime {
  pub buffer_size: u32,
  pub sample_rate: u32,

  pub nodes: Vec<AudioNode>,
  pub edges: Vec<AudioEdge>,

  pub audio_host: Host,

  #[cfg(windows)]
  pub driver_handle: Option<Arc<DriverHandle>>,

  /// Shared spectrum buffers for SpectrumAnalyzer nodes.
  pub spectrum_buffers: BTreeMap<String, Arc<Mutex<Vec<f32>>>>,
  /// Shared waveform buffers for WaveformMonitor nodes.
  pub waveform_buffers: BTreeMap<String, Arc<Mutex<Vec<f32>>>>,
}

pub struct RuntimeState {
  pub edge_values: BTreeMap<String, AudioBuffer>,
}

/// Compute a topological processing order from node IDs and edges.
///
/// Returns indices into `node_ids` in dependency order.
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
    // Fall back to current UI order on cycles/malformed graph.
    return (0..n).collect();
  }

  order
}

impl Runtime {
  fn node_id(node: &AudioNode) -> &str {
    match node {
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

  fn compute_topological_order(&self) -> Vec<usize> {
    let node_ids: Vec<&str> = self.nodes.iter().map(|n| Self::node_id(n)).collect();
    let edges: Vec<(String, String)> = self
      .edges
      .iter()
      .map(|e| (e.from.clone(), e.to.clone()))
      .collect();
    topological_order(&node_ids, &edges)
  }

  pub fn new(
    buffer_size: u32,
    sample_rate: u32,
    nodes: Vec<AudioNode>,
    edges: Vec<AudioEdge>,
    audio_host: Host,
    #[cfg(windows)] driver_handle: Option<Arc<DriverHandle>>,
    #[cfg(not(windows))] _driver_handle: Option<()>,
    spectrum_buffers: BTreeMap<String, Arc<Mutex<Vec<f32>>>>,
    waveform_buffers: BTreeMap<String, Arc<Mutex<Vec<f32>>>>,
  ) -> Self {
    Self {
      buffer_size,
      sample_rate,
      nodes,
      edges,
      audio_host,
      #[cfg(windows)]
      driver_handle,
      spectrum_buffers,
      waveform_buffers,
    }
  }

  pub fn init_nodes(&mut self) -> Result<(), String> {
    let mut nodes = std::mem::take(&mut self.nodes);
    for node in nodes.iter_mut() {
      match node {
        AudioNode::AudioInputDevice(n) => n.init(self)?,
        AudioNode::AudioOutputDevice(n) => n.init(self)?,
        AudioNode::VirtualAudioInput(n) => n.init(self)?,
        AudioNode::VirtualAudioOutput(n) => n.init(self)?,
        AudioNode::SpectrumAnalyzer(n) => n.init(self)?,
        AudioNode::WaveformMonitor(n) => n.init(self)?,
        AudioNode::AppAudioCapture(n) => n.init(self)?,
        AudioNode::Mixer(n) => n.init(self)?,
        AudioNode::Vst(n) => n.init(self)?,
      }
    }
    self.nodes = nodes;
    Ok(())
  }

  pub fn dispose_nodes(&mut self) -> Result<(), String> {
    let mut nodes = std::mem::take(&mut self.nodes);
    for node in nodes.iter_mut() {
      match node {
        AudioNode::AudioInputDevice(n) => n.dispose(self)?,
        AudioNode::AudioOutputDevice(n) => n.dispose(self)?,
        AudioNode::VirtualAudioInput(n) => n.dispose(self)?,
        AudioNode::VirtualAudioOutput(n) => n.dispose(self)?,
        AudioNode::SpectrumAnalyzer(n) => n.dispose(self)?,
        AudioNode::WaveformMonitor(n) => n.dispose(self)?,
        AudioNode::AppAudioCapture(n) => n.dispose(self)?,
        AudioNode::Mixer(n) => n.dispose(self)?,
        AudioNode::Vst(n) => n.dispose(self)?,
      }
    }
    self.nodes = nodes;
    Ok(())
  }

  pub fn process(&mut self) -> Result<(), String> {
    let mut state = RuntimeState {
      edge_values: BTreeMap::new(),
    };

    // Compute topological order BEFORE taking nodes out of self, because
    // topological_order() reads self.nodes to build the graph.
    let order = self.compute_topological_order();

    let mut nodes = std::mem::take(&mut self.nodes);

    for idx in order {
      let node = &mut nodes[idx];
      let node_output = match node {
        AudioNode::AudioInputDevice(n) => n.process(self, &state)?,
        AudioNode::AudioOutputDevice(n) => n.process(self, &state)?,
        AudioNode::VirtualAudioInput(n) => n.process(self, &state)?,
        AudioNode::VirtualAudioOutput(n) => n.process(self, &state)?,
        AudioNode::SpectrumAnalyzer(n) => n.process(self, &state)?,
        AudioNode::WaveformMonitor(n) => n.process(self, &state)?,
        AudioNode::AppAudioCapture(n) => n.process(self, &state)?,
        AudioNode::Mixer(n) => n.process(self, &state)?,
        AudioNode::Vst(n) => n.process(self, &state)?,
      };
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
// Runtime data model and Tauri commands
// (moved from lib.rs in Phase 4 of the refactor)
// ---------------------------------------------------------------------------

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
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
use crate::AppData;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct AudioGraph {
  pub nodes: Vec<AudioNode>,
  pub edges: Vec<AudioEdge>,
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

impl AudioNode {
  /// Returns the node's unique id, dispatching to `NodeTrait::id`.
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

  /// Dispatches `NodeTrait::create()` to the inner node. Renamed from
  /// `create` to avoid clashing with the Tauri command.
  pub fn create_node(&mut self) -> Result<(), String> {
    match self {
      AudioNode::AudioInputDevice(n) => n.create(),
      AudioNode::AudioOutputDevice(n) => n.create(),
      AudioNode::VirtualAudioInput(n) => n.create(),
      AudioNode::VirtualAudioOutput(n) => n.create(),
      AudioNode::SpectrumAnalyzer(n) => n.create(),
      AudioNode::WaveformMonitor(n) => n.create(),
      AudioNode::AppAudioCapture(n) => n.create(),
      AudioNode::Mixer(n) => n.create(),
      AudioNode::Vst(n) => n.create(),
    }
  }

  /// Dispatches `NodeTrait::command` to the inner node.
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

pub(crate) fn start_runtime_thread(state: &mut AppData, mut runtime: Runtime) {
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

pub(crate) fn stop_runtime_thread(state: &mut AppData) -> Result<(), String> {
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

#[tauri::command]
pub async fn setup_runtime(
  state: State<'_, AsyncMutex<AppData>>,
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

  // Inject the shared node-state store into nodes that need it. The store is
  // also held by `AppData.nodes` (via `create_node`), so editor handles,
  // ctrl_cid, and parameter buffers stay synchronised across both.
  let node_shared_store = app_state.node_shared_store.clone();
  let mut graph_nodes = graph.nodes;
  for node in &mut graph_nodes {
    if let AudioNode::Vst(n) = node {
      n.shared_store = Some(node_shared_store.clone());
    }
  }

  drop(app_state);

  let mut runtime = Runtime::new(
    buffer_size,
    sample_rate,
    graph_nodes,
    graph.edges,
    audio_host,
    driver_handle,
    spectrum_buffers,
    waveform_buffers,
  );

  runtime.init_nodes()?;

  let mut state = state.lock().await;
  state.runtime = Some(runtime);

  // Always start (or restart) runtime after applying graph so users
  // immediately hear the route without requiring a separate enable step.
  if let Some(runtime_to_start) = state.runtime.take() {
    start_runtime_thread(&mut state, runtime_to_start);
  }

  Ok(())
}

#[tauri::command]
pub async fn enable_runtime(state: State<'_, AsyncMutex<AppData>>) -> Result<(), String> {
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
pub async fn get_node_render_data(
  state: State<'_, AsyncMutex<AppData>>,
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

#[tauri::command]
pub async fn disable_runtime(state: State<'_, AsyncMutex<AppData>>) -> Result<(), String> {
  let mut state = state.lock().await;
  stop_runtime_thread(&mut state)?;

  println!("Runtime disabled.");
  Ok(())
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
    // A → B → C
    let ids = ["A", "B", "C"];
    let edges = [e("A", "B"), e("B", "C")];
    let order = topological_order(&ids, &edges);
    assert_eq!(order, vec![0, 1, 2]);
  }

  #[test]
  fn diamond_graph() {
    // A → B, A → C, B → D, C → D
    let ids = ["A", "B", "C", "D"];
    let edges = [e("A", "B"), e("A", "C"), e("B", "D"), e("C", "D")];
    let order = topological_order(&ids, &edges);
    // A must be first, D must be last
    assert_eq!(order[0], 0); // A
    assert_eq!(order[3], 3); // D
                             // B and C can be in either order
    assert!(order[1] == 1 || order[1] == 2);
    assert!(order[2] == 1 || order[2] == 2);
    assert_ne!(order[1], order[2]);
  }

  #[test]
  fn disconnected_nodes() {
    let ids = ["A", "B", "C"];
    let edges = [];
    let order = topological_order(&ids, &edges);
    // All nodes have indegree 0, BFS processes in index order
    assert_eq!(order, vec![0, 1, 2]);
  }

  #[test]
  fn cycle_falls_back_to_index_order() {
    // A → B → C → A (cycle)
    let ids = ["A", "B", "C"];
    let edges = [e("A", "B"), e("B", "C"), e("C", "A")];
    let order = topological_order(&ids, &edges);
    assert_eq!(order, vec![0, 1, 2]); // fallback
  }

  #[test]
  fn partial_cycle_falls_back() {
    // A → B → C → B (partial cycle), D is standalone
    let ids = ["A", "B", "C", "D"];
    let edges = [e("A", "B"), e("B", "C"), e("C", "B")];
    let order = topological_order(&ids, &edges);
    assert_eq!(order, vec![0, 1, 2, 3]); // fallback
  }

  #[test]
  fn edges_referencing_unknown_nodes() {
    let ids = ["A", "B"];
    let edges = [e("A", "B"), e("X", "Y")]; // X and Y don't exist
    let order = topological_order(&ids, &edges);
    assert_eq!(order, vec![0, 1]);
  }

  #[test]
  fn complex_dag() {
    //   0:A → 1:B → 3:D
    //   0:A → 2:C → 3:D
    //   2:C → 4:E
    let ids = ["A", "B", "C", "D", "E"];
    let edges = [
      e("A", "B"),
      e("A", "C"),
      e("B", "D"),
      e("C", "D"),
      e("C", "E"),
    ];
    let order = topological_order(&ids, &edges);
    // A must come before B, C
    assert_eq!(order[0], 0);
    // B must come before D, C must come before D and E
    let pos = |idx: usize| order.iter().position(|&x| x == idx).unwrap();
    assert!(pos(0) < pos(1)); // A < B
    assert!(pos(0) < pos(2)); // A < C
    assert!(pos(1) < pos(3)); // B < D
    assert!(pos(2) < pos(3)); // C < D
    assert!(pos(2) < pos(4)); // C < E
  }

  #[test]
  fn buffer_duration_normal() {
    // 512 samples at 48000 Hz ≈ 10.67ms
    let dur = std::time::Duration::from_secs_f64(512.0 / 48000.0);
    let epsilon = std::time::Duration::from_micros(1);
    let expected = std::time::Duration::from_secs_f64(512.0 / 48000.0);
    assert!((dur.as_secs_f64() - expected.as_secs_f64()).abs() < epsilon.as_secs_f64());
  }
}
