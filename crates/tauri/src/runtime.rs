use std::collections::BTreeMap;

use crate::{nodes::NodeTrait, AudioEdge, AudioNode};
use cpal::Host;

pub(crate) struct Runtime {
  pub buffer_size: u32,
  pub sample_rate: u32,

  pub nodes: Vec<AudioNode>,
  pub edges: Vec<AudioEdge>,

  pub audio_host: Host,
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
  ) -> Self {
    Self {
      buffer_size,
      sample_rate,
      nodes,
      edges,
      audio_host,
    }
  }

  pub fn init_nodes(&mut self) -> Result<(), String> {
    // init()에서 &Runtime이 필요하지만 &mut self에서 nodes를 빌려야 하므로,
    // 노드를 임시로 꺼낸 뒤 init 후 다시 넣는다.
    let mut nodes = std::mem::take(&mut self.nodes);
    for node in nodes.iter_mut() {
      match node {
        AudioNode::AudioInputDevice(n) => n.init(self)?,
        AudioNode::AudioOutputDevice(n) => n.init(self)?,
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
      }
    }
    self.nodes = nodes;
    Ok(())
  }

  pub fn process(&mut self) -> Result<(), String> {
    let mut state = RuntimeState {
      edge_values: BTreeMap::new(),
    };

    // 노드를 임시로 꺼내서 &mut 접근. self(Runtime)의 나머지 필드는 읽기 전용으로 참조 가능.
    let mut nodes = std::mem::take(&mut self.nodes);

    for node in nodes.iter_mut() {
      let node_output = match node {
        AudioNode::AudioInputDevice(n) => n.process(self, &state)?,
        AudioNode::AudioOutputDevice(n) => n.process(self, &state)?,
      };
      for (edge_id, values) in node_output {
        state.edge_values.insert(edge_id, values);
      }
    }

    self.nodes = nodes;
    Ok(())
  }

  /// timed sleep을 위한 한 버퍼 주기의 Duration 계산
  pub fn buffer_duration(&self) -> std::time::Duration {
    if self.sample_rate == 0 {
      return std::time::Duration::from_millis(10);
    }
    std::time::Duration::from_secs_f64(self.buffer_size as f64 / self.sample_rate as f64)
  }
}
