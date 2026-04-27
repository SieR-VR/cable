use std::collections::BTreeMap;

use crate::runtime::{Runtime, RuntimeState};

pub mod audio_input_device;
pub mod audio_output_device;
pub mod virtual_audio_input;
pub mod virtual_audio_output;
pub mod spectrum_analyzer;
pub mod waveform_monitor;
pub mod app_audio_capture;
pub mod mixer;
pub mod vst_node;
pub(crate) mod vst3_com;

/// 노드 간에 전달되는 오디오 데이터 버퍼.
///
/// `samples`는 인터리브드 f32 샘플이다.
/// VST 등 채널 수가 필요한 노드는 이 값을 참조한다.
#[derive(Clone, Debug)]
pub struct AudioBuffer {
  pub samples: Vec<f32>,
  pub channels: u16,
  pub sample_rate: u32,
  /// 원본 포맷의 비트 심도 (처리는 항상 f32로 수행).
  pub bits_per_sample: u16,
}

impl AudioBuffer {
  pub fn new(samples: Vec<f32>, channels: u16, sample_rate: u32, bits_per_sample: u16) -> Self {
    Self { samples, channels, sample_rate, bits_per_sample }
  }

  /// silence 버퍼 생성.
  pub fn silence(frames: usize, channels: u16, sample_rate: u32) -> Self {
    Self {
      samples: vec![0.0f32; frames * channels as usize],
      channels,
      sample_rate,
      bits_per_sample: 32,
    }
  }
}

pub(crate) trait NodeTrait {
  fn id(&self) -> &str;

  /// 노드가 처음 생성될 때 호출된다. Runtime이 없는 상태에서도 실행된다.
  /// 플러그인 메타데이터 추출 등 사전 초기화에 사용. 기본 구현은 no-op.
  fn create(&mut self) -> Result<(), String> {
    Ok(())
  }

  fn init(&mut self, runtime: &Runtime) -> Result<(), String>;

  fn dispose(&mut self, runtime: &Runtime) -> Result<(), String>;

  fn process(
    &mut self,
    runtime: &Runtime,
    state: &RuntimeState,
  ) -> Result<BTreeMap<String, AudioBuffer>, String>;
}
