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

    let mut nodes = std::mem::take(&mut self.nodes);

    for node in nodes.iter_mut() {
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
