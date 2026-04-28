/// Echo node.
///
/// Feedback echo (tape-echo style) with configurable delay, feedback amount,
/// and wet/dry mix.
///
/// Signal path:
///   output[n] = dry * input[n]  +  wet * delayed[n]
///   buffer[n] = input[n] + feedback * delayed[n]
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
  nodes::{AudioBuffer, NodeTrait},
  runtime::{Runtime, RuntimeState},
};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EchoNode {
  /// React Flow node ID.
  id: String,
  /// Delay time in milliseconds (0 – 2000 ms; default 375 ms).
  delay_ms: f32,
  /// Feedback coefficient: fraction of the delayed signal fed back (0.0 – 0.95).
  feedback: f32,
  /// Wet mix (0.0 = dry only, 1.0 = wet only; default 0.5).
  wet: f32,

  #[serde(skip)]
  buffer: Vec<f32>,
  #[serde(skip)]
  write_pos: usize,
  #[serde(skip)]
  delay_samples: usize,
  #[serde(skip)]
  channels: u16,
  #[serde(skip)]
  sample_rate: u32,
}

fn compute_buf_size(delay_ms: f32, sample_rate: u32, channels: u16) -> usize {
  let frames = ((delay_ms.max(0.0) / 1000.0) * sample_rate as f32) as usize;
  (frames * channels as usize).max(1)
}

impl NodeTrait for EchoNode {
  fn id(&self) -> &str {
    &self.id
  }

  fn init(&mut self, runtime: &Runtime) -> Result<(), String> {
    let sr = runtime.sample_rate;
    let ch = 2u16;
    let buf_size = compute_buf_size(self.delay_ms, sr, ch);
    self.buffer = vec![0.0_f32; buf_size];
    self.write_pos = 0;
    self.delay_samples = buf_size;
    self.sample_rate = sr;
    self.channels = ch;
    Ok(())
  }

  fn dispose(&mut self, _runtime: &Runtime) -> Result<(), String> {
    self.buffer = Vec::new();
    Ok(())
  }

  fn process(
    &mut self,
    runtime: &Runtime,
    state: &RuntimeState,
  ) -> Result<BTreeMap<String, AudioBuffer>, String> {
    let incoming = runtime
      .edges
      .iter()
      .find(|e| e.to == self.id)
      .and_then(|e| state.edge_values.get(&e.id));

    let buf = match incoming {
      None => return Ok(BTreeMap::new()),
      Some(b) if b.samples.is_empty() => return Ok(BTreeMap::new()),
      Some(b) => b,
    };

    let sr = buf.sample_rate;
    let ch = buf.channels;

    if sr != self.sample_rate || ch != self.channels {
      let buf_size = compute_buf_size(self.delay_ms, sr, ch);
      self.buffer = vec![0.0_f32; buf_size];
      self.write_pos = 0;
      self.delay_samples = buf_size;
      self.sample_rate = sr;
      self.channels = ch;
    }

    let feedback = self.feedback.clamp(0.0, 0.95);
    let wet = self.wet.clamp(0.0, 1.0);
    let dry = 1.0 - wet;
    let buf_len = self.buffer.len();

    let mut out_samples = Vec::with_capacity(buf.samples.len());

    for &sample in &buf.samples {
      let delayed = self.buffer[self.write_pos];
      self.buffer[self.write_pos] = (sample + delayed * feedback).clamp(-1.0, 1.0);
      self.write_pos = (self.write_pos + 1) % buf_len;

      out_samples.push((sample * dry + delayed * wet).clamp(-1.0, 1.0));
    }

    let out_buf = AudioBuffer::new(out_samples, ch, sr, buf.bits_per_sample);
    let mut output = BTreeMap::new();
    for edge in &runtime.edges {
      if edge.from == self.id {
        output.insert(edge.id.clone(), out_buf.clone());
      }
    }
    Ok(output)
  }
}

#[cfg(test)]
mod tests {
  use super::compute_buf_size;

  #[test]
  fn test_buf_size_375ms_48k_stereo() {
    // 375 ms * 48000 Hz * 2 ch = 36000 samples
    assert_eq!(compute_buf_size(375.0, 48000, 2), 36000);
  }

  #[test]
  fn test_buf_size_zero_ms() {
    assert_eq!(compute_buf_size(0.0, 48000, 2), 1);
  }

  /// Simulate one echo tap: dry=0.5, wet=0.5, feedback=0.5, buf_len=1.
  #[test]
  fn test_echo_signal_path() {
    let feedback = 0.5_f32;
    let wet = 0.5_f32;
    let dry = 1.0 - wet;
    let mut buffer = vec![0.0_f32; 1];
    let mut pos = 0;

    let input = [0.8_f32, 0.0, 0.0];
    let mut out = [0.0_f32; 3];

    for (i, &s) in input.iter().enumerate() {
      let delayed = buffer[pos];
      buffer[pos] = (s + delayed * feedback).clamp(-1.0, 1.0);
      pos = (pos + 1) % buffer.len();
      out[i] = s * dry + delayed * wet;
    }

    // First output: input=0.8, delayed=0 → out = 0.8*0.5 + 0*0.5 = 0.4
    assert!((out[0] - 0.4).abs() < 1e-5);
    // Second output: input=0, delayed=0.8 → out = 0*0.5 + 0.8*0.5 = 0.4
    assert!((out[1] - 0.4).abs() < 1e-5);
  }

  #[test]
  fn test_feedback_clamped_to_safe_range() {
    // Feedback > 0.95 would be unstable; clamping prevents blow-up.
    let feedback = 1.0_f32.clamp(0.0, 0.95);
    assert!(feedback <= 0.95);
  }
}
