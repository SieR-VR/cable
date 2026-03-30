use std::collections::BTreeMap;

use crate::{nodes::NodeTrait, AudioEdge, AudioNode};
use cpal::Host;

pub(crate) struct Runtime {
  pub buffer_size: u32,

  pub nodes: Vec<AudioNode>,
  pub edges: Vec<AudioEdge>,

  pub audio_host: Host,
}

pub struct RuntimeState {
  edge_values: BTreeMap<String, Vec<f32>>,
}

impl Runtime {
  pub fn new(
    buffer_size: u32,
    nodes: Vec<AudioNode>,
    edges: Vec<AudioEdge>,
    audio_host: Host,
  ) -> Self {
    Self {
      buffer_size,
      nodes,
      edges,
      audio_host,
    }
  }

  pub fn process(&self) -> Result<(), String> {
    let mut state = RuntimeState {
      edge_values: BTreeMap::new(),
    };

    let runtime_nodes: Vec<&dyn NodeTrait> = self
      .nodes
      .iter()
      .map(|node| match node {
        AudioNode::AudioInputDevice(n) => n as &dyn NodeTrait,
        AudioNode::AudioOutputDevice(n) => n as &dyn NodeTrait,
      })
      .collect();

    for node in runtime_nodes {
      let node_output = node.process(self, &state)?;
      for (edge_id, values) in node_output {
        state.edge_values.insert(edge_id, values);
      }
    }

    Ok(())
  }
}
