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

impl Runtime {
  fn node_id(node: &AudioNode) -> &str {
    match node {
      AudioNode::AudioInputDevice(n) => n.id(),
      AudioNode::AudioOutputDevice(n) => n.id(),
      AudioNode::VirtualAudioInput(n) => n.id(),
      AudioNode::VirtualAudioOutput(n) => n.id(),
    }
  }

  fn topological_order(&self) -> Vec<usize> {
    let n = self.nodes.len();
    if n <= 1 {
      return (0..n).collect();
    }

    let mut index_by_id = std::collections::HashMap::<String, usize>::new();
    for (idx, node) in self.nodes.iter().enumerate() {
      index_by_id.insert(Self::node_id(node).to_string(), idx);
    }

    let mut indegree = vec![0usize; n];
    let mut outgoing = vec![Vec::<usize>::new(); n];

    for edge in &self.edges {
      let from = index_by_id.get(&edge.from).copied();
      let to = index_by_id.get(&edge.to).copied();
      if let (Some(f), Some(t)) = (from, to) {
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
    let order = self.topological_order();

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
