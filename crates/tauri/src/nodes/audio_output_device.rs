use std::collections::BTreeMap;

use cpal::Device;
use serde::{Deserialize, Serialize};

use crate::{
  nodes::NodeTrait,
  runtime::{Runtime, RuntimeState},
  AudioDevice,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AudioOutputDeviceNode {
  id: String,
  device: AudioDevice,
}

impl NodeTrait for AudioOutputDeviceNode {
  fn process(
    &self,
    runtime: &Runtime,
    _state: &RuntimeState,
  ) -> Result<BTreeMap<String, Vec<f32>>, String> {
    println!(
      "Processing audio output device: {} ({})",
      self.device.readable_name, self.id
    );

    Ok(BTreeMap::new())
  }
}
