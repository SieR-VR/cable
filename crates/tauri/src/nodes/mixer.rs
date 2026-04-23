/// Mixer node.
///
/// Fixed two-input node: reads audio from the edge connected to "input-a"
/// and the edge connected to "input-b", sums them element-wise, clamps the
/// result to [-1.0, 1.0], and forwards the mixed samples to all downstream
/// edges.
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
    // Find the edge IDs connected to each named input handle.
    let find_samples = |handle: &str| -> Option<&Vec<f32>> {
      runtime
        .edges
        .iter()
        .find(|e| {
          e.to == self.id
            && e.to_handle.as_deref() == Some(handle)
        })
        .and_then(|e| state.edge_values.get(&e.id))
        .filter(|s| !s.is_empty())
    };

    let a = find_samples("input-a");
    let b = find_samples("input-b");

    let mixed: Vec<f32> = match (a, b) {
      (None, None) => return Ok(BTreeMap::new()),
      (Some(buf), None) | (None, Some(buf)) => buf.clone(),
      (Some(a), Some(b)) => {
        // Use the minimum length to avoid zero-padding discontinuities.
        let len = a.len().min(b.len());
        let mut buf = vec![0.0f32; len];
        for i in 0..len {
          buf[i] = (a[i] + b[i]).clamp(-1.0, 1.0);
        }
        buf
      }
    };

    let mut output = BTreeMap::new();
    for edge in &runtime.edges {
      if edge.from == self.id {
        output.insert(edge.id.clone(), mixed.clone());
      }
    }

    Ok(output)
  }
}

#[cfg(test)]
mod tests {
  fn mix(a: Option<Vec<f32>>, b: Option<Vec<f32>>) -> Vec<f32> {
    match (a.as_ref(), b.as_ref()) {
      (None, None) => Vec::new(),
      (Some(buf), None) | (None, Some(buf)) => buf.clone(),
      (Some(a), Some(b)) => {
        let len = a.len().min(b.len());
        let mut buf = vec![0.0f32; len];
        for i in 0..len {
          buf[i] = (a[i] + b[i]).clamp(-1.0, 1.0);
        }
        buf
      }
    }
  }

  #[test]
  fn test_mix_two_inputs_sums_element_wise() {
    let result = mix(Some(vec![0.2, 0.4, 0.6]), Some(vec![0.1, 0.1, 0.1]));
    let expected = [0.3f32, 0.5, 0.7];
    for (a, b) in result.iter().zip(expected.iter()) {
      assert!((a - b).abs() < 1e-5, "got {a}, expected {b}");
    }
  }

  #[test]
  fn test_clipping_clamped_to_one() {
    let result = mix(Some(vec![0.8, 0.9]), Some(vec![0.8, 0.9]));
    assert_eq!(result, vec![1.0, 1.0]);
  }

  #[test]
  fn test_empty_input_produces_empty_output() {
    let result = mix(None, None);
    assert!(result.is_empty());
  }

  #[test]
  fn test_unequal_length_inputs_min_length() {
    let result = mix(Some(vec![0.5, 0.5, 0.5]), Some(vec![0.1, 0.1]));
    assert_eq!(result.len(), 2);
    assert!((result[0] - 0.6).abs() < 1e-5);
    assert!((result[1] - 0.6).abs() < 1e-5);
  }

  #[test]
  fn test_negative_clipping() {
    let result = mix(Some(vec![-0.8, -0.9]), Some(vec![-0.8, -0.9]));
    assert_eq!(result, vec![-1.0, -1.0]);
  }

  #[test]
  fn test_single_input_a_passthrough() {
    let result = mix(Some(vec![0.1, -0.2, 0.3]), None);
    assert!((result[0] - 0.1).abs() < 1e-5);
    assert!((result[1] - (-0.2)).abs() < 1e-5);
    assert!((result[2] - 0.3).abs() < 1e-5);
  }

  #[test]
  fn test_single_input_b_passthrough() {
    let result = mix(None, Some(vec![0.1, -0.2, 0.3]));
    assert!((result[0] - 0.1).abs() < 1e-5);
    assert!((result[1] - (-0.2)).abs() < 1e-5);
    assert!((result[2] - 0.3).abs() < 1e-5);
  }
}

