use std::collections::BTreeMap;

use cpal::{
  SampleFormat, Stream, StreamConfig,
  traits::{DeviceTrait, HostTrait, StreamTrait},
};
use ringbuf::{
  HeapProd, HeapRb,
  traits::{Consumer, Producer, Split},
};
use serde::{Deserialize, Serialize};

use crate::{
  AudioDevice,
  nodes::NodeTrait,
  runtime::{Runtime, RuntimeState},
};

#[derive(Serialize, Deserialize)]
pub(crate) struct AudioOutputDeviceNode {
  id: String,
  device: AudioDevice,

  #[serde(skip)]
  stream: Option<Stream>,
  #[serde(skip)]
  ring_producer: Option<HeapProd<f32>>,
  #[serde(skip)]
  debug_tick: u64,
}

impl std::fmt::Debug for AudioOutputDeviceNode {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("AudioOutputDeviceNode")
      .field("id", &self.id)
      .field("device", &self.device)
      .field("stream", &self.stream.as_ref().map(|_| "active"))
      .finish()
  }
}

impl NodeTrait for AudioOutputDeviceNode {
  fn id(&self) -> &str {
    &self.id
  }

  fn init(&mut self, runtime: &Runtime) -> Result<(), String> {
    println!(
      "Initializing audio output device: {} ({})",
      self.device.readable_name, self.id
    );

    let device = runtime
      .audio_host
      .output_devices()
      .map_err(|e| format!("Failed to enumerate output devices: {}", e))?
      .find(|d| {
        d.id()
          .map(|id| id.to_string() == self.device.id)
          .unwrap_or(false)
      })
      .ok_or_else(|| format!("Audio output device not found: {}", self.device.id))?;

    let default_cfg = device
      .default_output_config()
      .map_err(|e| format!("Failed to get default output config: {}", e))?;

    let config = StreamConfig {
      channels: default_cfg.channels(),
      sample_rate: runtime.sample_rate,
      buffer_size: cpal::BufferSize::Fixed(runtime.buffer_size),
    };

    let sample_format = default_cfg.sample_format();

    // 링 버퍼 생성: buffer_size * channels * 4 (여유 배수)
    let rb_size = runtime.buffer_size as usize * self.device.channels as usize * 16;
    let rb = HeapRb::<f32>::new(rb_size);
    let (producer, mut consumer) = rb.split();

    let stream = match sample_format {
      SampleFormat::F32 => device
        .build_output_stream(
          &config,
          move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            let read = consumer.pop_slice(data);
            for sample in &mut data[read..] {
              *sample = 0.0;
            }
          },
          move |err| {
            eprintln!("Audio output stream error: {}", err);
          },
          None,
        )
        .map_err(|e| format!("Failed to build f32 output stream: {}", e))?,
      SampleFormat::I16 => device
        .build_output_stream(
          &config,
          move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
            let mut temp = vec![0.0f32; data.len()];
            let read = consumer.pop_slice(&mut temp);
            for i in 0..read {
              let s = temp[i].clamp(-1.0, 1.0);
              data[i] = (s * i16::MAX as f32) as i16;
            }
            for sample in &mut data[read..] {
              *sample = 0;
            }
          },
          move |err| {
            eprintln!("Audio output stream error: {}", err);
          },
          None,
        )
        .map_err(|e| format!("Failed to build i16 output stream: {}", e))?,
      SampleFormat::U16 => device
        .build_output_stream(
          &config,
          move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
            let mut temp = vec![0.0f32; data.len()];
            let read = consumer.pop_slice(&mut temp);
            for i in 0..read {
              let s = temp[i].clamp(-1.0, 1.0);
              data[i] = (((s + 1.0) * 0.5) * u16::MAX as f32) as u16;
            }
            for sample in &mut data[read..] {
              *sample = u16::MAX / 2;
            }
          },
          move |err| {
            eprintln!("Audio output stream error: {}", err);
          },
          None,
        )
        .map_err(|e| format!("Failed to build u16 output stream: {}", e))?,
      _ => {
        return Err(format!(
          "Unsupported output sample format for {}: {:?}",
          self.device.readable_name, sample_format
        ));
      }
    };

    stream
      .play()
      .map_err(|e| format!("Failed to start audio output stream: {}", e))?;

    self.stream = Some(stream);
    self.ring_producer = Some(producer);

    println!(
      "Audio output device initialized: {} (rb_size={}, sample_format={:?}, cfg={}ch/{}Hz)",
      self.device.readable_name, rb_size, sample_format, config.channels, config.sample_rate
    );

    Ok(())
  }

  fn dispose(&mut self, _runtime: &Runtime) -> Result<(), String> {
    println!(
      "Disposing audio output device: {} ({})",
      self.device.readable_name, self.id
    );

    // Stream을 drop하면 cpal이 자동으로 스트림을 중지함
    self.stream.take();
    self.ring_producer.take();

    Ok(())
  }

  fn process(
    &mut self,
    runtime: &Runtime,
    state: &RuntimeState,
  ) -> Result<BTreeMap<String, Vec<f32>>, String> {
    let producer = match self.ring_producer.as_mut() {
      Some(p) => p,
      None => return Ok(BTreeMap::new()),
    };

    // 이 노드로 향하는 엣지의 데이터를 ring buffer에 push
    for edge in &runtime.edges {
      if edge.to == self.id {
        if let Some(data) = state.edge_values.get(&edge.id) {
          let pushed = producer.push_slice(data);
          self.debug_tick = self.debug_tick.wrapping_add(1);
          if self.debug_tick % 200 == 0 {
            println!(
              "AudioOutputDevice[{}] pushed {}/{} samples from edge {}{}",
              self.id,
              pushed,
              data.len(),
              edge.id,
              if pushed < data.len() {
                " [OVERFLOW: samples dropped]"
              } else {
                ""
              },
            );
          }
        }
      }
    }

    // 출력 노드이므로 하류 엣지 없음 → 빈 맵 반환
    Ok(BTreeMap::new())
  }
}
