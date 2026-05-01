//! Win32 / COM helpers for audio endpoint discovery and rename.
//!
//! Used by the driver-side virtual-device commands to map newly-created
//! KSDevices onto Windows MM endpoints, and to write the user-visible
//! FriendlyName via PKEY_Device_DeviceDesc.

#![cfg(windows)]
// ---------------------------------------------------------------------------
// COM helpers for audio endpoint discovery and rename
// ---------------------------------------------------------------------------

/// Collect the IDs of all currently active MM audio endpoints into a HashSet.
///
/// Called on a blocking thread before creating a virtual device so we can
/// identify the new endpoint by set-difference after creation.
pub(crate) fn snapshot_endpoint_ids() -> std::collections::HashSet<String> {
  use windows::Win32::Media::Audio::{eAll, IMMDeviceEnumerator, MMDeviceEnumerator, DEVICE_STATE};
  use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
  };

  let mut ids = std::collections::HashSet::new();

  unsafe {
    let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
    if hr.is_err() && hr != windows::Win32::Foundation::S_FALSE {
      eprintln!("snapshot_endpoint_ids: CoInitializeEx failed: {:?}", hr);
      return ids;
    }
    let _guard = CoUninitGuard;

    let enumerator: IMMDeviceEnumerator =
      match CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER) {
        Ok(e) => e,
        Err(e) => {
          eprintln!("snapshot_endpoint_ids: CoCreateInstance failed: {}", e);
          return ids;
        }
      };

    // Enumerate all endpoints (active/disabled/not-present/unplugged).
    let collection = match enumerator.EnumAudioEndpoints(eAll, DEVICE_STATE(0xF)) {
      Ok(c) => c,
      Err(e) => {
        eprintln!("snapshot_endpoint_ids: EnumAudioEndpoints failed: {}", e);
        return ids;
      }
    };

    let count = match collection.GetCount() {
      Ok(n) => n,
      Err(_) => return ids,
    };

    for i in 0..count {
      let device = match collection.Item(i) {
        Ok(d) => d,
        Err(_) => continue,
      };
      let id_pwstr = match device.GetId() {
        Ok(p) => p,
        Err(_) => continue,
      };
      let id_str = id_pwstr.to_string().unwrap_or_default();
      windows::Win32::System::Com::CoTaskMemFree(Some(id_pwstr.as_ptr() as *const _));
      ids.insert(id_str);
    }
  }

  ids
}

/// Poll MM audio endpoints until one appears that is NOT in `pre_snapshot`.
///
/// Returns the new endpoint's ID string, e.g. `{0.0.0.00000000}.{guid}`.
/// Returns an empty string if no new endpoint appears within the retry window.
///
/// This avoids all PnP-tree traversal (CM_Get_Parent, SetupDi) — the new
/// endpoint is simply the one that wasn't there before the IOCTL.
pub(crate) fn find_new_endpoint_id(
  pre_snapshot: &std::collections::HashSet<String>,
  max_retries: u32,
  retry_delay_ms: u64,
) -> Result<String, String> {
  use windows::Win32::Media::Audio::{eAll, IMMDeviceEnumerator, MMDeviceEnumerator, DEVICE_STATE};
  use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
  };

  unsafe {
    let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
    if hr.is_err() && hr != windows::Win32::Foundation::S_FALSE {
      return Err(format!("CoInitializeEx failed: {:?}", hr));
    }
    let _coinit_guard = CoUninitGuard;

    let enumerator: IMMDeviceEnumerator =
      CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER)
        .map_err(|e| format!("CoCreateInstance(MMDeviceEnumerator) failed: {}", e))?;

    for attempt in 0..=max_retries {
      if attempt > 0 {
        std::thread::sleep(std::time::Duration::from_millis(retry_delay_ms));
        eprintln!("find_new_endpoint_id: retry {}/{}", attempt, max_retries);
      }

      let collection = match enumerator.EnumAudioEndpoints(eAll, DEVICE_STATE(0xF)) {
        Ok(c) => c,
        Err(e) => return Err(format!("EnumAudioEndpoints failed: {}", e)),
      };

      let count = match collection.GetCount() {
        Ok(n) => n,
        Err(e) => return Err(format!("GetCount failed: {}", e)),
      };

      for i in 0..count {
        let device = match collection.Item(i) {
          Ok(d) => d,
          Err(_) => continue,
        };
        let id_pwstr = match device.GetId() {
          Ok(p) => p,
          Err(_) => continue,
        };
        let id_str = id_pwstr.to_string().unwrap_or_default();
        windows::Win32::System::Com::CoTaskMemFree(Some(id_pwstr.as_ptr() as *const _));

        if !pre_snapshot.contains(&id_str) {
          eprintln!(
            "find_new_endpoint_id: found new endpoint '{}' on attempt {}",
            id_str, attempt
          );
          return Ok(id_str);
        }
      }
    }

    eprintln!(
      "find_new_endpoint_id: no new endpoint appeared after {} retries",
      max_retries
    );
    Ok(String::new())
  }
}

pub(crate) fn endpoint_exists(endpoint_id: &str) -> bool {
  use windows::Win32::Media::Audio::{IMMDeviceEnumerator, MMDeviceEnumerator};
  use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
  };

  unsafe {
    let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
    if hr.is_err() && hr != windows::Win32::Foundation::S_FALSE {
      return false;
    }
    let _coinit_guard = CoUninitGuard;

    let enumerator: IMMDeviceEnumerator =
      match CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER) {
        Ok(e) => e,
        Err(_) => return false,
      };

    let ep_wide: Vec<u16> = endpoint_id
      .encode_utf16()
      .chain(std::iter::once(0))
      .collect();

    enumerator
      .GetDevice(windows::core::PCWSTR(ep_wide.as_ptr()))
      .is_ok()
  }
}

/// Write PKEY_Device_DeviceDesc (pid=2) on the MM endpoint identified by
/// `endpoint_id`. This changes the first component of the FriendlyName that
/// Windows Audio shows in the Sound control panel and GetFriendlyName().
pub(crate) fn set_endpoint_device_desc(endpoint_id: &str, new_name: &str) -> Result<(), String> {
  use windows::Win32::Foundation::PROPERTYKEY;
  use windows::Win32::Media::Audio::{IMMDeviceEnumerator, MMDeviceEnumerator};
  use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
  use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemAlloc, CoTaskMemFree, CLSCTX_INPROC_SERVER,
    COINIT_MULTITHREADED, STGM,
  };
  use windows::Win32::System::Variant::VT_LPWSTR;
  use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;

  // PKEY_Device_DeviceDesc = {A45C254E-DF1C-4EFD-8020-67D146A850E0}, pid=2
  const PKEY_DEVICE_DESC_FMTID: windows::core::GUID = windows::core::GUID::from_values(
    0xA45C254E,
    0xDF1C,
    0x4EFD,
    [0x80, 0x20, 0x67, 0xD1, 0x46, 0xA8, 0x50, 0xE0],
  );

  let ep_wide: Vec<u16> = endpoint_id
    .encode_utf16()
    .chain(std::iter::once(0))
    .collect();
  let name_wide: Vec<u16> = new_name.encode_utf16().chain(std::iter::once(0)).collect();

  unsafe {
    let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
    if hr.is_err() && hr != windows::Win32::Foundation::S_FALSE {
      return Err(format!("CoInitializeEx failed: {:?}", hr));
    }
    let _coinit_guard = CoUninitGuard;

    let enumerator: IMMDeviceEnumerator =
      CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER)
        .map_err(|e| format!("CoCreateInstance(MMDeviceEnumerator) failed: {}", e))?;

    let device = enumerator
      .GetDevice(windows::core::PCWSTR(ep_wide.as_ptr()))
      .map_err(|e| format!("GetDevice('{}') failed: {}", endpoint_id, e))?;

    // STGM_READWRITE = 2
    let props: IPropertyStore = device
      .OpenPropertyStore(STGM(2))
      .map_err(|e| format!("OpenPropertyStore(READWRITE) failed: {}", e))?;

    // Build a VT_LPWSTR PROPVARIANT for the new name.
    // PROPVARIANT layout: vt (u16 at offset 0) + padding (6 bytes) + union (8 bytes).
    // We allocate a CoTaskMem buffer for the string and store the pointer at offset 8.
    let mut pv = PROPVARIANT::default();
    let byte_len = name_wide.len() * 2; // includes null terminator
    let buf = CoTaskMemAlloc(byte_len) as *mut u16;
    if buf.is_null() {
      return Err("CoTaskMemAlloc failed".to_string());
    }
    std::ptr::copy_nonoverlapping(name_wide.as_ptr(), buf, name_wide.len());

    let pv_ptr = &mut pv as *mut PROPVARIANT as *mut u8;
    *(pv_ptr as *mut u16) = VT_LPWSTR.0 as u16; // vt at offset 0
    *(pv_ptr.add(8) as *mut *mut u16) = buf; // pwszVal at offset 8

    let key = PROPERTYKEY {
      fmtid: PKEY_DEVICE_DESC_FMTID,
      pid: 2,
    };

    let set_result = props.SetValue(&key, &pv);

    // Always free the buffer we allocated.
    CoTaskMemFree(Some(buf as *const _));
    // Zero the pointer in pv so it isn't double-freed by accident.
    *(pv_ptr.add(8) as *mut *mut u16) = std::ptr::null_mut();

    set_result.map_err(|e| format!("IPropertyStore::SetValue(DeviceDesc) failed: {}", e))?;

    props
      .Commit()
      .map_err(|e| format!("IPropertyStore::Commit failed: {}", e))?;

    println!(
      "IPropertyStore::SetValue(DeviceDesc) OK for endpoint '{}'",
      endpoint_id
    );
    Ok(())
  }
}

/// RAII guard that calls CoUninitialize when dropped.
pub(crate) struct CoUninitGuard;

impl Drop for CoUninitGuard {
  fn drop(&mut self) {
    unsafe {
      windows::Win32::System::Com::CoUninitialize();
    }
  }
}

// ---------------------------------------------------------------------------
// Elevated rename helpers
// ---------------------------------------------------------------------------

/// Public entry point called from `main.rs` when the process is re-launched
/// with `--rename-endpoint <endpoint_id> <name>`.
///
/// This runs in an elevated (admin) context and writes PKEY_Device_DeviceDesc
/// via IPropertyStore, then returns.
pub fn rename_endpoint_elevated(endpoint_id: &str, new_name: &str) -> Result<(), String> {
  set_endpoint_device_desc(endpoint_id, new_name)
}

// ---------------------------------------------------------------------------
// Elevated device format helpers
// ---------------------------------------------------------------------------

/// WAVEFORMATEXTENSIBLE as a packed C struct for building property-store blobs.
///
/// Layout (packed, 40 bytes total):
///   WAVEFORMATEX  (18 bytes)
///   wValidBitsPerSample (2 bytes)
///   dwChannelMask       (4 bytes)
///   SubFormat GUID     (16 bytes)
#[repr(C, packed)]
struct WaveFormatExtensible {
  w_format_tag: u16,
  n_channels: u16,
  n_samples_per_sec: u32,
  n_avg_bytes_per_sec: u32,
  n_block_align: u16,
  w_bits_per_sample: u16,
  cb_size: u16,              // 22 = size of WAVEFORMATEXTENSIBLE extension
  w_valid_bits_per_sample: u16,
  dw_channel_mask: u32,
  sub_format: [u8; 16],      // GUID – KSDATAFORMAT_SUBTYPE_PCM
}

// KSDATAFORMAT_SUBTYPE_PCM = {00000001-0000-0010-8000-00AA00389B71}
const KSDATAFORMAT_SUBTYPE_PCM: [u8; 16] = [
  0x01, 0x00, 0x00, 0x00, // Data1
  0x00, 0x00,             // Data2
  0x10, 0x00,             // Data3
  0x80, 0x00,             // Data4[0..1]
  0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71, // Data4[2..7]
];

// Common channel masks
const SPEAKER_FRONT_LEFT: u32 = 0x1;
const SPEAKER_FRONT_RIGHT: u32 = 0x2;
const SPEAKER_FRONT_CENTER: u32 = 0x4;

/// Write `PKEY_AudioEngine_DeviceFormat` on the MM endpoint identified by
/// `endpoint_id`. This changes the preferred audio stream format that Windows
/// Audio Engine uses when opening the endpoint.
pub(crate) fn set_endpoint_device_format(
  endpoint_id: &str,
  sample_rate: u32,
  channels: u16,
  bits_per_sample: u16,
) -> Result<(), String> {
  use windows::Win32::Foundation::PROPERTYKEY;
  use windows::Win32::Media::Audio::{IMMDeviceEnumerator, MMDeviceEnumerator};
  use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
  use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemAlloc, CoTaskMemFree, CLSCTX_INPROC_SERVER,
    COINIT_MULTITHREADED, STGM,
  };
  use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;

  // PKEY_AudioEngine_DeviceFormat = {F19F064D-082C-4E27-BC73-6882A1BB8E4C}, pid=0
  const PKEY_AUDIO_ENGINE_DEVICE_FORMAT_FMTID: windows::core::GUID =
    windows::core::GUID::from_values(
      0xF19F064D,
      0x082C,
      0x4E27,
      [0xBC, 0x73, 0x68, 0x82, 0xA1, 0xBB, 0x8E, 0x4C],
    );

  let bytes_per_sample = (bits_per_sample / 8) as u32;
  let channel_mask = match channels {
    1 => SPEAKER_FRONT_CENTER,
    _ => SPEAKER_FRONT_LEFT | SPEAKER_FRONT_RIGHT,
  };

  let fmt = WaveFormatExtensible {
    w_format_tag: 0xFFFE, // WAVE_FORMAT_EXTENSIBLE
    n_channels: channels,
    n_samples_per_sec: sample_rate,
    n_avg_bytes_per_sec: sample_rate * channels as u32 * bytes_per_sample,
    n_block_align: channels * (bits_per_sample / 8),
    w_bits_per_sample: bits_per_sample,
    cb_size: 22, // size of WAVEFORMATEXTENSIBLE fields after cbSize
    w_valid_bits_per_sample: bits_per_sample,
    dw_channel_mask: channel_mask,
    sub_format: KSDATAFORMAT_SUBTYPE_PCM,
  };

  let blob_size = std::mem::size_of::<WaveFormatExtensible>() as u32;

  let ep_wide: Vec<u16> = endpoint_id
    .encode_utf16()
    .chain(std::iter::once(0))
    .collect();

  unsafe {
    let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
    if hr.is_err() && hr != windows::Win32::Foundation::S_FALSE {
      return Err(format!("CoInitializeEx failed: {:?}", hr));
    }
    let _coinit_guard = CoUninitGuard;

    let enumerator: IMMDeviceEnumerator =
      CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER)
        .map_err(|e| format!("CoCreateInstance(MMDeviceEnumerator) failed: {}", e))?;

    let device = enumerator
      .GetDevice(windows::core::PCWSTR(ep_wide.as_ptr()))
      .map_err(|e| format!("GetDevice('{}') failed: {}", endpoint_id, e))?;

    // STGM_READWRITE = 2
    let props: IPropertyStore = device
      .OpenPropertyStore(STGM(2))
      .map_err(|e| format!("OpenPropertyStore(READWRITE) failed: {}", e))?;

    // Build VT_BLOB PROPVARIANT.
    //
    // PROPVARIANT memory layout (x86-64):
    //   offset  0: vt              (u16)
    //   offset  2: wReserved1-3    (3 × u16)
    //   offset  8: blob.cbSize     (u32)
    //   offset 12: [4-byte padding to align the 8-byte pointer]
    //   offset 16: blob.pBlobData  (*mut u8)
    //
    // Using pointer arithmetic because windows-rs does not expose the
    // BLOB sub-union directly and `std::mem::offset_of!` is stable only
    // from Rust 1.77+.
    let buf = CoTaskMemAlloc(blob_size as usize) as *mut WaveFormatExtensible;
    if buf.is_null() {
      return Err("CoTaskMemAlloc for format blob failed".to_string());
    }
    std::ptr::write_unaligned(buf, fmt);

    let mut pv = PROPVARIANT::default();
    let pv_ptr = &mut pv as *mut PROPVARIANT as *mut u8;
    *(pv_ptr as *mut u16) = 0x41u16; // VT_BLOB
    *(pv_ptr.add(8) as *mut u32) = blob_size; // blob.cbSize
    *(pv_ptr.add(16) as *mut *mut u8) = buf as *mut u8; // blob.pBlobData (8-byte aligned)

    let key = PROPERTYKEY {
      fmtid: PKEY_AUDIO_ENGINE_DEVICE_FORMAT_FMTID,
      pid: 0,
    };

    let set_result = props.SetValue(&key, &pv);

    CoTaskMemFree(Some(buf as *const _));
    *(pv_ptr.add(16) as *mut *mut u8) = std::ptr::null_mut();

    set_result.map_err(|e| {
      format!(
        "IPropertyStore::SetValue(AudioEngineDeviceFormat) failed: {}",
        e
      )
    })?;

    props
      .Commit()
      .map_err(|e| format!("IPropertyStore::Commit failed: {}", e))?;

    println!(
      "IPropertyStore::SetValue(DeviceFormat) OK: endpoint='{}' rate={} ch={} bits={}",
      endpoint_id, sample_rate, channels, bits_per_sample
    );
    Ok(())
  }
}

/// Public entry point called from `main.rs` when the process is re-launched
/// with `--set-endpoint-format <endpoint_id> <sample_rate> <channels> <bits_per_sample>`.
pub fn set_endpoint_format_elevated(
  endpoint_id: &str,
  sample_rate: u32,
  channels: u16,
  bits_per_sample: u16,
) -> Result<(), String> {
  set_endpoint_device_format(endpoint_id, sample_rate, channels, bits_per_sample)
}

/// Re-launch the current executable elevated with `--set-endpoint-format`
/// arguments so that the UAC-protected property store write runs in an admin context.
///
/// Blocks until the elevated child exits.
pub(crate) fn elevated_set_endpoint_device_format(
  endpoint_id: &str,
  sample_rate: u32,
  channels: u16,
  bits_per_sample: u16,
) -> Result<(), String> {
  use windows::core::PCWSTR;
  use windows::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
  use windows::Win32::System::Threading::{GetExitCodeProcess, INFINITE, WaitForSingleObject};
  use windows::Win32::UI::Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW};

  let args_str = format!(
    "--set-endpoint-format {} {} {} {}",
    endpoint_id, sample_rate, channels, bits_per_sample
  );

  let exe_path = std::env::current_exe().map_err(|e| format!("current_exe() failed: {}", e))?;
  let exe_str = exe_path
    .to_str()
    .ok_or_else(|| "exe path is not valid UTF-8".to_string())?;

  let verb: Vec<u16> = "runas\0".encode_utf16().collect();
  let exe_wide: Vec<u16> = exe_str.encode_utf16().chain(std::iter::once(0)).collect();
  let args_wide: Vec<u16> = args_str.encode_utf16().chain(std::iter::once(0)).collect();

  unsafe {
    let mut sei = SHELLEXECUTEINFOW {
      cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
      fMask: SEE_MASK_NOCLOSEPROCESS,
      hwnd: windows::Win32::Foundation::HWND(std::ptr::null_mut()),
      lpVerb: PCWSTR(verb.as_ptr()),
      lpFile: PCWSTR(exe_wide.as_ptr()),
      lpParameters: PCWSTR(args_wide.as_ptr()),
      lpDirectory: PCWSTR(std::ptr::null()),
      nShow: 0, // SW_HIDE
      ..Default::default()
    };

    ShellExecuteExW(&mut sei).map_err(|e| {
      format!(
        "ShellExecuteExW(runas) failed: {} — user may have cancelled UAC",
        e
      )
    })?;

    let hproc = sei.hProcess;
    if hproc.is_invalid() {
      return Err("ShellExecuteExW returned invalid process handle".to_string());
    }

    let wait_result = WaitForSingleObject(hproc, INFINITE);
    if wait_result != WAIT_OBJECT_0 {
      let _ = CloseHandle(hproc);
      return Err(format!(
        "WaitForSingleObject failed (result={:?})",
        wait_result
      ));
    }

    let mut exit_code: u32 = 0;
    let _ = GetExitCodeProcess(hproc, &mut exit_code);
    let _ = CloseHandle(hproc);

    if exit_code != 0 {
      return Err(format!(
        "Elevated set-format process exited with code {} (format change failed)",
        exit_code
      ));
    }

    Ok(())
  }
}

/// Re-launch the current executable with verb "runas" so that Windows shows a
/// UAC prompt and the child process runs elevated.  The child is invoked with
/// `--rename-endpoint <endpoint_id> <new_name>` arguments.
///
/// This function blocks until the elevated child exits and returns an error if
/// the user cancels the UAC prompt or the child exits with a non-zero code.
pub(crate) fn elevated_set_endpoint_device_desc(
  endpoint_id: &str,
  new_name: &str,
) -> Result<(), String> {
  use windows::core::PCWSTR;
  use windows::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
  use windows::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject, INFINITE};
  use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};

  // Build the argument string: --rename-endpoint <endpoint_id> <new_name>
  // We quote the name component to preserve spaces.
  // The endpoint_id is a Windows audio endpoint path — it can contain braces
  // and dots but not spaces, so no quoting needed.
  let args_str = format!(
    "--rename-endpoint {} {}",
    endpoint_id,
    shell_quote(new_name)
  );

  // Get the path of the current executable.
  let exe_path = std::env::current_exe().map_err(|e| format!("current_exe() failed: {}", e))?;
  let exe_str = exe_path
    .to_str()
    .ok_or_else(|| "exe path is not valid UTF-8".to_string())?;

  // Encode as wide strings with null terminator.
  let verb: Vec<u16> = "runas\0".encode_utf16().collect();
  let exe_wide: Vec<u16> = exe_str.encode_utf16().chain(std::iter::once(0)).collect();
  let args_wide: Vec<u16> = args_str.encode_utf16().chain(std::iter::once(0)).collect();

  unsafe {
    let mut sei = SHELLEXECUTEINFOW {
      cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
      fMask: SEE_MASK_NOCLOSEPROCESS,
      hwnd: windows::Win32::Foundation::HWND(std::ptr::null_mut()),
      lpVerb: PCWSTR(verb.as_ptr()),
      lpFile: PCWSTR(exe_wide.as_ptr()),
      lpParameters: PCWSTR(args_wide.as_ptr()),
      lpDirectory: PCWSTR(std::ptr::null()),
      nShow: 0, // SW_HIDE
      ..Default::default()
    };

    ShellExecuteExW(&mut sei).map_err(|e| {
      format!(
        "ShellExecuteExW(runas) failed: {} — user may have cancelled UAC",
        e
      )
    })?;

    // hProcess is only valid when SEE_MASK_NOCLOSEPROCESS is set and the
    // operation succeeded.
    let hproc = sei.hProcess;
    if hproc.is_invalid() {
      return Err("ShellExecuteExW returned invalid process handle".to_string());
    }

    // Wait for the elevated child to complete.
    let wait_result = WaitForSingleObject(hproc, INFINITE);
    if wait_result != WAIT_OBJECT_0 {
      let _ = CloseHandle(hproc);
      return Err(format!(
        "WaitForSingleObject failed (result={:?})",
        wait_result
      ));
    }

    let mut exit_code: u32 = 0;
    let _ = GetExitCodeProcess(hproc, &mut exit_code);
    let _ = CloseHandle(hproc);

    if exit_code != 0 {
      return Err(format!(
        "Elevated rename process exited with code {} (rename failed)",
        exit_code
      ));
    }

    Ok(())
  }
}

/// Minimally quote a string for use as a single shell token:
/// wraps in double-quotes if the string contains spaces, escaping embedded
/// double-quotes with backslash.
pub(crate) fn shell_quote(s: &str) -> String {
  if s.contains(' ') || s.contains('"') {
    let escaped = s.replace('"', "\\\"");
    format!("\"{}\"", escaped)
  } else {
    s.to_string()
  }
}
