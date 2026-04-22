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
  nodes::NodeTrait,
  runtime::{Runtime, RuntimeState},
};

/// Source node that captures audio from a specific application process via the
/// Windows WASAPI Application Loopback API (requires Windows 10 20H1 / build 19041+).
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppAudioCaptureNode {
  /// React Flow node ID.
  id: String,
  /// Target process ID obtained from the window list.
  process_id: u32,
  /// Display name of the selected window (informational only).
  window_title: String,

  #[serde(skip)]
  thread_handle: Option<std::thread::JoinHandle<()>>,
  #[serde(skip)]
  stop_flag: Option<Arc<AtomicBool>>,
  #[serde(skip)]
  ring_consumer: Option<HeapCons<f32>>,
}

impl std::fmt::Debug for AppAudioCaptureNode {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("AppAudioCaptureNode")
      .field("id", &self.id)
      .field("process_id", &self.process_id)
      .field("window_title", &self.window_title)
      .finish()
  }
}

// ---------------------------------------------------------------------------
// Windows WASAPI Application Loopback implementation
// ---------------------------------------------------------------------------

#[cfg(windows)]
fn spawn_process_loopback_thread(
  process_id: u32,
  rb_size: usize,
  stop_flag: Arc<AtomicBool>,
) -> (std::thread::JoinHandle<()>, HeapCons<f32>) {
  let rb = HeapRb::<f32>::new(rb_size);
  let (producer, consumer) = rb.split();

  let handle = std::thread::spawn(move || unsafe {
    wasapi_process_loopback_thread(process_id, producer, stop_flag);
  });

  (handle, consumer)
}

#[cfg(windows)]
unsafe fn wasapi_process_loopback_thread(
  process_id: u32,
  mut producer: HeapProd<f32>,
  stop_flag: Arc<AtomicBool>,
) {
  use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};
  let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

  if let Err(e) = wasapi_process_loopback_inner(process_id, &mut producer, &stop_flag) {
    eprintln!("AppAudioCapture process loopback error for pid {}: {}", process_id, e);
  }

  CoUninitialize();
}

/// AUDIOCLIENT_ACTIVATION_TYPE values (audioclientactivationparams.h)
#[cfg(windows)]
const AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK: u32 = 1;
/// PROCESS_LOOPBACK_MODE values
#[cfg(windows)]
const PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE: u32 = 0;
/// AUDCLNT_BUFFERFLAGS_SILENT
#[cfg(windows)]
const BUFFERFLAGS_SILENT: u32 = 0x2;

/// Manual layout mirrors `AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS` (C ABI).
#[cfg(windows)]
#[repr(C)]
struct ProcessLoopbackParams {
  target_process_id: u32,
  process_loopback_mode: u32,
}

/// Manual layout mirrors `AUDIOCLIENT_ACTIVATION_PARAMS` (C ABI).
#[cfg(windows)]
#[repr(C)]
struct AudioClientActivationParams {
  activation_type: u32,
  process_loopback_params: ProcessLoopbackParams,
}

#[cfg(windows)]
unsafe fn wasapi_process_loopback_inner(
  process_id: u32,
  producer: &mut HeapProd<f32>,
  stop_flag: &AtomicBool,
) -> Result<(), String> {
  use std::mem::ManuallyDrop;
  use windows::Win32::Media::Audio::{
    IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator, MMDeviceEnumerator,
    AUDCLNT_SHAREMODE_SHARED, eConsole, eRender,
  };
  use windows::Win32::System::Com::{
    CoCreateInstance, CoTaskMemFree, CLSCTX_ALL, CLSCTX_INPROC_SERVER,
  };
  use windows::Win32::System::Com::StructuredStorage::{
    PROPVARIANT, PROPVARIANT_0_0, PROPVARIANT_0_0_0,
  };
  use windows::Win32::System::Variant::VT_BLOB;

  // BLOB type: cbSize (u32) + pBlobData (*mut u8)
  #[repr(C)]
  struct BlobData {
    cb_size: u32,
    p_blob_data: *mut u8,
  }

  // Build activation params on the stack; must outlive the PROPVARIANT reference.
  let mut activation_params = AudioClientActivationParams {
    activation_type: AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
    process_loopback_params: ProcessLoopbackParams {
      target_process_id: process_id,
      process_loopback_mode: PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE,
    },
  };

  // Construct a PROPVARIANT with vt=VT_BLOB pointing at the activation params.
  //
  // SAFETY: We wrap in ManuallyDrop to prevent Drop from running. PROPVARIANT's
  // Drop calls PropVariantClear, which for VT_BLOB calls CoTaskMemFree on
  // pBlobData. Our pBlobData points to stack memory (activation_params), so
  // letting PropVariantClear run would free a stack address and corrupt the heap.
  let prop_variant = ManuallyDrop::new({
    let mut pv: PROPVARIANT = std::mem::zeroed();
    let inner = &mut pv.Anonymous.Anonymous;
    let inner_ref = &mut *(inner as *mut ManuallyDrop<PROPVARIANT_0_0> as *mut PROPVARIANT_0_0);
    inner_ref.vt = VT_BLOB;
    let blob_ptr = &mut inner_ref.Anonymous as *mut PROPVARIANT_0_0_0 as *mut BlobData;
    (*blob_ptr).cb_size = std::mem::size_of::<AudioClientActivationParams>() as u32;
    (*blob_ptr).p_blob_data = &mut activation_params as *mut _ as *mut u8;
    pv
  });

  // 1. Create device enumerator.
  let enumerator: IMMDeviceEnumerator =
    CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER)
      .map_err(|e| format!("CoCreateInstance(MMDeviceEnumerator) failed: {e}"))?;

  // 2. Get the default render endpoint — required as the activation target for
  //    process loopback (the actual audio is filtered by process ID via params).
  let device = enumerator
    .GetDefaultAudioEndpoint(eRender, eConsole)
    .map_err(|e| format!("GetDefaultAudioEndpoint failed: {e}"))?;

  // 3. Activate IAudioClient with process-loopback params.
  let audio_client: IAudioClient = device
    .Activate(CLSCTX_ALL, Some(&*prop_variant))
    .map_err(|e| format!("IMMDevice::Activate(process loopback) failed: {e}"))?;

  // 4. Query mix format (determines channels and sample width).
  let mix_fmt_ptr = audio_client
    .GetMixFormat()
    .map_err(|e| format!("GetMixFormat failed: {e}"))?;

  let channels = (*mix_fmt_ptr).nChannels as usize;
  let bits_per_sample = (*mix_fmt_ptr).wBitsPerSample;

  // 5. Initialize in shared mode. No explicit loopback flag — the activation
  //    params already configure process loopback routing.
  let init_result = audio_client.Initialize(
    AUDCLNT_SHAREMODE_SHARED,
    0,
    0i64,
    0i64,
    mix_fmt_ptr,
    None,
  );
  CoTaskMemFree(Some(mix_fmt_ptr as *const _));
  init_result.map_err(|e| format!("IAudioClient::Initialize failed: {e}"))?;

  // 6. Obtain capture client and start the stream.
  let capture_client: IAudioCaptureClient = audio_client
    .GetService()
    .map_err(|e| format!("GetService(IAudioCaptureClient) failed: {e}"))?;

  audio_client
    .Start()
    .map_err(|e| format!("IAudioClient::Start failed: {e}"))?;

  // 7. Capture loop.
  while !stop_flag.load(Ordering::Relaxed) {
    let mut data_ptr: *mut u8 = std::ptr::null_mut();
    let mut frames_available: u32 = 0;
    let mut flags: u32 = 0;

    let _ = capture_client.GetBuffer(&mut data_ptr, &mut frames_available, &mut flags, None, None);

    if frames_available == 0 {
      std::thread::sleep(std::time::Duration::from_millis(1));
      continue;
    }

    let sample_count = frames_available as usize * channels;

    if (flags & BUFFERFLAGS_SILENT) != 0 || data_ptr.is_null() {
      let silence = vec![0.0f32; sample_count];
      producer.push_slice(&silence);
    } else {
      match bits_per_sample {
        32 => {
          let slice = std::slice::from_raw_parts(data_ptr as *const f32, sample_count);
          producer.push_slice(slice);
        }
        16 => {
          let slice = std::slice::from_raw_parts(data_ptr as *const i16, sample_count);
          let f32s: Vec<f32> = slice.iter().map(|&s| s as f32 / i16::MAX as f32).collect();
          producer.push_slice(&f32s);
        }
        _ => {
          producer.push_slice(&vec![0.0f32; sample_count]);
        }
      }
    }

    capture_client
      .ReleaseBuffer(frames_available)
      .map_err(|e| format!("ReleaseBuffer failed: {e}"))?;
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
      // 2 channels * 8× headroom is a safe default; actual channel count is
      // determined from GetMixFormat inside the capture thread.
      let rb_size = runtime.buffer_size as usize * 2 * 8;
      let stop_flag = Arc::new(AtomicBool::new(false));
      let (handle, consumer) =
        spawn_process_loopback_thread(self.process_id, rb_size, stop_flag.clone());

      self.stop_flag = Some(stop_flag);
      self.thread_handle = Some(handle);
      self.ring_consumer = Some(consumer);
      Ok(())
    }
    #[cfg(not(windows))]
    {
      let _ = runtime;
      Err("AppAudioCapture requires Windows".to_string())
    }
  }

  fn dispose(&mut self, _runtime: &Runtime) -> Result<(), String> {
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
      process_id: 1234,
      window_title: "Test Window".to_string(),
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
    assert!(node.ring_consumer.is_none());
  }

  #[test]
  fn test_serialization_skips_runtime_fields() {
    let node = make_node();
    let json = serde_json::to_value(&node).unwrap();
    assert_eq!(json["id"], "test-node");
    assert_eq!(json["processId"], 1234);
    assert_eq!(json["windowTitle"], "Test Window");
    assert!(json.get("threadHandle").is_none());
    assert!(json.get("stopFlag").is_none());
    assert!(json.get("ringConsumer").is_none());
  }

  #[test]
  fn test_stop_flag_set_on_dispose() {
    let mut node = make_node();
    let flag = Arc::new(AtomicBool::new(false));
    node.stop_flag = Some(flag.clone());

    if let Some(f) = node.stop_flag.take() {
      f.store(true, Ordering::Relaxed);
    }
    node.thread_handle.take();
    node.ring_consumer.take();

    assert!(flag.load(Ordering::Relaxed));
  }
}
