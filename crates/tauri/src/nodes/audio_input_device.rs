use std::collections::BTreeMap;

use cpal::{
  BufferSize, Stream, StreamConfig,
  traits::{DeviceTrait, HostTrait, StreamTrait},
};
use ringbuf::{
  HeapCons, HeapRb,
  traits::{Consumer, Observer, Producer, Split},
};
use serde::{Deserialize, Serialize};

use crate::{
  AudioDevice,
  nodes::NodeTrait,
  runtime::{Runtime, RuntimeState},
};

#[derive(Serialize, Deserialize)]
pub(crate) struct AudioInputDeviceNode {
  id: String,
  device: AudioDevice,

  #[serde(skip)]
  stream: Option<Stream>,
  #[serde(skip)]
  ring_consumer: Option<HeapCons<f32>>,
}

impl std::fmt::Debug for AudioInputDeviceNode {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("AudioInputDeviceNode")
      .field("id", &self.id)
      .field("device", &self.device)
      .field("stream", &self.stream.as_ref().map(|_| "active"))
      .finish()
  }
}

impl NodeTrait for AudioInputDeviceNode {
  fn init(&mut self, runtime: &Runtime) -> Result<(), String> {
    println!(
      "Initializing audio input device: {} ({})",
      self.device.readable_name, self.id
    );

    let device = runtime
      .audio_host
      .input_devices()
      .map_err(|e| format!("Failed to enumerate input devices: {}", e))?
      .find(|d| {
        d.id()
          .map(|id| id.to_string() == self.device.id)
          .unwrap_or(false)
      })
      .ok_or_else(|| format!("Audio input device not found: {}", self.device.id))?;

    let config = StreamConfig {
      channels: self.device.channels,
      sample_rate: self.device.frequency,
      buffer_size: BufferSize::Fixed(runtime.buffer_size),
    };

    // 링 버퍼 생성: buffer_size * channels * 4 (여유 배수)
    let rb_size = runtime.buffer_size as usize * self.device.channels as usize * 4;
    let rb = HeapRb::<f32>::new(rb_size);
    let (mut producer, consumer) = rb.split();

    let stream = device
      .build_input_stream(
        &config,
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
          // cpal 콜백 스레드에서 lock-free로 데이터 push
          producer.push_slice(data);
        },
        move |err| {
          eprintln!("Audio input stream error: {}", err);
        },
        None,
      )
      .map_err(|e| format!("Failed to build audio input stream: {}", e))?;

    stream
      .play()
      .map_err(|e| format!("Failed to start audio input stream: {}", e))?;

    self.stream = Some(stream);
    self.ring_consumer = Some(consumer);

    println!(
      "Audio input device initialized: {} (rb_size={})",
      self.device.readable_name, rb_size
    );

    Ok(())
  }

  fn dispose(&mut self, _runtime: &Runtime) -> Result<(), String> {
    println!(
      "Disposing audio input device: {} ({})",
      self.device.readable_name, self.id
    );

    // Stream을 drop하면 cpal이 자동으로 스트림을 중지함
    self.stream.take();
    self.ring_consumer.take();

    Ok(())
  }

  fn process(
    &mut self,
    runtime: &Runtime,
    _state: &RuntimeState,
  ) -> Result<BTreeMap<String, Vec<f32>>, String> {
    let consumer = match self.ring_consumer.as_mut() {
      Some(c) => c,
      None => return Ok(BTreeMap::new()),
    };

    let available = consumer.occupied_len();
    if available == 0 {
      return Ok(BTreeMap::new());
    }

    // 링 버퍼에서 사용 가능한 데이터를 모두 읽기
    let mut buffer = vec![0.0f32; available];
    consumer.pop_slice(&mut buffer);

    // 이 노드에서 출발하는 모든 엣지에 대해 데이터를 복제하여 전달
    let mut output = BTreeMap::new();
    for edge in &runtime.edges {
      if edge.from == self.id {
        output.insert(edge.id.clone(), buffer.clone());
      }
    }

    Ok(output)
  }
}
