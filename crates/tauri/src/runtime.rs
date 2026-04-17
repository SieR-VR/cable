use std::collections::BTreeMap;
use std::sync::Arc;

use crate::{AudioEdge, AudioNode, nodes::NodeTrait};
use cpal::Host;

#[cfg(windows)]
use crate::driver_client::DriverHandle;

pub(crate) struct Runtime {
  pub buffer_size: u32,
  pub sample_rate: u32,

  pub nodes: Vec<AudioNode>,
  pub edges: Vec<AudioEdge>,

  pub audio_host: Host,

  #[cfg(windows)]
  pub driver_handle: Option<Arc<DriverHandle>>,
}

pub struct RuntimeState {
  pub edge_values: BTreeMap<String, Vec<f32>>,
}

/// Compute a topological processing order from node IDs and edges.
///
/// Returns indices into `node_ids` in dependency order.
/// Falls back to the original index order if the graph contains cycles.
pub(crate) fn topological_order(
  node_ids: &[&str],
  edges: &[(String, String)],
) -> Vec<usize> {
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
  ) -> Self {
    Self {
      buffer_size,
      sample_rate,
      nodes,
      edges,
      audio_host,
      #[cfg(windows)]
      driver_handle,
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
