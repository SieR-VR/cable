use std::collections::BTreeMap;

use cpal::{
  traits::{DeviceTrait, HostTrait, StreamTrait},
  BufferSize, Stream, StreamConfig,
};
use ringbuf::{
  traits::{Consumer, Producer, Split},
  HeapProd, HeapRb,
};
use serde::{Deserialize, Serialize};

use crate::{
  nodes::NodeTrait,
  runtime::{Runtime, RuntimeState},
  AudioDevice,
};

#[derive(Serialize, Deserialize)]
pub(crate) struct AudioOutputDeviceNode {
  id: String,
  device: AudioDevice,

  #[serde(skip)]
  stream: Option<Stream>,
  #[serde(skip)]
  ring_producer: Option<HeapProd<f32>>,
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

    let config = StreamConfig {
      channels: self.device.channels,
      sample_rate: cpal::SampleRate(self.device.frequency),
      buffer_size: BufferSize::Fixed(runtime.buffer_size),
    };

    // 링 버퍼 생성: buffer_size * channels * 4 (여유 배수)
    let rb_size = runtime.buffer_size as usize * self.device.channels as usize * 4;
    let rb = HeapRb::<f32>::new(rb_size);
    let (producer, mut consumer) = rb.split();

    let stream = device
      .build_output_stream(
        &config,
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
          // cpal 콜백 스레드에서 lock-free로 데이터 pop
          let read = consumer.pop_slice(data);
          // 데이터 부족 시 나머지를 silence(0.0)로 채움
          for sample in &mut data[read..] {
            *sample = 0.0;
          }
        },
        move |err| {
          eprintln!("Audio output stream error: {}", err);
        },
        None,
      )
      .map_err(|e| format!("Failed to build audio output stream: {}", e))?;

    stream
      .play()
      .map_err(|e| format!("Failed to start audio output stream: {}", e))?;

    self.stream = Some(stream);
    self.ring_producer = Some(producer);

    println!(
      "Audio output device initialized: {} (rb_size={})",
      self.device.readable_name, rb_size
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
          producer.push_slice(data);
        }
      }
    }

    // 출력 노드이므로 하류 엣지 없음 → 빈 맵 반환
    Ok(BTreeMap::new())
  }
}
