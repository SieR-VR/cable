use cpal::traits::{DeviceTrait, HostTrait};
use serde::{Deserialize, Serialize};
use tauri::{async_runtime::Mutex, Builder, State};

pub mod nodes;
mod runtime;

use nodes::audio_input_device::AudioInputDeviceNode;
use nodes::audio_output_device::AudioOutputDeviceNode;

struct AppData {
  runtime: Option<runtime::Runtime>,
  runtime_thread: Option<std::thread::JoinHandle<()>>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AudioGraph {
  nodes: Vec<AudioNode>,
  edges: Vec<AudioEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub(crate) enum AudioNode {
  AudioInputDevice(AudioInputDeviceNode),
  AudioOutputDevice(AudioOutputDeviceNode),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

  println!(
    "Creating runtime with buffer size: {}, host: {:?}",
    buffer_size, host_id
  );
  let mut state = state.lock().await;
  state.runtime = Some(runtime::Runtime::new(
    buffer_size,
    graph.nodes,
    graph.edges,
    audio_host,
  ));

  Ok(())
}

#[tauri::command]
async fn enable_runtime(state: State<'_, Mutex<AppData>>) -> Result<(), String> {
  let mut state = state.lock().await;
  if let Some(runtime) = state.runtime.take() {
    let handle = std::thread::spawn(move || loop {
      if let Err(e) = runtime.process() {
        eprintln!("Error processing audio graph: {}", e);
      }
    });
    state.runtime_thread = Some(handle);
  }
  Ok(())
}

#[tauri::command]
async fn disable_runtime(state: State<'_, Mutex<AppData>>) -> Result<(), String> {
  let mut state = state.lock().await;
  if let Some(handle) = state.runtime_thread.take() {
    // In a real application, you'd want a more graceful shutdown mechanism
    handle.thread().unpark();
  }
  Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  Builder::default()
    .plugin(tauri_plugin_opener::init())
    .manage(Mutex::new(AppData {
      runtime: None,
      runtime_thread: None,
    }))
    .invoke_handler(tauri::generate_handler![
      get_audio_hosts,
      get_audio_devices,
      setup_runtime,
      enable_runtime,
      disable_runtime,
    ])
    .run(tauri::generate_context!())
    .unwrap();
}
