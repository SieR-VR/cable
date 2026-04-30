/// Channel Split node.
///
/// Splits an interleaved multi-channel audio buffer into per-channel mono outputs.
/// Each outgoing edge is matched by its `from_handle` field:
///
///   - `"ch-0"` → channel 0 (left for stereo)
///   - `"ch-1"` → channel 1 (right for stereo)
///   - `"ch-N"` → channel N
///
/// An edge without a `from_handle` (or with an unrecognised handle) receives the
/// full interleaved buffer unchanged, so existing graphs remain unaffected.
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
  nodes::{AudioBuffer, NodeTrait},
  runtime::{Runtime, RuntimeState},
};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChannelSplitNode {
  /// React Flow node ID.
  id: String,
}

/// Extract a single channel from an interleaved buffer.
fn extract_channel(samples: &[f32], channel: usize, total_channels: usize) -> Vec<f32> {
  if total_channels == 0 {
    return Vec::new();
  }
  samples
    .chunks_exact(total_channels)
    .map(|frame| *frame.get(channel).unwrap_or(&0.0))
    .collect()
}

impl NodeTrait for ChannelSplitNode {
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

    let total_ch = buf.channels as usize;
    let mut output = BTreeMap::new();

    for edge in &runtime.edges {
      if edge.from != self.id {
        continue;
      }

      let out_buf = match edge.from_handle.as_deref() {
        Some(handle) if handle.starts_with("ch-") => {
          if let Ok(ch_idx) = handle["ch-".len()..].parse::<usize>() {
            if ch_idx < total_ch {
              let mono_samples = extract_channel(&buf.samples, ch_idx, total_ch);
              AudioBuffer::new(mono_samples, 1, buf.sample_rate, buf.bits_per_sample)
            } else {
              // Channel index out of range — emit silence.
              AudioBuffer::silence(buf.samples.len() / total_ch.max(1), 1, buf.sample_rate)
            }
          } else {
            buf.clone()
          }
        }
        // No handle or unrecognised handle → full buffer passthrough.
        _ => buf.clone(),
      };

      output.insert(edge.id.clone(), out_buf);
    }

    Ok(output)
  }
}

#[cfg(test)]
mod tests {
  use super::extract_channel;

  #[test]
  fn test_extract_left_channel() {
    // Interleaved stereo: L0 R0 L1 R1 ...
    let samples = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
    let left = extract_channel(&samples, 0, 2);
    assert_eq!(left, vec![1.0, 3.0, 5.0]);
  }

  #[test]
  fn test_extract_right_channel() {
    let samples = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
    let right = extract_channel(&samples, 1, 2);
    assert_eq!(right, vec![2.0, 4.0, 6.0]);
  }

  #[test]
  fn test_extract_mono_passthrough() {
    let samples = vec![0.1, 0.2, 0.3];
    let ch0 = extract_channel(&samples, 0, 1);
    assert_eq!(ch0, vec![0.1, 0.2, 0.3]);
  }

  #[test]
  fn test_extract_out_of_range_returns_zeros() {
    let samples = vec![1.0, 2.0];
    let ch = extract_channel(&samples, 5, 2);
    // chunks_exact with total_channels=2 → 1 frame; channel 5 doesn't exist → 0.0
    assert_eq!(ch, vec![0.0]);
  }

  #[test]
  fn test_extract_quad_channel() {
    // 4-channel interleaved
    let samples = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
    let ch2 = extract_channel(&samples, 2, 4);
    assert_eq!(ch2, vec![3.0, 7.0]);
  }
}
