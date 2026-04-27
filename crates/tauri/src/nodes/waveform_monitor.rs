/// Waveform Monitor node.
///
/// Passthrough node: reads audio from upstream edges, keeps a rolling window
/// of the most recent `window_size` samples, stores it in a shared buffer
/// accessible via the `get_waveform_data` Tauri command, and forwards the
/// original samples to downstream edges unchanged.
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::{
  nodes::{AudioBuffer, NodeTrait},
  runtime::{Runtime, RuntimeState},
};

const DEFAULT_WINDOW_SIZE: usize = 2048;

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WaveformMonitorNode {
  /// Node ID (matches ReactFlow node id).
  id: String,
  /// Number of samples to keep in the rolling window. Must be ≥ 1.
  window_size: usize,

  /// Shared rolling buffer of the latest `window_size` samples.
  /// Shared via Arc with AppData.waveform_buffers so the Tauri command
  /// can read the latest frame without blocking the audio thread.
  #[serde(skip)]
  waveform_out: Option<Arc<Mutex<Vec<f32>>>>,
}

impl NodeTrait for WaveformMonitorNode {
  fn id(&self) -> &str {
    &self.id
  }

  fn init(&mut self, runtime: &Runtime) -> Result<(), String> {
    println!("Initializing WaveformMonitor node: {}", self.id);

    if self.window_size < 1 {
      self.window_size = DEFAULT_WINDOW_SIZE;
    }

    let arc = runtime
      .waveform_buffers
      .get(&self.id)
      .ok_or_else(|| {
        format!(
          "WaveformMonitor node '{}': no waveform buffer found in runtime",
          self.id
        )
      })?
      .clone();

    self.waveform_out = Some(arc);

    println!(
      "WaveformMonitor node '{}' initialized (window_size={})",
      self.id, self.window_size
    );
    Ok(())
  }

  fn dispose(&mut self, _runtime: &Runtime) -> Result<(), String> {
    println!("Disposing WaveformMonitor node: {}", self.id);
    self.waveform_out = None;
    Ok(())
  }

  fn process(
    &mut self,
    runtime: &Runtime,
    state: &RuntimeState,
  ) -> Result<BTreeMap<String, AudioBuffer>, String> {
    let mut incoming_samples: Vec<f32> = Vec::new();
    let mut incoming_buf: Option<AudioBuffer> = None;
    for edge in &runtime.edges {
      if edge.to == self.id {
        if let Some(buf) = state.edge_values.get(&edge.id) {
          incoming_samples.extend_from_slice(&buf.samples);
          if incoming_buf.is_none() {
            incoming_buf = Some(buf.clone());
          }
        }
      }
    }

    // Update the rolling window: append new samples, keep only the last window_size.
    if let Some(wf_buf) = &self.waveform_out {
      if let Ok(mut guard) = wf_buf.lock() {
        guard.extend_from_slice(&incoming_samples);
        let len = guard.len();
        if len > self.window_size {
          guard.drain(..len - self.window_size);
        }
      }
    }

    // Passthrough: forward original AudioBuffer to all outgoing edges.
    let mut output = BTreeMap::new();
    if let Some(buf) = incoming_buf {
      if !buf.samples.is_empty() {
        let passthrough = AudioBuffer::new(
          incoming_samples,
          buf.channels,
          buf.sample_rate,
          buf.bits_per_sample,
        );
        for edge in &runtime.edges {
          if edge.from == self.id {
            output.insert(edge.id.clone(), passthrough.clone());
          }
        }
      }
    }

    Ok(output)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn make_node(window_size: usize) -> WaveformMonitorNode {
    WaveformMonitorNode {
      id: "test-node".to_string(),
      window_size,
      waveform_out: None,
    }
  }

  fn init_node(node: &mut WaveformMonitorNode) {
    node.waveform_out = Some(Arc::new(Mutex::new(Vec::new())));
  }

  #[test]
  fn test_rolling_window_fills() {
    let window_size = 512;
    let mut node = make_node(window_size);
    init_node(&mut node);

    let buf = node.waveform_out.as_ref().unwrap();
    buf.lock().unwrap().extend(vec![0.5f32; window_size / 2]);

    // Simulate a process call with incoming samples
    let incoming = vec![0.25f32; window_size / 2];
    {
      let mut guard = buf.lock().unwrap();
      guard.extend_from_slice(&incoming);
      let len = guard.len();
      if len > window_size {
        guard.drain(..len - window_size);
      }
    }

    let result = buf.lock().unwrap().clone();
    assert_eq!(result.len(), window_size);
  }

  #[test]
  fn test_rolling_window_truncates_to_window_size() {
    let window_size = 256;
    let mut node = make_node(window_size);
    init_node(&mut node);

    let buf = node.waveform_out.as_ref().unwrap();

    // Feed more than window_size samples
    let incoming = vec![1.0f32; window_size * 2];
    {
      let mut guard = buf.lock().unwrap();
      guard.extend_from_slice(&incoming);
      let len = guard.len();
      if len > window_size {
        guard.drain(..len - window_size);
      }
    }

    let result = buf.lock().unwrap().clone();
    assert_eq!(result.len(), window_size);
  }

  #[test]
  fn test_samples_preserved_within_window() {
    let window_size = 8;
    let mut node = make_node(window_size);
    init_node(&mut node);

    let buf = node.waveform_out.as_ref().unwrap();

    // First batch: [1,2,3,4]
    let batch1 = vec![1.0, 2.0, 3.0, 4.0];
    {
      let mut guard = buf.lock().unwrap();
      guard.extend_from_slice(&batch1);
    }

    // Second batch: [5,6,7,8] — total 8, exactly window_size
    let batch2 = vec![5.0, 6.0, 7.0, 8.0];
    {
      let mut guard = buf.lock().unwrap();
      guard.extend_from_slice(&batch2);
      let len = guard.len();
      if len > window_size {
        guard.drain(..len - window_size);
      }
    }

    let result = buf.lock().unwrap().clone();
    assert_eq!(result, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
  }

  #[test]
  fn test_old_samples_evicted() {
    let window_size = 4;
    let mut node = make_node(window_size);
    init_node(&mut node);

    let buf = node.waveform_out.as_ref().unwrap();

    // Fill with old samples
    buf.lock().unwrap().extend(vec![0.0f32; window_size]);

    // New batch pushes out old ones
    let new_samples = vec![1.0, 2.0, 3.0, 4.0];
    {
      let mut guard = buf.lock().unwrap();
      guard.extend_from_slice(&new_samples);
      let len = guard.len();
      if len > window_size {
        guard.drain(..len - window_size);
      }
    }

    let result = buf.lock().unwrap().clone();
    assert_eq!(result, vec![1.0, 2.0, 3.0, 4.0]);
  }
}
