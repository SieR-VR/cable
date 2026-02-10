use cpal::traits::{DeviceTrait, HostTrait};
use tauri::Builder;

struct AppData {}

#[derive(Debug, Clone, serde::Serialize)]
struct AudioDevice {
  id: String,
  readable_name: String,
  descriptions: Vec<String>,
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

      AudioDevice {
        id: d.id().unwrap().to_string(),
        readable_name: description.name().to_string(),
        descriptions: description.extended().to_vec(),
      }
    })
    .collect();
  let output_devices = host
    .output_devices()
    .unwrap()
    .map(|d| {
      let description = d.description().unwrap();

      AudioDevice {
        id: d.id().unwrap().to_string(),
        readable_name: description.name().to_string(),
        descriptions: description.extended().to_vec(),
      }
    })
    .collect();

  println!(
    "Input devices: {:?}, Output devices: {:?}",
    input_devices, output_devices
  );
  (input_devices, output_devices)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  Builder::default()
    .plugin(tauri_plugin_opener::init())
    .manage(AppData {})
    .invoke_handler(tauri::generate_handler![get_audio_hosts, get_audio_devices])
    .run(tauri::generate_context!())
    .unwrap();
}
