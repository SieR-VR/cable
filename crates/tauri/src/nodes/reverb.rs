/// Reverb node.
///
/// Implements a Freeverb-style stereo reverb using parallel comb filters
/// followed by series all-pass diffusers.  The wet/dry mix and room-size
/// (comb feedback coefficient) are user-configurable.
///
/// Reference: https://ccrma.stanford.edu/~jos/pasp/Freeverb.html
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
  nodes::{AudioBuffer, NodeTrait},
  runtime::{Runtime, RuntimeState},
};

// ---------------------------------------------------------------------------
// Freeverb magic numbers (tuned for 44 100 Hz; scaled for other rates).
// ---------------------------------------------------------------------------

const COMB_TUNINGS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASS_TUNINGS: [usize; 4] = [556, 441, 341, 225];

const FIXED_GAIN: f32 = 0.015;
const SCALE_WET: f32 = 3.0;
const ALLPASS_FEEDBACK: f32 = 0.5;
const STEREO_SPREAD: usize = 23;

// ---------------------------------------------------------------------------
// Lightweight comb and all-pass filter types
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct CombFilter {
  buffer: Vec<f32>,
  pos: usize,
  feedback: f32,
  damp1: f32,
  damp2: f32,
  filterstore: f32,
}

impl CombFilter {
  fn new(size: usize) -> Self {
    Self {
      buffer: vec![0.0; size],
      pos: 0,
      feedback: 0.5,
      damp1: 0.5,
      damp2: 0.5,
      filterstore: 0.0,
    }
  }

  fn process(&mut self, input: f32) -> f32 {
    let output = self.buffer[self.pos];
    self.filterstore = output * self.damp2 + self.filterstore * self.damp1;
    self.buffer[self.pos] = input + self.filterstore * self.feedback;
    self.pos = (self.pos + 1) % self.buffer.len();
    output
  }

  fn set_feedback(&mut self, v: f32) {
    self.feedback = v;
  }

  fn set_damp(&mut self, v: f32) {
    self.damp1 = v;
    self.damp2 = 1.0 - v;
  }
}

#[derive(Debug)]
struct AllpassFilter {
  buffer: Vec<f32>,
  pos: usize,
}

impl AllpassFilter {
  fn new(size: usize) -> Self {
    Self {
      buffer: vec![0.0; size],
      pos: 0,
    }
  }

  fn process(&mut self, input: f32) -> f32 {
    let buf_out = self.buffer[self.pos];
    let output = -input + buf_out;
    self.buffer[self.pos] = input + buf_out * ALLPASS_FEEDBACK;
    self.pos = (self.pos + 1) % self.buffer.len();
    output
  }
}

// ---------------------------------------------------------------------------
// Node
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReverbNode {
  /// React Flow node ID.
  id: String,
  /// Room size: controls comb-filter feedback (0.0 – 1.0; default 0.5).
  room_size: f32,
  /// Wet mix level (0.0 – 1.0; default 0.33).
  wet: f32,
  /// Damping factor for high-frequency absorption (0.0 – 1.0; default 0.5).
  damp: f32,

  #[serde(skip)]
  combs_l: Vec<CombFilter>,
  #[serde(skip)]
  combs_r: Vec<CombFilter>,
  #[serde(skip)]
  allpasses_l: Vec<AllpassFilter>,
  #[serde(skip)]
  allpasses_r: Vec<AllpassFilter>,
  #[serde(skip)]
  sample_rate: u32,
}

fn scaled(base: usize, sr: u32) -> usize {
  ((base as f64 * sr as f64 / 44100.0).round() as usize).max(1)
}

impl ReverbNode {
  fn build_filters(&mut self, sr: u32) {
    let feedback = self.room_size * 0.28 + 0.7; // map [0,1] → [0.70, 0.98]
    let damp = self.damp;

    self.combs_l = COMB_TUNINGS
      .iter()
      .map(|&t| {
        let mut c = CombFilter::new(scaled(t, sr));
        c.set_feedback(feedback);
        c.set_damp(damp);
        c
      })
      .collect();

    self.combs_r = COMB_TUNINGS
      .iter()
      .map(|&t| {
        let mut c = CombFilter::new(scaled(t + STEREO_SPREAD, sr));
        c.set_feedback(feedback);
        c.set_damp(damp);
        c
      })
      .collect();

    self.allpasses_l = ALLPASS_TUNINGS
      .iter()
      .map(|&t| AllpassFilter::new(scaled(t, sr)))
      .collect();

    self.allpasses_r = ALLPASS_TUNINGS
      .iter()
      .map(|&t| AllpassFilter::new(scaled(t + STEREO_SPREAD, sr)))
      .collect();
  }

  fn process_sample(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
    let mono = (input_l + input_r) * FIXED_GAIN;

    let mut out_l = 0.0_f32;
    let mut out_r = 0.0_f32;

    for c in &mut self.combs_l {
      out_l += c.process(mono);
    }
    for c in &mut self.combs_r {
      out_r += c.process(mono);
    }

    for ap in &mut self.allpasses_l {
      out_l = ap.process(out_l);
    }
    for ap in &mut self.allpasses_r {
      out_r = ap.process(out_r);
    }

    (out_l, out_r)
  }
}

impl NodeTrait for ReverbNode {
  fn id(&self) -> &str {
    &self.id
  }

  fn init(&mut self, runtime: &Runtime) -> Result<(), String> {
    self.sample_rate = runtime.sample_rate;
    self.build_filters(self.sample_rate);
    Ok(())
  }

  fn dispose(&mut self, _runtime: &Runtime) -> Result<(), String> {
    self.combs_l.clear();
    self.combs_r.clear();
    self.allpasses_l.clear();
    self.allpasses_r.clear();
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

    if buf.sample_rate != self.sample_rate || self.combs_l.is_empty() {
      self.sample_rate = buf.sample_rate;
      self.build_filters(self.sample_rate);
    }

    let channels = buf.channels as usize;
    let wet_scaled = self.wet * SCALE_WET;
    let dry = 1.0 - self.wet;

    let frames = buf.samples.len() / channels.max(1);
    let mut out_samples = Vec::with_capacity(buf.samples.len());

    for f in 0..frames {
      let base = f * channels;
      let l = *buf.samples.get(base).unwrap_or(&0.0);
      let r = if channels >= 2 {
        *buf.samples.get(base + 1).unwrap_or(&0.0)
      } else {
        l
      };

      let (rev_l, rev_r) = self.process_sample(l, r);

      // Write all output channels; if input was mono, duplicate both reverb outputs.
      for c in 0..channels {
        let dry_s = *buf.samples.get(base + c).unwrap_or(&0.0);
        let wet_s = if c % 2 == 0 { rev_l } else { rev_r };
        out_samples.push((dry_s * dry + wet_s * wet_scaled).clamp(-1.0, 1.0));
      }
    }

    let out_buf = AudioBuffer::new(
      out_samples,
      buf.channels,
      buf.sample_rate,
      buf.bits_per_sample,
    );
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
  use super::scaled;

  #[test]
  fn test_scaled_identity_at_44100() {
    assert_eq!(scaled(1116, 44100), 1116);
  }

  #[test]
  fn test_scaled_at_48k() {
    let v = scaled(1116, 48000);
    // 1116 * 48000 / 44100 ≈ 1215
    assert!(v > 1100 && v < 1300);
  }

  #[test]
  fn test_scaled_minimum_one() {
    // Very small base with low sample rate should still return at least 1.
    assert_eq!(scaled(1, 8000), 1);
  }
}
