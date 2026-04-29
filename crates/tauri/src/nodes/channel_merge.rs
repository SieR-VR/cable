/// Channel Merge node.
///
/// Interleaves N mono per-channel inputs into a single multi-channel output.
/// Each incoming edge is matched by its `to_handle` field:
///
///   - `"ch-0"` → channel 0 (left for stereo)
///   - `"ch-1"` → channel 1 (right for stereo)
///   - `"ch-N"` → channel N
///
/// Channels without a connected edge are filled with silence so the output
/// buffer always has exactly `input_count` interleaved channels.
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
  nodes::{AudioBuffer, NodeTrait},
  runtime::{Runtime, RuntimeState},
};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChannelMergeNode {
  /// React Flow node ID.
  id: String,
  /// Number of mono input channels to merge (2, 4, 6, or 8).
  input_count: u16,
}

/// Interleave `count` mono sample slices into a single buffer.
///
/// Each element of `channels` is a mono sample slice (one f32 per frame).
/// Missing channels (None) are filled with silence.
/// All slices must have the same length; `frames` is taken from the first
/// non-None slice (or 0 if all are None).
fn interleave(channels: &[Option<&[f32]>]) -> Vec<f32> {
  let count = channels.len();
  if count == 0 {
    return Vec::new();
  }
  let frames = channels.iter().filter_map(|c| *c).next().map(|s| s.len()).unwrap_or(0);
  if frames == 0 {
    return Vec::new();
  }

  let mut out = vec![0.0f32; frames * count];
  for (ch_idx, maybe_slice) in channels.iter().enumerate() {
    if let Some(slice) = maybe_slice {
      for frame in 0..frames {
        out[frame * count + ch_idx] = slice.get(frame).copied().unwrap_or(0.0);
      }
    }
  }
  out
}

impl NodeTrait for ChannelMergeNode {
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
    let count = self.input_count as usize;
    if count == 0 {
      return Ok(BTreeMap::new());
    }

    // Collect per-channel mono buffers from incoming edges.
    let mut channel_bufs: Vec<Option<&AudioBuffer>> = vec![None; count];
    let mut sample_rate = 48000u32;
    let mut bits_per_sample = 32u16;
    let mut any_input = false;

    for edge in &runtime.edges {
      if edge.to != self.id {
        continue;
      }
      let Some(handle) = edge.to_handle.as_deref() else {
        continue;
      };
      let Some(ch_str) = handle.strip_prefix("ch-") else {
        continue;
      };
      let Ok(ch_idx) = ch_str.parse::<usize>() else {
        continue;
      };
      if ch_idx >= count {
        continue;
      }
      if let Some(buf) = state.edge_values.get(&edge.id) {
        if !any_input {
          sample_rate = buf.sample_rate;
          bits_per_sample = buf.bits_per_sample;
          any_input = true;
        }
        channel_bufs[ch_idx] = Some(buf);
      }
    }

    if !any_input {
      return Ok(BTreeMap::new());
    }

    let mono_slices: Vec<Option<&[f32]>> = channel_bufs
      .iter()
      .map(|b| b.map(|buf| buf.samples.as_slice()))
      .collect();

    let interleaved = interleave(&mono_slices);

    let frames = interleaved.len() / count;
    let out_buf = AudioBuffer::new(interleaved, count as u16, sample_rate, bits_per_sample);
    let _ = frames;

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
  use super::interleave;

  #[test]
  fn test_interleave_stereo() {
    let left = [1.0f32, 2.0, 3.0];
    let right = [4.0f32, 5.0, 6.0];
    let result = interleave(&[Some(&left), Some(&right)]);
    assert_eq!(result, vec![1.0, 4.0, 2.0, 5.0, 3.0, 6.0]);
  }

  #[test]
  fn test_interleave_missing_channel_is_silence() {
    let left = [1.0f32, 2.0];
    let result = interleave(&[Some(&left), None]);
    assert_eq!(result, vec![1.0, 0.0, 2.0, 0.0]);
  }

  #[test]
  fn test_interleave_quad() {
    let a = [1.0f32];
    let b = [2.0f32];
    let c = [3.0f32];
    let d = [4.0f32];
    let result = interleave(&[Some(&a), Some(&b), Some(&c), Some(&d)]);
    assert_eq!(result, vec![1.0, 2.0, 3.0, 4.0]);
  }

  #[test]
  fn test_interleave_all_none_returns_empty() {
    let nones: &[Option<&[f32]>] = &[None, None];
    let result = interleave(nones);
    assert!(result.is_empty());
  }

  #[test]
  fn test_interleave_empty_input() {
    let empty: &[Option<&[f32]>] = &[];
    let result = interleave(empty);
    assert!(result.is_empty());
  }
}
