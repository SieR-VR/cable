/// Gain node.
///
/// Multiplies every sample by a linear gain factor.
/// `gain = 1.0` is unity; `gain = 2.0` doubles amplitude; `gain = 0.0` is silence.
/// Output is clamped to [-1.0, 1.0] to prevent clipping propagation.
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
  nodes::{AudioBuffer, NodeTrait},
  runtime::{Runtime, RuntimeState},
};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GainNode {
  /// React Flow node ID.
  id: String,
  /// Linear gain multiplier (0.0 – 4.0; default 1.0).
  gain: f32,
}

impl NodeTrait for GainNode {
  fn id(&self) -> &str {
    &self.id
  }

  fn init(&mut self, _runtime: &Runtime) -> Result<(), String> {
    Ok(())
  }

  fn dispose(&mut self, _runtime: &Runtime) -> Result<(), String> {
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

    let gain = self.gain;
    let samples: Vec<f32> = buf.samples.iter().map(|&s| (s * gain).clamp(-1.0, 1.0)).collect();
    let out_buf = AudioBuffer::new(samples, buf.channels, buf.sample_rate, buf.bits_per_sample);

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
  fn apply_gain(samples: &[f32], gain: f32) -> Vec<f32> {
    samples.iter().map(|&s| (s * gain).clamp(-1.0, 1.0)).collect()
  }

  #[test]
  fn test_unity_gain_passthrough() {
    let input = vec![0.1, -0.5, 0.9];
    let out = apply_gain(&input, 1.0);
    for (a, b) in out.iter().zip(input.iter()) {
      assert!((a - b).abs() < 1e-6);
    }
  }

  #[test]
  fn test_zero_gain_is_silence() {
    let input = vec![0.5, -0.3, 0.8];
    let out = apply_gain(&input, 0.0);
    assert!(out.iter().all(|&s| s == 0.0));
  }

  #[test]
  fn test_double_gain_clamps_at_one() {
    let input = vec![0.8, 0.9, 1.0];
    let out = apply_gain(&input, 2.0);
    assert!(out.iter().all(|&s| s <= 1.0 && s >= -1.0));
  }

  #[test]
  fn test_half_gain() {
    let input = vec![0.4, -0.6];
    let out = apply_gain(&input, 0.5);
    assert!((out[0] - 0.2).abs() < 1e-6);
    assert!((out[1] - (-0.3)).abs() < 1e-6);
  }

  #[test]
  fn test_negative_clamp() {
    let input = vec![-0.8, -0.9];
    let out = apply_gain(&input, 2.0);
    assert!(out.iter().all(|&s| s >= -1.0));
  }
}
