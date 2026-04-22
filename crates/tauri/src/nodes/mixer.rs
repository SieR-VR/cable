/// Mixer node.
///
/// Passthrough node: reads audio from all upstream edges, mixes them by
/// summing element-wise, clamps the result to [-1.0, 1.0], and forwards
/// the mixed samples to all downstream edges.
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
  nodes::NodeTrait,
  runtime::{Runtime, RuntimeState},
};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MixerNode {
  /// Node ID (matches ReactFlow node id).
  id: String,
}

impl NodeTrait for MixerNode {
  fn id(&self) -> &str {
    &self.id
  }

  fn init(&mut self, _runtime: &Runtime) -> Result<(), String> {
    println!("Initializing Mixer node: {}", self.id);
    Ok(())
  }

  fn dispose(&mut self, _runtime: &Runtime) -> Result<(), String> {
    println!("Disposing Mixer node: {}", self.id);
    Ok(())
  }

  fn process(
    &mut self,
    runtime: &Runtime,
    state: &RuntimeState,
  ) -> Result<BTreeMap<String, Vec<f32>>, String> {
    // Collect all incoming sample buffers.
    let mut inputs: Vec<&Vec<f32>> = Vec::new();
    for edge in &runtime.edges {
      if edge.to == self.id {
        if let Some(samples) = state.edge_values.get(&edge.id) {
          if !samples.is_empty() {
            inputs.push(samples);
          }
        }
      }
    }

    let mixed: Vec<f32> = if inputs.is_empty() {
      Vec::new()
    } else {
      let len = inputs.iter().map(|s| s.len()).max().unwrap_or(0);
      let mut buf = vec![0.0f32; len];
      for input in &inputs {
        for (i, &s) in input.iter().enumerate() {
          buf[i] += s;
        }
      }
      // Clamp to prevent clipping.
      buf.iter_mut().for_each(|s| *s = s.clamp(-1.0, 1.0));
      buf
    };

    let mut output = BTreeMap::new();
    if !mixed.is_empty() {
      for edge in &runtime.edges {
        if edge.from == self.id {
          output.insert(edge.id.clone(), mixed.clone());
        }
      }
    }

    Ok(output)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn mix(inputs: Vec<Vec<f32>>) -> Vec<f32> {
    if inputs.is_empty() {
      return Vec::new();
    }
    let len = inputs.iter().map(|s| s.len()).max().unwrap_or(0);
    let mut buf = vec![0.0f32; len];
    for input in &inputs {
      for (i, &s) in input.iter().enumerate() {
        buf[i] += s;
      }
    }
    buf.iter_mut().for_each(|s| *s = s.clamp(-1.0, 1.0));
    buf
  }

  #[test]
  fn test_mix_two_inputs_sums_element_wise() {
    let result = mix(vec![vec![0.2, 0.4, 0.6], vec![0.1, 0.1, 0.1]]);
    let expected = [0.3f32, 0.5, 0.7];
    for (a, b) in result.iter().zip(expected.iter()) {
      assert!((a - b).abs() < 1e-5, "got {a}, expected {b}");
    }
  }

  #[test]
  fn test_clipping_clamped_to_one() {
    let result = mix(vec![vec![0.8, 0.9], vec![0.8, 0.9]]);
    assert_eq!(result, vec![1.0, 1.0]);
  }

  #[test]
  fn test_empty_input_produces_empty_output() {
    let result = mix(vec![]);
    assert!(result.is_empty());
  }

  #[test]
  fn test_unequal_length_inputs_padded() {
    let result = mix(vec![vec![0.5, 0.5, 0.5], vec![0.1, 0.1]]);
    assert_eq!(result.len(), 3);
    assert!((result[0] - 0.6).abs() < 1e-5);
    assert!((result[1] - 0.6).abs() < 1e-5);
    assert!((result[2] - 0.5).abs() < 1e-5);
  }

  #[test]
  fn test_negative_clipping() {
    let result = mix(vec![vec![-0.8, -0.9], vec![-0.8, -0.9]]);
    assert_eq!(result, vec![-1.0, -1.0]);
  }

  #[test]
  fn test_single_input_passthrough() {
    let result = mix(vec![vec![0.1, -0.2, 0.3]]);
    assert!((result[0] - 0.1).abs() < 1e-5);
    assert!((result[1] - (-0.2)).abs() < 1e-5);
    assert!((result[2] - 0.3).abs() < 1e-5);
  }
}
