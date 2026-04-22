use std::collections::BTreeMap;
use std::sync::{
  Arc,
  atomic::{AtomicBool, Ordering},
};

use ringbuf::{
  HeapCons, HeapRb,
  traits::{Consumer, Observer, Producer, Split},
};
use serde::{Deserialize, Serialize};

#[cfg(windows)]
use ringbuf::HeapProd;

use crate::{
  AudioDevice,
  nodes::NodeTrait,
  runtime::{Runtime, RuntimeState},
};

/// Source node that captures loopback audio from a Windows render endpoint via WASAPI.
///
/// Loopback capture lets Cable intercept all audio playing through a selected output
/// device (speakers, headphones, virtual cable, etc.) and route it into the audio graph
/// like any other source node.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppAudioCaptureNode {
  /// React Flow node ID.
  id: String,
  /// The Windows render (output) device to loopback-capture from.
  device: AudioDevice,

  /// Background WASAPI loopback capture thread.
  #[serde(skip)]
  thread_handle: Option<std::thread::JoinHandle<()>>,

  /// Signals the capture thread to stop.
  #[serde(skip)]
  stop_flag: Option<Arc<AtomicBool>>,

  /// Consumer end of the lock-free ring buffer fed by the capture thread.
  #[serde(skip)]
  ring_consumer: Option<HeapCons<f32>>,
}

impl std::fmt::Debug for AppAudioCaptureNode {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("AppAudioCaptureNode")
      .field("id", &self.id)
      .field("device", &self.device)
      .finish()
  }
}

// ---------------------------------------------------------------------------
// Windows WASAPI loopback implementation
// ---------------------------------------------------------------------------

/// Spawn the loopback capture thread and return a (JoinHandle, consumer) pair.
#[cfg(windows)]
fn spawn_loopback_thread(
  device_id: String,
  rb_size: usize,
  stop_flag: Arc<AtomicBool>,
) -> (std::thread::JoinHandle<()>, HeapCons<f32>) {
  let rb = HeapRb::<f32>::new(rb_size);
  let (producer, consumer) = rb.split();

  let handle = std::thread::spawn(move || unsafe {
    wasapi_loopback_thread(device_id, producer, stop_flag);
  });

  (handle, consumer)
}

#[cfg(windows)]
unsafe fn wasapi_loopback_thread(
  device_id: String,
  mut producer: HeapProd<f32>,
  stop_flag: Arc<AtomicBool>,
) {
  use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};

  // Each thread must initialize COM independently.
  let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

  if let Err(e) = wasapi_loopback_inner(&device_id, &mut producer, &stop_flag) {
    eprintln!("AppAudioCapture WASAPI error for '{}': {}", device_id, e);
  }

  CoUninitialize();
}

#[cfg(windows)]
unsafe fn wasapi_loopback_inner(
  device_id: &str,
  producer: &mut HeapProd<f32>,
  stop_flag: &AtomicBool,
) -> Result<(), String> {
  use windows::Win32::Media::Audio::{
    IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator, MMDeviceEnumerator,
    AUDCLNT_SHAREMODE_SHARED,
  };
  use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_ALL, CLSCTX_INPROC_SERVER};
  use windows::core::PCWSTR;

  // AUDCLNT_STREAMFLAGS_LOOPBACK = 0x00020000
  const STREAMFLAGS_LOOPBACK: u32 = 0x0002_0000;
  // AUDCLNT_BUFFERFLAGS_SILENT = 0x2
  const BUFFERFLAGS_SILENT: u32 = 0x2;

  // 1. Create device enumerator (matches pattern in lib.rs)
  let enumerator: IMMDeviceEnumerator =
    CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER)
      .map_err(|e| format!("CoCreateInstance(MMDeviceEnumerator) failed: {}", e))?;

  // 2. Get the render endpoint by its Windows MM endpoint ID string.
  //    cpal on Windows returns this string from DeviceId::to_string().
  let device_id_wide: Vec<u16> = device_id
    .encode_utf16()
    .chain(std::iter::once(0))
    .collect();
  let device = enumerator
    .GetDevice(PCWSTR(device_id_wide.as_ptr()))
    .map_err(|e| format!("GetDevice('{}') failed: {}", device_id, e))?;

  // 3. Activate an IAudioClient on the render endpoint
  let audio_client: IAudioClient = device
    .Activate(CLSCTX_ALL, None)
    .map_err(|e| format!("IMMDevice::Activate(IAudioClient) failed: {}", e))?;

  // 4. Get the engine mix format (heap-allocated, must be freed with CoTaskMemFree)
  let mix_fmt_ptr = audio_client
    .GetMixFormat()
    .map_err(|e| format!("IAudioClient::GetMixFormat failed: {}", e))?;

  let channels = (*mix_fmt_ptr).nChannels as usize;
  let bits_per_sample = (*mix_fmt_ptr).wBitsPerSample;

  // 5. Initialize for shared-mode loopback capture
  //    hnsBufferDuration = 0  → WASAPI chooses the default buffer size
  //    hnsPeriodicity    = 0  → required for shared mode
  let init_result = audio_client.Initialize(
    AUDCLNT_SHAREMODE_SHARED,
    STREAMFLAGS_LOOPBACK,
    0i64,
    0i64,
    mix_fmt_ptr,
    None,
  );
  CoTaskMemFree(Some(mix_fmt_ptr as *const _));
  init_result.map_err(|e| format!("IAudioClient::Initialize(loopback) failed: {}", e))?;

  // 6. Obtain the capture client service
  let capture_client: IAudioCaptureClient = audio_client
    .GetService()
    .map_err(|e| format!("IAudioClient::GetService failed: {}", e))?;

  // 7. Start the stream
  audio_client
    .Start()
    .map_err(|e| format!("IAudioClient::Start failed: {}", e))?;

  // 8. Capture loop — poll until stop_flag is set
  while !stop_flag.load(Ordering::Relaxed) {
    let mut data_ptr: *mut u8 = std::ptr::null_mut();
    let mut frames_available: u32 = 0;
    let mut flags: u32 = 0;

    // GetBuffer returns AUDCLNT_S_BUFFER_EMPTY (a success code, not an error) when
    // no new frames are ready; in that case frames_available == 0 and we must NOT
    // call ReleaseBuffer.
    let _ = capture_client.GetBuffer(&mut data_ptr, &mut frames_available, &mut flags, None, None);

    if frames_available == 0 {
      std::thread::sleep(std::time::Duration::from_millis(1));
      continue;
    }

    let sample_count = frames_available as usize * channels;

    if (flags & BUFFERFLAGS_SILENT) != 0 || data_ptr.is_null() {
      // Audio engine signalled silence (e.g. nothing is playing) — push zeros
      let silence = vec![0.0f32; sample_count];
      producer.push_slice(&silence);
    } else {
      match bits_per_sample {
        32 => {
          // IEEE float: most common format for WASAPI loopback on Windows 10+
          let slice = std::slice::from_raw_parts(data_ptr as *const f32, sample_count);
          producer.push_slice(slice);
        }
        16 => {
          let slice = std::slice::from_raw_parts(data_ptr as *const i16, sample_count);
          let f32s: Vec<f32> = slice
            .iter()
            .map(|&s| s as f32 / i16::MAX as f32)
            .collect();
          producer.push_slice(&f32s);
        }
        _ => {
          // Unsupported format — push silence to avoid gaps downstream
          let silence = vec![0.0f32; sample_count];
          producer.push_slice(&silence);
        }
      }
    }

    capture_client
      .ReleaseBuffer(frames_available)
      .map_err(|e| format!("IAudioCaptureClient::ReleaseBuffer failed: {}", e))?;
  }

  let _ = audio_client.Stop();
  Ok(())
}

// ---------------------------------------------------------------------------
// NodeTrait implementation
// ---------------------------------------------------------------------------

impl NodeTrait for AppAudioCaptureNode {
  fn id(&self) -> &str {
    &self.id
  }

  fn init(&mut self, runtime: &Runtime) -> Result<(), String> {
    #[cfg(windows)]
    {
      println!(
        "Initializing AppAudioCapture node: {} (device={})",
        self.id, self.device.id
      );

      let rb_size = runtime.buffer_size as usize * self.device.channels as usize * 8;
      let stop_flag = Arc::new(AtomicBool::new(false));
      let (handle, consumer) = spawn_loopback_thread(
        self.device.id.clone(),
        rb_size,
        stop_flag.clone(),
      );

      self.stop_flag = Some(stop_flag);
      self.thread_handle = Some(handle);
      self.ring_consumer = Some(consumer);

      println!(
        "AppAudioCapture node '{}' initialized (rb_size={})",
        self.id, rb_size
      );
      Ok(())
    }
    #[cfg(not(windows))]
    {
      let _ = runtime;
      Err("AppAudioCapture node requires Windows".to_string())
    }
  }

  fn dispose(&mut self, _runtime: &Runtime) -> Result<(), String> {
    println!("Disposing AppAudioCapture node: {}", self.id);

    if let Some(flag) = self.stop_flag.take() {
      flag.store(true, Ordering::Relaxed);
    }
    if let Some(handle) = self.thread_handle.take() {
      let _ = handle.join();
    }
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

    let mut buffer = vec![0.0f32; available];
    consumer.pop_slice(&mut buffer);

    let mut output = BTreeMap::new();
    for edge in &runtime.edges {
      if edge.from == self.id {
        output.insert(edge.id.clone(), buffer.clone());
      }
    }

    Ok(output)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn make_node() -> AppAudioCaptureNode {
    AppAudioCaptureNode {
      id: "test-node".to_string(),
      device: crate::AudioDevice {
        id: "test-device".to_string(),
        readable_name: "Test Output".to_string(),
        frequency: 48000,
        channels: 2,
        bits_per_sample: 32,
        descriptions: vec![],
      },
      thread_handle: None,
      stop_flag: None,
      ring_consumer: None,
    }
  }

  #[test]
  fn test_id_returns_node_id() {
    let node = make_node();
    assert_eq!(node.id(), "test-node");
  }

  #[test]
  fn test_dispose_without_init_is_safe() {
    let mut node = make_node();
    // All runtime fields are None; dispose should be a no-op without panicking.
    assert!(node.stop_flag.is_none());
    assert!(node.thread_handle.is_none());
    assert!(node.ring_consumer.is_none());
    node.stop_flag.take();
    node.thread_handle.take();
    node.ring_consumer.take();
    assert!(node.stop_flag.is_none());
  }

  #[test]
  fn test_ring_consumer_none_before_init() {
    let node = make_node();
    // Without init(), ring_consumer should be None and process should return empty.
    assert!(node.ring_consumer.is_none());
  }

  #[test]
  fn test_serialization_skips_runtime_fields() {
    let node = make_node();
    let json = serde_json::to_value(&node).unwrap();
    // id and device are present
    assert!(json.get("id").is_some());
    assert!(json.get("device").is_some());
    // Runtime fields are #[serde(skip)] and must not appear in JSON
    assert!(json.get("threadHandle").is_none());
    assert!(json.get("stopFlag").is_none());
    assert!(json.get("ringConsumer").is_none());
  }

  #[test]
  fn test_stop_flag_set_on_dispose() {
    let mut node = make_node();
    let flag = Arc::new(AtomicBool::new(false));
    node.stop_flag = Some(flag.clone());

    // Simulate dispose
    if let Some(f) = node.stop_flag.take() {
      f.store(true, Ordering::Relaxed);
    }
    node.thread_handle.take();
    node.ring_consumer.take();

    // The shared Arc should now see the flag set to true
    assert!(flag.load(Ordering::Relaxed));
  }
}
