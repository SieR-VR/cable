use std::collections::BTreeMap;

use crate::runtime::{Runtime, RuntimeState};

pub mod audio_input_device;
pub mod audio_output_device;
pub mod virtual_audio_input;
pub mod virtual_audio_output;
pub mod spectrum_analyzer;
pub mod waveform_monitor;
pub mod app_audio_capture;
pub mod mixer;
pub mod vst_node;
pub(crate) mod vst3_com;

/// Audio data buffer passed between nodes.
///
/// `samples` is an interleaved f32 sample array.
/// Nodes such as VST that need the channel count reference this value.
#[derive(Clone, Debug)]
pub struct AudioBuffer {
  pub samples: Vec<f32>,
  pub channels: u16,
  pub sample_rate: u32,
  /// Bit depth of the original format (processing always runs as f32).
  pub bits_per_sample: u16,
}

impl AudioBuffer {
  pub fn new(samples: Vec<f32>, channels: u16, sample_rate: u32, bits_per_sample: u16) -> Self {
    Self { samples, channels, sample_rate, bits_per_sample }
  }

  /// Returns a silent buffer.
  pub fn silence(frames: usize, channels: u16, sample_rate: u32) -> Self {
    Self {
      samples: vec![0.0f32; frames * channels as usize],
      channels,
      sample_rate,
      bits_per_sample: 32,
    }
  }
}

pub(crate) trait NodeTrait {
  fn id(&self) -> &str;

  /// Called when the node is first created. Runs before a Runtime exists.
  /// Used for pre-initialization such as plugin metadata extraction. Default implementation is a no-op.
  fn create(&mut self) -> Result<(), String> {
    Ok(())
  }

  fn init(&mut self, runtime: &Runtime) -> Result<(), String>;

  fn dispose(&mut self, runtime: &Runtime) -> Result<(), String>;

  fn process(
    &mut self,
    runtime: &Runtime,
    state: &RuntimeState,
  ) -> Result<BTreeMap<String, AudioBuffer>, String>;
}
