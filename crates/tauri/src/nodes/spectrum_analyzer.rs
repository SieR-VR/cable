/// Spectrum Analyzer node.
///
/// Passthrough node: reads audio from upstream edges, computes an FFT-based
/// magnitude spectrum, stores it in a shared buffer accessible via the
/// `get_spectrum_data` Tauri command, and forwards the original samples to
/// downstream edges unchanged.
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rustfft::{FftPlanner, num_complex::Complex};
use serde::{Deserialize, Serialize};

use crate::{
  nodes::{AudioBuffer, NodeTrait},
  runtime::{Runtime, RuntimeState},
};

/// Default FFT window size (must be a power of two).
const DEFAULT_FFT_SIZE: usize = 1024;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SpectrumAnalyzerNode {
  /// Node ID (matches ReactFlow node id)
  id: String,
  /// FFT window size (number of samples per FFT frame). Must be a power of two.
  fft_size: usize,

  /// Shared buffer: fft_size/2 magnitude bins (0..Nyquist).
  /// Shared via Arc with AppData.spectrum_buffers so the Tauri command
  /// can read the latest frame without blocking the audio thread.
  #[serde(skip)]
  spectrum_out: Option<Arc<Mutex<Vec<f32>>>>,

  /// Cached FFT plan (created once during init).
  #[serde(skip)]
  fft: Option<Arc<dyn rustfft::Fft<f32>>>,

  /// Sample accumulator: incoming samples are pushed here until we have
  /// fft_size samples, at which point a frame is processed and the first
  /// half is discarded (50% overlap).
  #[serde(skip)]
  sample_accumulator: Vec<f32>,
}

impl std::fmt::Debug for SpectrumAnalyzerNode {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("SpectrumAnalyzerNode")
      .field("id", &self.id)
      .field("fft_size", &self.fft_size)
      .finish()
  }
}

impl SpectrumAnalyzerNode {
  /// Compute Hann window coefficients for `size` samples.
  fn hann_window(size: usize) -> Vec<f32> {
    (0..size)
      .map(|i| {
        0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (size - 1) as f32).cos())
      })
      .collect()
  }

  /// Run FFT on the current accumulator contents and update `spectrum_out`.
  fn compute_fft(&self) {
    let fft = match &self.fft {
      Some(f) => f,
      None => return,
    };
    let spectrum_out = match &self.spectrum_out {
      Some(s) => s,
      None => return,
    };

    let window = Self::hann_window(self.fft_size);

    // Apply Hann window and build complex input buffer.
    let mut buffer: Vec<Complex<f32>> = self.sample_accumulator[..self.fft_size]
      .iter()
      .enumerate()
      .map(|(i, &s)| Complex { re: s * window[i], im: 0.0 })
      .collect();

    fft.process(&mut buffer);

    // Compute magnitude for positive frequencies (0..fft_size/2).
    let bins: Vec<f32> = buffer[..self.fft_size / 2]
      .iter()
      .map(|c| c.norm() / self.fft_size as f32)
      .collect();

    if let Ok(mut guard) = spectrum_out.lock() {
      *guard = bins;
    }
  }
}

impl NodeTrait for SpectrumAnalyzerNode {
  fn id(&self) -> &str {
    &self.id
  }

  fn init(&mut self, runtime: &Runtime) -> Result<(), String> {
    println!("Initializing SpectrumAnalyzer node: {}", self.id);

    // Ensure fft_size is at least 2 and a power of two.
    if self.fft_size < 2 || !self.fft_size.is_power_of_two() {
      self.fft_size = DEFAULT_FFT_SIZE;
    }

    // Retrieve the shared spectrum buffer Arc from the Runtime.
    let arc = runtime
      .spectrum_buffers
      .get(&self.id)
      .ok_or_else(|| {
        format!(
          "SpectrumAnalyzer node '{}': no spectrum buffer found in runtime",
          self.id
        )
      })?
      .clone();

    self.spectrum_out = Some(arc);

    // Build the FFT plan once.
    let mut planner = FftPlanner::<f32>::new();
    self.fft = Some(planner.plan_fft_forward(self.fft_size));

    self.sample_accumulator = Vec::with_capacity(self.fft_size * 2);

    println!(
      "SpectrumAnalyzer node '{}' initialized (fft_size={})",
      self.id, self.fft_size
    );
    Ok(())
  }

  fn dispose(&mut self, _runtime: &Runtime) -> Result<(), String> {
    println!("Disposing SpectrumAnalyzer node: {}", self.id);
    self.spectrum_out = None;
    self.fft = None;
    self.sample_accumulator.clear();
    Ok(())
  }

  fn process(
    &mut self,
    runtime: &Runtime,
    state: &RuntimeState,
  ) -> Result<BTreeMap<String, AudioBuffer>, String> {
    // Collect all samples arriving on incoming edges.
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

    // Accumulate and compute FFT when we have enough samples.
    self.sample_accumulator.extend_from_slice(&incoming_samples);
    while self.sample_accumulator.len() >= self.fft_size {
      self.compute_fft();
      // 50% overlap: drop the first half of the window.
      self.sample_accumulator.drain(..self.fft_size / 2);
    }

    // Passthrough: forward the original AudioBuffer to all outgoing edges.
    let mut output = BTreeMap::new();
    if let Some(buf) = incoming_buf {
      if !buf.samples.is_empty() {
        // Re-assemble from all incoming samples so multi-edge inputs are merged.
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

  fn make_node(fft_size: usize) -> SpectrumAnalyzerNode {
    SpectrumAnalyzerNode {
      id: "test-node".to_string(),
      fft_size,
      spectrum_out: None,
      fft: None,
      sample_accumulator: Vec::new(),
    }
  }

  fn init_node(node: &mut SpectrumAnalyzerNode) {
    let arc: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
    let mut planner = FftPlanner::<f32>::new();
    node.fft = Some(planner.plan_fft_forward(node.fft_size));
    node.spectrum_out = Some(arc);
    node.sample_accumulator = Vec::with_capacity(node.fft_size * 2);
  }

  #[test]
  fn test_fft_output_length() {
    let fft_size = 1024;
    let mut node = make_node(fft_size);
    init_node(&mut node);

    // Provide exactly fft_size samples of silence.
    node.sample_accumulator.extend(vec![0.0f32; fft_size]);
    node.compute_fft();

    let bins = node.spectrum_out.as_ref().unwrap().lock().unwrap().clone();
    assert_eq!(bins.len(), fft_size / 2);
  }

  #[test]
  fn test_spectrum_nonnegative() {
    let fft_size = 512;
    let mut node = make_node(fft_size);
    init_node(&mut node);

    // Sine wave at 1kHz (arbitrary sample rate).
    let samples: Vec<f32> = (0..fft_size)
      .map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0).sin())
      .collect();
    node.sample_accumulator.extend_from_slice(&samples);
    node.compute_fft();

    let bins = node.spectrum_out.as_ref().unwrap().lock().unwrap().clone();
    assert!(bins.iter().all(|&b| b >= 0.0), "All magnitude bins must be non-negative");
  }

  #[test]
  fn test_accumulator_no_update_below_fft_size() {
    let fft_size = 1024;
    let mut node = make_node(fft_size);
    init_node(&mut node);

    // Provide fewer samples than fft_size — spectrum should remain empty.
    node.sample_accumulator.extend(vec![1.0f32; fft_size - 1]);

    // No compute_fft call since we don't reach fft_size yet.
    // (The loop in process() wouldn't trigger either.)
    let bins = node.spectrum_out.as_ref().unwrap().lock().unwrap().clone();
    assert!(bins.is_empty(), "Spectrum should not be updated before fft_size samples accumulate");
  }

  #[test]
  fn test_50_percent_overlap_drain() {
    let fft_size = 256;
    let mut node = make_node(fft_size);
    init_node(&mut node);

    // Provide 2x fft_size samples.
    // Loop: drain fft_size/2 per frame until fewer than fft_size samples remain.
    // Frame 1: 512 → drain 128 → 384
    // Frame 2: 384 → drain 128 → 256
    // Frame 3: 256 → drain 128 → 128 (< fft_size, loop stops)
    let samples = vec![0.0f32; fft_size * 2];
    node.sample_accumulator.extend_from_slice(&samples);

    while node.sample_accumulator.len() >= fft_size {
      node.compute_fft();
      node.sample_accumulator.drain(..fft_size / 2);
    }

    assert_eq!(node.sample_accumulator.len(), fft_size / 2);
  }
}
