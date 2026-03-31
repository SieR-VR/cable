use std::sync::{
  Arc,
  atomic::{AtomicBool, Ordering},
};

use cpal::traits::{DeviceTrait, HostTrait};
use serde::{Deserialize, Serialize};
use tauri::{Builder, State, async_runtime::Mutex};

#[cfg(windows)]
pub mod driver_client;
pub mod nodes;
mod runtime;

use nodes::audio_input_device::AudioInputDeviceNode;
use nodes::audio_output_device::AudioOutputDeviceNode;
use nodes::virtual_audio_input::VirtualAudioInputNode;
use nodes::virtual_audio_output::VirtualAudioOutputNode;

struct AppData {
  runtime: Option<runtime::Runtime>,
  runtime_thread: Option<std::thread::JoinHandle<()>>,
  runtime_running: Option<Arc<AtomicBool>>,
  #[cfg(windows)]
  driver_handle: Option<Arc<driver_client::DriverHandle>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AudioDevice {
  id: String,
  readable_name: String,

  frequency: u32,
  channels: u16,
  bits_per_sample: usize,

  descriptions: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AudioGraph {
  nodes: Vec<AudioNode>,
  edges: Vec<AudioEdge>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub(crate) enum AudioNode {
  AudioInputDevice(AudioInputDeviceNode),
  AudioOutputDevice(AudioOutputDeviceNode),
  VirtualAudioInput(VirtualAudioInputNode),
  VirtualAudioOutput(VirtualAudioOutputNode),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AudioEdge {
  id: String,

  from: String,
  to: String,

  frequency: Option<u32>,
  channels: Option<u16>,
  bits_per_sample: Option<usize>,
}

#[tauri::command]
fn get_audio_hosts() -> Vec<String> {
  let available_hosts = cpal::available_hosts();
  println!("Available audio hosts: {:?}", available_hosts);

  available_hosts.iter().map(|h| format!("{:?}", h)).collect()
}

#[tauri::command]
fn get_audio_devices(host: String) -> (Vec<AudioDevice>, Vec<AudioDevice>) {
  let host_id = match cpal::available_hosts()
    .into_iter()
    .find(|h| format!("{:?}", h) == host)
  {
    Some(h) => h,
    None => return (Vec::new(), Vec::new()),
  };

  let host = cpal::host_from_id(host_id).unwrap();
  let input_devices = host
    .input_devices()
    .unwrap()
    .map(|d| {
      let description = d.description().unwrap();
      let interface_type = d.default_input_config().unwrap();

      AudioDevice {
        id: d.id().unwrap().to_string(),
        readable_name: description.name().to_string(),
        descriptions: description.extended().to_vec(),
        frequency: interface_type.sample_rate(),
        channels: interface_type.channels(),
        bits_per_sample: interface_type.sample_format().sample_size(),
      }
    })
    .collect();
  let output_devices = host
    .output_devices()
    .unwrap()
    .map(|d| {
      let description = d.description().unwrap();
      let interface_type = d.default_output_config().unwrap();

      AudioDevice {
        id: d.id().unwrap().to_string(),
        readable_name: description.name().to_string(),
        descriptions: description.extended().to_vec(),
        frequency: interface_type.sample_rate(),
        channels: interface_type.channels(),
        bits_per_sample: interface_type.sample_format().sample_size(),
      }
    })
    .collect();

  println!(
    "Input devices: {:?}, Output devices: {:?}",
    input_devices, output_devices
  );
  (input_devices, output_devices)
}

/// Try to open a handle to the CableAudio kernel driver.
/// Returns true if the driver is available, false otherwise.
#[tauri::command]
async fn connect_driver(state: State<'_, Mutex<AppData>>) -> Result<bool, String> {
  #[cfg(windows)]
  {
    let mut state = state.lock().await;
    match driver_client::DriverHandle::open() {
      Ok(handle) => {
        println!("CableAudio driver connected successfully");
        state.driver_handle = Some(Arc::new(handle));
        Ok(true)
      }
      Err(e) => {
        println!("CableAudio driver not available: {}", e);
        state.driver_handle = None;
        Ok(false)
      }
    }
  }
  #[cfg(not(windows))]
  {
    let _ = state;
    Ok(false)
  }
}

/// Check if the driver is currently connected.
#[tauri::command]
async fn is_driver_connected(state: State<'_, Mutex<AppData>>) -> Result<bool, String> {
  #[cfg(windows)]
  {
    let state = state.lock().await;
    Ok(state.driver_handle.is_some())
  }
  #[cfg(not(windows))]
  {
    let _ = state;
    Ok(false)
  }
}

#[tauri::command]
async fn setup_runtime(
  state: State<'_, Mutex<AppData>>,
  graph: AudioGraph,
  host: String,
  buffer_size: u32,
) -> Result<(), String> {
  println!("Setting up audio graph: {:?}", graph);
  let host_id = match cpal::available_hosts()
    .into_iter()
    .find(|h| format!("{:?}", h) == host)
  {
    Some(h) => h,
    None => return Err(format!("Audio host not found: {}", host)),
  };
  let audio_host = cpal::host_from_id(host_id).unwrap();

  let sample_rate = graph
    .edges
    .first()
    .and_then(|e| e.frequency)
    .unwrap_or(48000);

  println!(
    "Creating runtime with buffer size: {}, sample_rate: {}, host: {:?}",
    buffer_size, sample_rate, host_id
  );

  let app_state = state.lock().await;

  #[cfg(windows)]
  let driver_handle = app_state.driver_handle.clone();
  #[cfg(not(windows))]
  let driver_handle: Option<()> = None;

  drop(app_state);

  let mut runtime = runtime::Runtime::new(
    buffer_size,
    sample_rate,
    graph.nodes,
    graph.edges,
    audio_host,
    driver_handle,
  );

  runtime.init_nodes()?;

  let mut state = state.lock().await;
  state.runtime = Some(runtime);

  Ok(())
}

#[tauri::command]
async fn enable_runtime(state: State<'_, Mutex<AppData>>) -> Result<(), String> {
  let mut state = state.lock().await;
  if let Some(mut runtime) = state.runtime.take() {
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();

    let sleep_duration = runtime.buffer_duration();
    println!("Enabling runtime with sleep duration: {:?}", sleep_duration);

    let handle = std::thread::spawn(move || {
      while running_clone.load(Ordering::Relaxed) {
        if let Err(e) = runtime.process() {
          eprintln!("Error processing audio graph: {}", e);
        }
        std::thread::sleep(sleep_duration);
      }

      // 루프 종료 시 노드 정리
      if let Err(e) = runtime.dispose_nodes() {
        eprintln!("Error disposing nodes: {}", e);
      }
      println!("Runtime thread stopped.");
    });

    state.runtime_running = Some(running);
    state.runtime_thread = Some(handle);
  }
  Ok(())
}

#[tauri::command]
async fn disable_runtime(state: State<'_, Mutex<AppData>>) -> Result<(), String> {
  let mut state = state.lock().await;

  // AtomicBool을 false로 설정하여 루프 종료 시그널
  if let Some(running) = state.runtime_running.take() {
    running.store(false, Ordering::Relaxed);
  }

  // 스레드가 종료될 때까지 대기
  if let Some(handle) = state.runtime_thread.take() {
    handle
      .join()
      .map_err(|_| "Failed to join runtime thread".to_string())?;
  }

  println!("Runtime disabled.");
  Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  Builder::default()
    .plugin(tauri_plugin_opener::init())
    .manage(Mutex::new(AppData {
      runtime: None,
      runtime_thread: None,
      runtime_running: None,
      #[cfg(windows)]
      driver_handle: None,
    }))
    .invoke_handler(tauri::generate_handler![
      get_audio_hosts,
      get_audio_devices,
      connect_driver,
      is_driver_connected,
      setup_runtime,
      enable_runtime,
      disable_runtime,
    ])
    .run(tauri::generate_context!())
    .unwrap();
}
