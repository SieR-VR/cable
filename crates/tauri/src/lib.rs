use std::sync::Mutex;
use tauri::{Builder, Manager, State};

use windows::core::PWSTR;
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Media::Audio::{
  IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator, MMDeviceEnumerator,
  AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK,
};
use windows::Win32::System::Com::StructuredStorage::PropVariantToBSTR;
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL, STGM_READ};

struct AppData {
  devices: Option<Vec<AudioDevice>>,
}

#[derive(Clone)]
struct AudioDevice {
  friendly_name: String,
  id: Vec<u16>,
}

#[tauri::command]
fn get_audio_devices(state: State<'_, Mutex<AppData>>) -> Vec<String> {
  let mut state = state.lock().unwrap();
  if let Some(devices) = &state.devices {
    return devices.iter().map(|d| d.friendly_name.clone()).collect();
  }

  unsafe {
    let enumerator: IMMDeviceEnumerator =
      CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).unwrap();

    let device_collection = enumerator
      .EnumAudioEndpoints(
        windows::Win32::Media::Audio::eRender,
        windows::Win32::Media::Audio::DEVICE_STATE_ACTIVE,
      )
      .unwrap();
    let count = device_collection.GetCount().unwrap();

    let mut devices: Vec<AudioDevice> = Vec::new();
    for i in 0..count {
      let device = device_collection.Item(i).unwrap();
      let device_name_propvariant = device
        .OpenPropertyStore(STGM_READ)
        .unwrap()
        .GetValue(&PKEY_Device_FriendlyName)
        .unwrap();
      let device_name = PropVariantToBSTR(&device_name_propvariant)
        .unwrap()
        .to_string();
      let device_id = device.GetId().unwrap();

      devices.push(AudioDevice {
        friendly_name: device_name,
        id: device_id.as_wide().to_vec(),
      });
    }

    state.devices = Some(devices.clone());
    devices.iter().map(|d| d.friendly_name.clone()).collect()
  }
}

#[tauri::command]
fn capture_audio(
  state: State<'_, Mutex<AppData>>,
  device_index: usize,
) -> Result<(), windows::core::Error> {
  let state = state.lock().unwrap();
  let devices = state.devices.as_ref().expect("Audio devices not loaded");
  let device = &devices[device_index];
  let device_id = PWSTR::from_raw(device.id.as_ptr() as *mut u16);

  unsafe {
    let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
    let device = enumerator.GetDevice(device_id)?;

    let audio_client: IAudioClient = device.Activate::<IAudioClient>(CLSCTX_ALL, None).unwrap();

    audio_client.Initialize(
      AUDCLNT_SHAREMODE_SHARED,
      AUDCLNT_STREAMFLAGS_LOOPBACK,
      10000000,
      0,
      audio_client.GetMixFormat()?,
      None,
    )?;

    let capture_client = audio_client.GetService::<IAudioCaptureClient>()?;

    Ok(())
  }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  Builder::default()
    .setup(|app| {
      app.manage(AppData { devices: None });
      Ok(())
    })
    .plugin(tauri_plugin_opener::init())
    .invoke_handler(tauri::generate_handler![get_audio_devices])
    .run(tauri::generate_context!())
    .unwrap();
}
