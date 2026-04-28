/// Delay node.
///
/// Delays the incoming audio signal by a configurable number of milliseconds
/// using a circular buffer. Latency equals `delay_ms` exactly; no feedback.
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
  nodes::{AudioBuffer, NodeTrait},
  runtime::{Runtime, RuntimeState},
};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DelayNode {
  /// React Flow node ID.
  id: String,
  /// Delay time in milliseconds (0 – 2000 ms; default 250 ms).
  delay_ms: f32,

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
  #[serde(skip)]
  bits_per_sample: u16,
}

/// Compute the required circular-buffer length (in interleaved samples).
fn compute_buf_size(delay_ms: f32, sample_rate: u32, channels: u16) -> usize {
  let delay_frames = ((delay_ms.max(0.0) / 1000.0) * sample_rate as f32) as usize;
  (delay_frames * channels as usize).max(1)
}

impl NodeTrait for DelayNode {
  fn id(&self) -> &str {
    &self.id
  }

  fn init(&mut self, runtime: &Runtime) -> Result<(), String> {
    let sr = runtime.sample_rate;
    // Initialise with a plausible default channel count; will be corrected on
    // the first `process()` call once real audio arrives.
    let ch = 2u16;
    let buf_size = compute_buf_size(self.delay_ms, sr, ch);
    self.buffer = vec![0.0_f32; buf_size];
    self.write_pos = 0;
    self.delay_samples = buf_size;
    self.sample_rate = sr;
    self.channels = ch;
    self.bits_per_sample = 32;
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

    // Reinitialise the circular buffer whenever the audio format changes.
    if sr != self.sample_rate || ch != self.channels {
      let buf_size = compute_buf_size(self.delay_ms, sr, ch);
      self.buffer = vec![0.0_f32; buf_size];
      self.write_pos = 0;
      self.delay_samples = buf_size;
      self.sample_rate = sr;
      self.channels = ch;
      self.bits_per_sample = buf.bits_per_sample;
    }

    let n = buf.samples.len();
    let mut out_samples = vec![0.0_f32; n];
    let buf_len = self.buffer.len();

    if buf_len <= 1 {
      // Zero-delay passthrough.
      out_samples.copy_from_slice(&buf.samples);
    } else {
      for (i, &sample) in buf.samples.iter().enumerate() {
        out_samples[i] = self.buffer[self.write_pos];
        self.buffer[self.write_pos] = sample;
        self.write_pos = (self.write_pos + 1) % buf_len;
      }
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
  fn test_buf_size_250ms_48k_stereo() {
    // 250 ms * 48000 Hz * 2 ch = 24000 samples
    assert_eq!(compute_buf_size(250.0, 48000, 2), 24000);
  }

  #[test]
  fn test_buf_size_zero_ms_returns_one() {
    assert_eq!(compute_buf_size(0.0, 48000, 2), 1);
  }

  #[test]
  fn test_buf_size_negative_ms_returns_one() {
    assert_eq!(compute_buf_size(-10.0, 48000, 2), 1);
  }

  /// Simple standalone delay simulation without a Runtime.
  fn run_delay(input: &[f32], buf_len: usize) -> Vec<f32> {
    let mut buffer = vec![0.0_f32; buf_len];
    let mut pos = 0;
    let mut out = vec![0.0_f32; input.len()];
    for (i, &s) in input.iter().enumerate() {
      out[i] = buffer[pos];
      buffer[pos] = s;
      pos = (pos + 1) % buf_len;
    }
    out
  }

  #[test]
  fn test_delay_shifts_signal() {
    // buf_len == 2 means a 2-sample delay.
    let input = vec![1.0, 2.0, 3.0, 4.0];
    let out = run_delay(&input, 2);
    // First two outputs should be silence (0.0), then shifted signal.
    assert_eq!(out, vec![0.0, 0.0, 1.0, 2.0]);
  }

  #[test]
  fn test_single_sample_delay() {
    let input = vec![0.5, 0.8, 0.2];
    let out = run_delay(&input, 1);
    // buf_len=1 → pos always 0, value is immediately overwritten: each output
    // is the *previous* input because we read before write.
    assert_eq!(out, vec![0.0, 0.5, 0.8]);
  }
}
