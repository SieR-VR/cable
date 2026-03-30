use std::collections::BTreeMap;

use cpal::{
  traits::{DeviceTrait, HostTrait},
  BufferSize, StreamConfig,
};
use serde::{Deserialize, Serialize};

use crate::{
  nodes::NodeTrait,
  runtime::{Runtime, RuntimeState},
  AudioDevice,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AudioInputDeviceNode {
  id: String,
  device: AudioDevice,
}

impl NodeTrait for AudioInputDeviceNode {
  fn init(&mut self, runtime: &Runtime) -> Result<(), String> {
    println!(
      "Initializing audio input device: {} ({})",
      self.device.readable_name, self.id
    );

    let device = runtime
      .audio_host
      .input_devices()
      .unwrap()
      .find(|d| d.id().unwrap().to_string() == self.id)
      .ok_or_else(|| format!("Audio input device not found: {}", self.id))?;

    let config = StreamConfig {
      channels: self.device.channels,
      sample_rate: self.device.frequency,
      buffer_size: BufferSize::Fixed(runtime.buffer_size),
    };

    device
      .build_input_stream(
        &config,
        move |data: &[f32], _| {},
        move |err| {
          eprintln!("Audio input stream error: {}", err);
        },
        None,
      )
      .map_err(|e| format!("Failed to build audio input stream: {}", e))?;

    Ok(())
  }

  fn process(
    &self,
    runtime: &Runtime,
    state: &RuntimeState,
  ) -> Result<BTreeMap<String, Vec<f32>>, String> {
    println!(
      "Processing audio input device: {} ({})",
      self.device.readable_name, self.id
    );

    Ok(BTreeMap::new())
  }
}
