/// Compressor node.
///
/// Feed-forward dynamic-range compressor operating on the peak envelope of
/// the incoming signal.  Gain is computed in the log domain and applied
/// sample-by-sample:
///
///   1. Estimate the peak envelope with per-sample attack / release smoothing.
///   2. Convert envelope to dB.
///   3. Apply the knee-less gain-computer: if above threshold, attenuate
///      according to the ratio.
///   4. Apply optional make-up gain.
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
  nodes::{AudioBuffer, NodeTrait},
  runtime::{Runtime, RuntimeState},
};

/// Smallest envelope value to prevent log(0).
const ENV_FLOOR: f32 = 1e-6;

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CompressorNode {
  /// React Flow node ID.
  id: String,
  /// Threshold in dB (−60 to 0).  Default: −12.
  threshold_db: f32,
  /// Compression ratio (1:1 = no compression, 20:1 ≈ limiting).  Default: 4.
  ratio: f32,
  /// Attack time in milliseconds.  Default: 5.
  attack_ms: f32,
  /// Release time in milliseconds.  Default: 50.
  release_ms: f32,
  /// Make-up gain in dB.  Default: 0.
  make_up_db: f32,

  #[serde(skip)]
  /// Current peak-envelope level (linear, initialised to 0).
  envelope: f32,
  #[serde(skip)]
  attack_coeff: f32,
  #[serde(skip)]
  release_coeff: f32,
  #[serde(skip)]
  sample_rate: u32,
}

/// Convert milliseconds to a one-pole smoothing coefficient.
fn ms_to_coeff(ms: f32, sample_rate: u32) -> f32 {
  if ms <= 0.0 {
    return 1.0;
  }
  // exp(-1 / (ms * 0.001 * sample_rate))
  (-1.0_f32 / (ms * 0.001 * sample_rate as f32)).exp()
}

impl NodeTrait for CompressorNode {
  fn id(&self) -> &str {
    &self.id
  }

  fn init(&mut self, runtime: &Runtime) -> Result<(), String> {
    self.sample_rate = runtime.sample_rate;
    self.attack_coeff = ms_to_coeff(self.attack_ms, self.sample_rate);
    self.release_coeff = ms_to_coeff(self.release_ms, self.sample_rate);
    self.envelope = 0.0;
    Ok(())
  }

  fn dispose(&mut self, _runtime: &Runtime) -> Result<(), String> {
    self.envelope = 0.0;
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

    // Recompute coefficients if the sample rate changed mid-graph.
    if buf.sample_rate != self.sample_rate {
      self.sample_rate = buf.sample_rate;
      self.attack_coeff = ms_to_coeff(self.attack_ms, self.sample_rate);
      self.release_coeff = ms_to_coeff(self.release_ms, self.sample_rate);
    }

    let threshold_linear = db_to_linear(self.threshold_db);
    let makeup_linear = db_to_linear(self.make_up_db);
    let ratio = self.ratio.max(1.0);

    let mut out_samples = Vec::with_capacity(buf.samples.len());

    for &sample in &buf.samples {
      let abs_sample = sample.abs();

      // Peak-envelope follower.
      if abs_sample > self.envelope {
        self.envelope = self.attack_coeff * self.envelope + (1.0 - self.attack_coeff) * abs_sample;
      } else {
        self.envelope =
          self.release_coeff * self.envelope + (1.0 - self.release_coeff) * abs_sample;
      }

      let env = self.envelope.max(ENV_FLOOR);

      // Gain computation in the log domain.
      let gain_linear = if env > threshold_linear {
        // How much the envelope exceeds the threshold (linear ratio).
        // gain_db = threshold_db + (env_db - threshold_db) / ratio - env_db
        //         = (threshold_db - env_db) * (1 - 1/ratio)
        let env_db = linear_to_db(env);
        let gain_db = (self.threshold_db - env_db) * (1.0 - 1.0 / ratio);
        db_to_linear(gain_db)
      } else {
        1.0
      };

      out_samples.push((sample * gain_linear * makeup_linear).clamp(-1.0, 1.0));
    }

    let out_buf =
      AudioBuffer::new(out_samples, buf.channels, buf.sample_rate, buf.bits_per_sample);

    let mut output = BTreeMap::new();
    for edge in &runtime.edges {
      if edge.from == self.id {
        output.insert(edge.id.clone(), out_buf.clone());
      }
    }
    Ok(output)
  }
}

#[inline]
fn db_to_linear(db: f32) -> f32 {
  10.0_f32.powf(db / 20.0)
}

#[inline]
fn linear_to_db(linear: f32) -> f32 {
  20.0 * linear.abs().max(ENV_FLOOR).log10()
}

#[cfg(test)]
mod tests {
  use super::{db_to_linear, linear_to_db, ms_to_coeff};

  #[test]
  fn test_db_to_linear_zero() {
    assert!((db_to_linear(0.0) - 1.0).abs() < 1e-5);
  }

  #[test]
  fn test_db_to_linear_minus_6() {
    // -6 dB ≈ 0.5
    assert!((db_to_linear(-6.0206) - 0.5).abs() < 1e-3);
  }

  #[test]
  fn test_linear_to_db_one() {
    assert!(linear_to_db(1.0).abs() < 1e-5);
  }

  #[test]
  fn test_roundtrip() {
    let db = -12.0_f32;
    assert!((linear_to_db(db_to_linear(db)) - db).abs() < 1e-4);
  }

  #[test]
  fn test_ms_to_coeff_zero_ms_returns_one() {
    assert!((ms_to_coeff(0.0, 48000) - 1.0).abs() < 1e-6);
  }

  #[test]
  fn test_ms_to_coeff_range() {
    let c = ms_to_coeff(10.0, 48000);
    assert!(c > 0.0 && c < 1.0);
  }
}
