//! Windows-specific Bluetooth resolution. Maps an MM endpoint device id to
//! the underlying Bluetooth PnP container via `PKEY_Device_ContainerId`.

#![cfg(windows)]

use windows::core::{GUID, PCWSTR};
use windows::Win32::Devices::DeviceAndDriverInstallation::{
  SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInfo, SetupDiGetClassDevsW,
  SetupDiGetDevicePropertyW, DIGCF_ALLCLASSES, DIGCF_PRESENT, HDEVINFO, SP_DEVINFO_DATA,
};
use windows::Win32::Devices::Properties::{
  DEVPROPTYPE, DEVPROP_TYPE_GUID, DEVPROP_TYPE_STRING, DEVPROP_TYPE_STRING_LIST,
  DEVPROP_TYPE_UINT16,
};
use windows::Win32::Foundation::{
  DEVPROPKEY, ERROR_INSUFFICIENT_BUFFER, ERROR_NO_MORE_ITEMS, HWND, PROPERTYKEY,
};
use windows::Win32::Media::Audio::{IMMDeviceEnumerator, MMDeviceEnumerator};
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::Win32::System::Com::{
  CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, STGM,
};
use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;

use super::BluetoothInfo;
use crate::driver::endpoint::CoUninitGuard;

// PKEY_Device_ContainerId = {8C7ED206-3F8A-4827-B3AB-AE9E1FAEFC6C}, pid=2
const FMT_DEVICE: GUID = GUID::from_values(
  0x8C7ED206,
  0x3F8A,
  0x4827,
  [0xB3, 0xAB, 0xAE, 0x9E, 0x1F, 0xAE, 0xFC, 0x6C],
);

const PKEY_DEVICE_CONTAINER_ID: PROPERTYKEY = PROPERTYKEY {
  fmtid: FMT_DEVICE,
  pid: 2,
};

const DEVPKEY_DEVICE_CONTAINER_ID: DEVPROPKEY = DEVPROPKEY {
  fmtid: FMT_DEVICE,
  pid: 2,
};

// PKEY_Device_EnumeratorName = {A45C254E-DF1C-4EFD-8020-67D146A850E0}, pid=24
const DEVPKEY_DEVICE_ENUMERATOR_NAME: DEVPROPKEY = DEVPROPKEY {
  fmtid: GUID::from_values(
    0xA45C254E,
    0xDF1C,
    0x4EFD,
    [0x80, 0x20, 0x67, 0xD1, 0x46, 0xA8, 0x50, 0xE0],
  ),
  pid: 24,
};

// Bluetooth FMTID = {2BD67D8B-8BEB-48D5-87E0-6CDA3428040A}
const FMT_BLUETOOTH: GUID = GUID::from_values(
  0x2BD67D8B,
  0x8BEB,
  0x48D5,
  [0x87, 0xE0, 0x6C, 0xDA, 0x34, 0x28, 0x04, 0x0A],
);

const DEVPKEY_BLUETOOTH_ADDRESS: DEVPROPKEY = DEVPROPKEY {
  fmtid: FMT_BLUETOOTH,
  pid: 1,
};
const DEVPKEY_BLUETOOTH_VENDOR_ID: DEVPROPKEY = DEVPROPKEY {
  fmtid: FMT_BLUETOOTH,
  pid: 8,
};
const DEVPKEY_BLUETOOTH_PRODUCT_ID: DEVPROPKEY = DEVPROPKEY {
  fmtid: FMT_BLUETOOTH,
  pid: 9,
};

// DEVPKEY_DeviceContainer_Category = {78C34FC8-104A-4ACA-9EA4-524D52996E57}, pid=90
const DEVPKEY_DEVICECONTAINER_CATEGORY: DEVPROPKEY = DEVPROPKEY {
  fmtid: GUID::from_values(
    0x78C34FC8,
    0x104A,
    0x4ACA,
    [0x9E, 0xA4, 0x52, 0x4D, 0x52, 0x99, 0x6E, 0x57],
  ),
  pid: 90,
};

/// Resolve the BT-side identity of an audio endpoint.
///
/// `audio_device_id` is the cpal `DeviceId::Display` string on Windows, which
/// is `"<HostId>:<MMDevice endpoint id>"`, e.g.
/// `Wasapi:{0.0.0.00000000}.{guid}`. We strip the host prefix and feed the
/// remainder into `IMMDeviceEnumerator::GetDevice`. Returns `None` for
/// endpoints that aren't backed by a Bluetooth device.
pub fn resolve_bluetooth_info(audio_device_id: &str) -> Option<BluetoothInfo> {
  let endpoint_id = strip_host_prefix(audio_device_id);
  let container = unsafe { read_endpoint_container_id(endpoint_id) }?;
  let info = unsafe { collect_container_info(&container) };
  if !info.is_bluetooth {
    return None;
  }
  Some(info)
}

/// Strip the cpal `HostId:` prefix (e.g. `Wasapi:`) so what's left is the
/// MMDevice endpoint id understood by `IMMDeviceEnumerator::GetDevice`.
fn strip_host_prefix(s: &str) -> &str {
  // The endpoint id starts with '{', so split on the first ':' that comes
  // before the first '{'. This avoids touching the GUID braces.
  if let Some(brace) = s.find('{') {
    if let Some(colon) = s[..brace].rfind(':') {
      return &s[colon + 1..];
    }
  }
  s
}

unsafe fn read_endpoint_container_id(endpoint_id: &str) -> Option<GUID> {
  // S_OK: we initialized; S_FALSE: already initialized in same mode (someone
  // else owns the deinit); RPC_E_CHANGED_MODE (0x80010106): cpal initialized
  // this thread as STA — that's fine, we don't need MTA, just proceed and
  // skip CoUninitialize on drop in that case.
  let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
  let owns_com = hr.is_ok();
  let changed_mode = hr == windows::Win32::Foundation::RPC_E_CHANGED_MODE;
  if hr.is_err() && hr != windows::Win32::Foundation::S_FALSE && !changed_mode {
    return None;
  }
  let _coinit_guard = if owns_com { Some(CoUninitGuard) } else { None };

  let enumerator: IMMDeviceEnumerator =
    CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER).ok()?;
  let ep_wide: Vec<u16> = endpoint_id
    .encode_utf16()
    .chain(std::iter::once(0))
    .collect();
  let device = enumerator.GetDevice(PCWSTR(ep_wide.as_ptr())).ok()?;
  let props: IPropertyStore = device.OpenPropertyStore(STGM(0)).ok()?;
  let pv = props.GetValue(&PKEY_DEVICE_CONTAINER_ID).ok()?;
  let guid = propvariant_clsid(&pv)?;
  // {00000000-0000-0000-FFFF-FFFFFFFFFFFF} is the Windows "system container"
  // used for non-removable on-board devices. It groups every internal device,
  // so matching against it would produce false positives. Treat as no
  // container.
  if guid == GUID::from_values(0, 0, 0, [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]) {
    return None;
  }
  Some(guid)
}

/// Extract a GUID from a VT_CLSID PROPVARIANT (puuid pointer at union offset).
unsafe fn propvariant_clsid(pv: &PROPVARIANT) -> Option<GUID> {
  let ptr = pv as *const PROPVARIANT as *const u8;
  let vt = *(ptr as *const u16);
  // VT_CLSID = 72
  if vt != 72 {
    return None;
  }
  let pguid_ptr = *(ptr.add(8) as *const *const GUID);
  if pguid_ptr.is_null() {
    return None;
  }
  Some(*pguid_ptr)
}

unsafe fn collect_container_info(target: &GUID) -> BluetoothInfo {
  let mut info = BluetoothInfo {
    container_id: format_guid(target),
    address: None,
    vendor_id: None,
    product_id: None,
    category: None,
    is_bluetooth: false,
  };

  let h: HDEVINFO = match SetupDiGetClassDevsW(
    None,
    PCWSTR::null(),
    Some(HWND(std::ptr::null_mut())),
    DIGCF_ALLCLASSES | DIGCF_PRESENT,
  ) {
    Ok(h) => h,
    Err(_) => return info,
  };

  let mut index: u32 = 0;
  loop {
    let mut data = SP_DEVINFO_DATA {
      cbSize: std::mem::size_of::<SP_DEVINFO_DATA>() as u32,
      ..Default::default()
    };
    match SetupDiEnumDeviceInfo(h, index, &mut data) {
      Ok(()) => {}
      Err(e) if e.code() == ERROR_NO_MORE_ITEMS.to_hresult() => break,
      Err(_) => break,
    }
    index += 1;

    let cid = match read_prop_guid(h, &data, &DEVPKEY_DEVICE_CONTAINER_ID) {
      Some(g) => g,
      None => continue,
    };
    if cid != *target {
      continue;
    }

    if !info.is_bluetooth {
      if let Some(en) = read_prop_string(h, &data, &DEVPKEY_DEVICE_ENUMERATOR_NAME) {
        let upper = en.to_uppercase();
        if upper.contains("BTH") {
          info.is_bluetooth = true;
        }
      }
    }
    if info.address.is_none() {
      if let Some(addr) = read_prop_string(h, &data, &DEVPKEY_BLUETOOTH_ADDRESS) {
        info.address = format_mac(&addr);
      }
    }
    if info.vendor_id.is_none() {
      info.vendor_id = read_prop_u16(h, &data, &DEVPKEY_BLUETOOTH_VENDOR_ID);
    }
    if info.product_id.is_none() {
      info.product_id = read_prop_u16(h, &data, &DEVPKEY_BLUETOOTH_PRODUCT_ID);
    }
    if info.category.is_none() {
      if let Some(list) = read_prop_string_list(h, &data, &DEVPKEY_DEVICECONTAINER_CATEGORY) {
        info.category = list.into_iter().next();
      }
    }
  }

  let _ = SetupDiDestroyDeviceInfoList(h);
  info
}

/// Walk every PnP device once and group those that share a Bluetooth
/// container. Used by the Phase B watcher to map an advertisement model id
/// back to a container id without re-running `collect_container_info` for
/// every endpoint.
pub fn enumerate_bluetooth_containers() -> Vec<BluetoothInfo> {
  unsafe {
    let h = match SetupDiGetClassDevsW(
      None,
      PCWSTR::null(),
      Some(HWND(std::ptr::null_mut())),
      DIGCF_ALLCLASSES | DIGCF_PRESENT,
    ) {
      Ok(h) => h,
      Err(_) => return Vec::new(),
    };
    let mut by_id: std::collections::HashMap<String, BluetoothInfo> =
      std::collections::HashMap::new();
    let mut index: u32 = 0;
    loop {
      let mut data = SP_DEVINFO_DATA {
        cbSize: std::mem::size_of::<SP_DEVINFO_DATA>() as u32,
        ..Default::default()
      };
      match SetupDiEnumDeviceInfo(h, index, &mut data) {
        Ok(()) => {}
        Err(e) if e.code() == ERROR_NO_MORE_ITEMS.to_hresult() => break,
        Err(_) => break,
      }
      index += 1;

      let cid = match read_prop_guid(h, &data, &DEVPKEY_DEVICE_CONTAINER_ID) {
        Some(g) => g,
        None => continue,
      };
      // Skip system container.
      if cid == GUID::from_values(0, 0, 0, [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]) {
        continue;
      }
      let cid_str = format_guid(&cid);
      let entry = by_id
        .entry(cid_str.clone())
        .or_insert_with(|| BluetoothInfo {
          container_id: cid_str,
          address: None,
          vendor_id: None,
          product_id: None,
          category: None,
          is_bluetooth: false,
        });
      if !entry.is_bluetooth {
        if let Some(en) = read_prop_string(h, &data, &DEVPKEY_DEVICE_ENUMERATOR_NAME) {
          if en.to_uppercase().contains("BTH") {
            entry.is_bluetooth = true;
          }
        }
      }
      if entry.address.is_none() {
        if let Some(addr) = read_prop_string(h, &data, &DEVPKEY_BLUETOOTH_ADDRESS) {
          entry.address = format_mac(&addr);
        }
      }
      if entry.vendor_id.is_none() {
        entry.vendor_id = read_prop_u16(h, &data, &DEVPKEY_BLUETOOTH_VENDOR_ID);
      }
      if entry.product_id.is_none() {
        entry.product_id = read_prop_u16(h, &data, &DEVPKEY_BLUETOOTH_PRODUCT_ID);
      }
      if entry.category.is_none() {
        if let Some(list) = read_prop_string_list(h, &data, &DEVPKEY_DEVICECONTAINER_CATEGORY) {
          entry.category = list.into_iter().next();
        }
      }
    }
    let _ = SetupDiDestroyDeviceInfoList(h);
    by_id.into_values().filter(|i| i.is_bluetooth).collect()
  }
}

unsafe fn read_prop_raw(
  h: HDEVINFO,
  data: &SP_DEVINFO_DATA,
  key: &DEVPROPKEY,
  expected_type: DEVPROPTYPE,
) -> Option<Vec<u8>> {
  let mut got_type = DEVPROPTYPE::default();
  let mut required: u32 = 0;
  // Probe size.
  let probe = SetupDiGetDevicePropertyW(h, data, key, &mut got_type, None, Some(&mut required), 0);
  match probe {
    Ok(()) => {}
    Err(e) => {
      if e.code() != ERROR_INSUFFICIENT_BUFFER.to_hresult() && required == 0 {
        return None;
      }
    }
  }
  if got_type != expected_type || required == 0 {
    return None;
  }
  let mut buf = vec![0u8; required as usize];
  SetupDiGetDevicePropertyW(
    h,
    data,
    key,
    &mut got_type,
    Some(&mut buf),
    Some(&mut required),
    0,
  )
  .ok()?;
  Some(buf)
}

unsafe fn read_prop_guid(h: HDEVINFO, data: &SP_DEVINFO_DATA, key: &DEVPROPKEY) -> Option<GUID> {
  let buf = read_prop_raw(h, data, key, DEVPROP_TYPE_GUID)?;
  if buf.len() < std::mem::size_of::<GUID>() {
    return None;
  }
  Some(*(buf.as_ptr() as *const GUID))
}

unsafe fn read_prop_string(
  h: HDEVINFO,
  data: &SP_DEVINFO_DATA,
  key: &DEVPROPKEY,
) -> Option<String> {
  let buf = read_prop_raw(h, data, key, DEVPROP_TYPE_STRING)?;
  decode_wide_z(&buf)
}

unsafe fn read_prop_string_list(
  h: HDEVINFO,
  data: &SP_DEVINFO_DATA,
  key: &DEVPROPKEY,
) -> Option<Vec<String>> {
  let buf = read_prop_raw(h, data, key, DEVPROP_TYPE_STRING_LIST)?;
  let wide: &[u16] = std::slice::from_raw_parts(buf.as_ptr() as *const u16, buf.len() / 2);
  let mut out = Vec::new();
  let mut start = 0usize;
  for i in 0..wide.len() {
    if wide[i] == 0 {
      if i > start {
        out.push(String::from_utf16_lossy(&wide[start..i]));
      }
      start = i + 1;
    }
  }
  Some(out)
}

unsafe fn read_prop_u16(h: HDEVINFO, data: &SP_DEVINFO_DATA, key: &DEVPROPKEY) -> Option<u16> {
  let buf = read_prop_raw(h, data, key, DEVPROP_TYPE_UINT16)?;
  if buf.len() < 2 {
    return None;
  }
  Some(u16::from_le_bytes([buf[0], buf[1]]))
}

fn decode_wide_z(buf: &[u8]) -> Option<String> {
  if buf.len() < 2 {
    return None;
  }
  let wide: &[u16] =
    unsafe { std::slice::from_raw_parts(buf.as_ptr() as *const u16, buf.len() / 2) };
  let end = wide.iter().position(|&c| c == 0).unwrap_or(wide.len());
  Some(String::from_utf16_lossy(&wide[..end]))
}

fn format_guid(g: &GUID) -> String {
  format!(
    "{{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
    g.data1,
    g.data2,
    g.data3,
    g.data4[0],
    g.data4[1],
    g.data4[2],
    g.data4[3],
    g.data4[4],
    g.data4[5],
    g.data4[6],
    g.data4[7],
  )
}

/// Format a 12-character hex MAC ("142876B1A2A3") as "14:28:76:B1:A2:A3".
fn format_mac(raw: &str) -> Option<String> {
  let cleaned: String = raw.chars().filter(|c| c.is_ascii_hexdigit()).collect();
  if cleaned.len() != 12 {
    return None;
  }
  let bytes: Vec<&str> = (0..6).map(|i| &cleaned[i * 2..i * 2 + 2]).collect();
  Some(bytes.join(":").to_uppercase())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn strip_host_prefix_wasapi() {
    assert_eq!(
      strip_host_prefix("Wasapi:{0.0.0.00000000}.{abc}"),
      "{0.0.0.00000000}.{abc}"
    );
    assert_eq!(
      strip_host_prefix("{0.0.0.00000000}.{abc}"),
      "{0.0.0.00000000}.{abc}"
    );
    assert_eq!(strip_host_prefix("Foo:Bar:{guid}"), "{guid}");
  }

  /// Run with: `cargo test -p cable-tauri --lib bluetooth::win::tests::probe -- --ignored --nocapture`
  /// Lists every cpal device on the default host with the result of
  /// `resolve_bluetooth_info`. Useful for diagnosing why the BT badge
  /// doesn't appear.
  #[test]
  #[ignore]
  fn probe() {
    use cpal::traits::{DeviceTrait, HostTrait};
    let host = cpal::default_host();
    println!("Host: {:?}", host.id());
    for (label, devices) in [
      ("INPUT", host.input_devices().unwrap()),
      ("OUTPUT", host.output_devices().unwrap()),
    ] {
      for d in devices {
        let id = match d.id() {
          Ok(i) => i.to_string(),
          Err(_) => continue,
        };
        let name = d.name().unwrap_or_else(|_| "<no name>".into());
        println!("\n[{}] {} :: {}", label, id, name);
        match super::resolve_bluetooth_info(&id) {
          Some(info) => println!("  -> BT: {:?}", info),
          None => println!("  -> not bluetooth"),
        }
      }
    }
  }

  #[test]
  fn format_mac_ok() {
    assert_eq!(
      format_mac("142876B1A2A3"),
      Some("14:28:76:B1:A2:A3".to_string())
    );
    assert_eq!(format_mac("short"), None);
  }

  #[test]
  fn format_guid_braces() {
    let g = GUID::from_values(
      0x89F1AF91,
      0x4B47,
      0x5A7C,
      [0xA0, 0xA2, 0xFD, 0x16, 0x84, 0x5A, 0x6E, 0x44],
    );
    assert_eq!(format_guid(&g), "{89F1AF91-4B47-5A7C-A0A2-FD16845A6E44}");
  }
}
