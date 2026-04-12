/// CableAudio driver IOCTL client.
///
/// Opens the kernel driver's device interface via SetupDi and sends
/// DeviceIoControl commands for virtual device management and ring
/// buffer mapping.
use std::mem;
use std::ptr;

use windows::Win32::Devices::DeviceAndDriverInstallation::{
  DIGCF_DEVICEINTERFACE, DIGCF_PRESENT, SP_DEVICE_INTERFACE_DATA,
  SP_DEVICE_INTERFACE_DETAIL_DATA_W, SP_DEVINFO_DATA, SetupDiDestroyDeviceInfoList,
  SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW, SetupDiGetDeviceInterfaceDetailW,
};
use windows::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows::Win32::Storage::FileSystem::{
  CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::IO::DeviceIoControl;
use windows::core::GUID;

use common::{
  DeviceControlPayload, DeviceId, DeviceType, IOCTL_CABLE_CREATE_VIRTUAL_DEVICE,
  IOCTL_CABLE_MAP_RING_BUFFER, IOCTL_CABLE_REMOVE_VIRTUAL_DEVICE, IOCTL_CABLE_UNMAP_RING_BUFFER,
  RingBufferHeader, RingBufferMapRequest, RingBufferMapResponse, RingBufferUnmapRequest,
};

/// Result of a successful `create_virtual_device` IOCTL.
pub struct CreatedDevice {
  /// The 16-byte device ID assigned by the driver.
  pub id: DeviceId,
}

/// Device interface GUID matching the driver's GUID_CABLE_CONTROL_INTERFACE.
/// {A3F2E8B1-7C4D-4F5A-9E6B-1D2C3A4B5E6F}
const GUID_CABLE_CONTROL_INTERFACE: GUID = GUID::from_values(
  0xa3f2e8b1,
  0x7c4d,
  0x4f5a,
  [0x9e, 0x6b, 0x1d, 0x2c, 0x3a, 0x4b, 0x5e, 0x6f],
);

/// Handle to an open CableAudio driver device.
pub struct DriverHandle {
  handle: HANDLE,
}

// SAFETY: The HANDLE is owned exclusively and DeviceIoControl is thread-safe
// when each call uses its own buffers (no shared mutable state).
unsafe impl Send for DriverHandle {}
unsafe impl Sync for DriverHandle {}

impl Drop for DriverHandle {
  fn drop(&mut self) {
    if !self.handle.is_invalid() {
      unsafe {
        let _ = CloseHandle(self.handle);
      }
    }
  }
}

impl DriverHandle {
  /// Open a handle to the CableAudio driver via its device interface GUID.
  ///
  /// Uses SetupDi APIs to enumerate device interfaces and find the first
  /// matching device path, then opens it with CreateFileW.
  pub fn open() -> Result<Self, String> {
    unsafe {
      // Get device info set for our interface class
      let dev_info = SetupDiGetClassDevsW(
        Some(&GUID_CABLE_CONTROL_INTERFACE),
        None,
        None,
        DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
      )
      .map_err(|e| format!("SetupDiGetClassDevsW failed: {}", e))?;

      // Enumerate the first device interface
      let mut iface_data = SP_DEVICE_INTERFACE_DATA {
        cbSize: mem::size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
        ..Default::default()
      };

      let enum_result = SetupDiEnumDeviceInterfaces(
        dev_info,
        None,
        &GUID_CABLE_CONTROL_INTERFACE,
        0,
        &mut iface_data,
      );

      if enum_result.is_err() {
        let _ = SetupDiDestroyDeviceInfoList(dev_info);
        return Err(
          "CableAudio driver not found. Is the driver installed and running?".to_string(),
        );
      }

      // Get required buffer size for detail data
      let mut required_size: u32 = 0;
      let _ = SetupDiGetDeviceInterfaceDetailW(
        dev_info,
        &iface_data,
        None,
        0,
        Some(&mut required_size),
        None,
      );

      if required_size == 0 {
        let _ = SetupDiDestroyDeviceInfoList(dev_info);
        return Err("Failed to get device interface detail size".to_string());
      }

      // Allocate buffer and set cbSize for the detail struct.
      // SP_DEVICE_INTERFACE_DETAIL_DATA_W has cbSize + DevicePath[1].
      // cbSize must be set to the struct size (not the allocation size).
      let mut detail_buf: Vec<u8> = vec![0u8; required_size as usize];
      let detail_data = detail_buf.as_mut_ptr() as *mut SP_DEVICE_INTERFACE_DETAIL_DATA_W;
      (*detail_data).cbSize = mem::size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;

      let mut dev_info_data = SP_DEVINFO_DATA {
        cbSize: mem::size_of::<SP_DEVINFO_DATA>() as u32,
        ..Default::default()
      };

      SetupDiGetDeviceInterfaceDetailW(
        dev_info,
        &iface_data,
        Some(detail_data),
        required_size,
        None,
        Some(&mut dev_info_data),
      )
      .map_err(|e| {
        let _ = SetupDiDestroyDeviceInfoList(dev_info);
        format!("SetupDiGetDeviceInterfaceDetailW failed: {}", e)
      })?;

      // Extract the device path (null-terminated wide string after cbSize)
      let device_path_ptr = &(*detail_data).DevicePath as *const u16;
      let device_path_len = {
        let mut len = 0usize;
        while *device_path_ptr.add(len) != 0 {
          len += 1;
        }
        len
      };
      let device_path =
        String::from_utf16_lossy(std::slice::from_raw_parts(device_path_ptr, device_path_len));

      let _ = SetupDiDestroyDeviceInfoList(dev_info);

      println!("CableAudio device path: {}", device_path);

      // Open the device
      let device_path_wide: Vec<u16> = device_path
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
      let handle = CreateFileW(
        windows::core::PCWSTR(device_path_wide.as_ptr()),
        (0x80000000u32 | 0x40000000u32).into(), // GENERIC_READ | GENERIC_WRITE
        FILE_SHARE_READ | FILE_SHARE_WRITE,
        None,
        OPEN_EXISTING,
        FILE_ATTRIBUTE_NORMAL,
        None,
      )
      .map_err(|e| format!("CreateFileW failed: {}", e))?;

      if handle == INVALID_HANDLE_VALUE {
        return Err("CreateFileW returned INVALID_HANDLE_VALUE".to_string());
      }

      Ok(Self { handle })
    }
  }

  /// Send a raw IOCTL to the driver.
  fn ioctl(
    &self,
    code: u32,
    input: *const u8,
    input_len: u32,
    output: *mut u8,
    output_len: u32,
  ) -> Result<u32, String> {
    let mut bytes_returned: u32 = 0;
    unsafe {
      DeviceIoControl(
        self.handle,
        code,
        Some(input as *const _),
        input_len,
        Some(output as *mut _),
        output_len,
        Some(&mut bytes_returned),
        None,
      )
      .map_err(|e| format!("DeviceIoControl(0x{:08X}) failed: {}", code, e))?;
    }
    Ok(bytes_returned)
  }

  /// Create a virtual audio device in the driver.
  ///
  /// Returns a `CreatedDevice` containing the driver-assigned device ID and
  /// the KS audio interface symbolic link for endpoint discovery.
  pub fn create_virtual_device(
    &self,
    friendly_name: &str,
    device_type: DeviceType,
  ) -> Result<CreatedDevice, String> {
    let mut name_wide = [0u16; 64];
    for (i, ch) in friendly_name.encode_utf16().take(63).enumerate() {
      name_wide[i] = ch;
    }

    let payload = DeviceControlPayload {
      id: [0u8; 16], // driver will assign
      friendly_name: name_wide,
      device_type,
      is_enabled: 1,
      persistent: 0,
      wave_symbolic_link: [0u16; 256],
    };

    let mut response = DeviceControlPayload {
      id: [0u8; 16],
      friendly_name: [0u16; 64],
      device_type: DeviceType::Render,
      is_enabled: 0,
      persistent: 0,
      wave_symbolic_link: [0u16; 256],
    };

    let _bytes = self.ioctl(
      IOCTL_CABLE_CREATE_VIRTUAL_DEVICE,
      &payload as *const _ as *const u8,
      mem::size_of::<DeviceControlPayload>() as u32,
      &mut response as *mut _ as *mut u8,
      mem::size_of::<DeviceControlPayload>() as u32,
    )?;

    Ok(CreatedDevice { id: response.id })
  }

  /// Remove a virtual audio device from the driver.
  pub fn remove_virtual_device(&self, device_id: &DeviceId) -> Result<(), String> {
    let payload = DeviceControlPayload {
      id: *device_id,
      friendly_name: [0u16; 64],
      device_type: DeviceType::Render,
      is_enabled: 0,
      persistent: 0,
      wave_symbolic_link: [0u16; 256],
    };

    self.ioctl(
      IOCTL_CABLE_REMOVE_VIRTUAL_DEVICE,
      &payload as *const _ as *const u8,
      mem::size_of::<DeviceControlPayload>() as u32,
      ptr::null_mut(),
      0,
    )?;

    Ok(())
  }

  /// Map a device's ring buffer into this process's address space.
  ///
  /// Returns (user_address, total_size, data_buffer_size).
  pub fn map_ring_buffer(&self, device_id: &DeviceId) -> Result<RingBufferMapping, String> {
    let request = RingBufferMapRequest {
      device_id: *device_id,
    };

    let mut response = RingBufferMapResponse {
      user_address: 0,
      total_size: 0,
      data_buffer_size: 0,
    };

    let bytes = self.ioctl(
      IOCTL_CABLE_MAP_RING_BUFFER,
      &request as *const _ as *const u8,
      mem::size_of::<RingBufferMapRequest>() as u32,
      &mut response as *mut _ as *mut u8,
      mem::size_of::<RingBufferMapResponse>() as u32,
    )?;

    if bytes < mem::size_of::<RingBufferMapResponse>() as u32 {
      return Err(format!(
        "MAP_RING_BUFFER returned {} bytes, expected {}",
        bytes,
        mem::size_of::<RingBufferMapResponse>()
      ));
    }

    if response.user_address == 0 {
      return Err("MAP_RING_BUFFER returned null address".to_string());
    }

    Ok(RingBufferMapping {
      user_address: response.user_address as *mut u8,
      total_size: response.total_size as usize,
      data_buffer_size: response.data_buffer_size as usize,
    })
  }

  /// Unmap a previously mapped ring buffer.
  pub fn unmap_ring_buffer(
    &self,
    device_id: &DeviceId,
    user_address: *mut u8,
  ) -> Result<(), String> {
    let request = RingBufferUnmapRequest {
      device_id: *device_id,
      user_address: user_address as u64,
    };

    self.ioctl(
      IOCTL_CABLE_UNMAP_RING_BUFFER,
      &request as *const _ as *const u8,
      mem::size_of::<RingBufferUnmapRequest>() as u32,
      ptr::null_mut(),
      0,
    )?;

    Ok(())
  }
}

/// Represents a mapped ring buffer in user-mode address space.
///
/// The memory layout is: [RingBufferHeader][audio data buffer]
/// The header contains write_index, read_index, buffer_size, status.
///
/// For RENDER devices (output from Windows apps):
///   - Driver writes audio data, app reads from the ring buffer
///   - App should read from read_index, driver advances write_index
///
/// For CAPTURE devices (input to Windows apps):
///   - App writes audio data, driver reads from the ring buffer
///   - App advances write_index, driver reads from read_index
pub struct RingBufferMapping {
  pub user_address: *mut u8,
  pub(crate) total_size: usize,
  pub(crate) data_buffer_size: usize,
}

// SAFETY: The mapped memory is process-global and can be accessed from any thread.
// Synchronization is handled via the atomic indices in RingBufferHeader.
unsafe impl Send for RingBufferMapping {}
unsafe impl Sync for RingBufferMapping {}

impl RingBufferMapping {
  /// Get a pointer to the start of the audio data buffer (after the header).
  fn data_ptr(&self) -> *mut u8 {
    unsafe { self.user_address.add(mem::size_of::<RingBufferHeader>()) }
  }

  // -- Raw pointer helpers for packed RingBufferHeader fields --
  // RingBufferHeader is repr(C, packed), so we must not take references
  // to its fields. Instead we compute raw pointers and use read/write_unaligned.

  fn write_index_ptr(&self) -> *mut u64 {
    // write_index is at offset 0 in RingBufferHeader
    self.user_address as *mut u64
  }

  fn read_index_ptr(&self) -> *mut u64 {
    // read_index is at offset 8 in RingBufferHeader
    unsafe { (self.user_address as *mut u64).add(1) }
  }

  fn buffer_size_ptr(&self) -> *const u32 {
    // buffer_size is at offset 16 in RingBufferHeader
    unsafe { self.user_address.add(16) as *const u32 }
  }

  fn read_buf_size(&self) -> u64 {
    unsafe { ptr::read_volatile(self.buffer_size_ptr()) as u64 }
  }

  /// Write f32 samples into the ring buffer (for capture devices - app is producer).
  ///
  /// Converts f32 samples to the byte representation and writes them into
  /// the circular buffer, advancing write_index.
  pub fn write_f32_samples(&mut self, samples: &[f32]) {
    let buf_size = self.read_buf_size();
    if buf_size == 0 {
      return;
    }

    let data = self.data_ptr();
    let bytes: &[u8] =
      unsafe { std::slice::from_raw_parts(samples.as_ptr() as *const u8, samples.len() * 4) };

    let write_idx = unsafe { ptr::read_volatile(self.write_index_ptr()) };

    for (i, &byte) in bytes.iter().enumerate() {
      let offset = ((write_idx + i as u64) % buf_size) as usize;
      unsafe {
        ptr::write_volatile(data.add(offset), byte);
      }
    }

    // Memory barrier before updating write index
    std::sync::atomic::fence(std::sync::atomic::Ordering::Release);

    let new_write_idx = write_idx + bytes.len() as u64;
    unsafe {
      ptr::write_volatile(self.write_index_ptr(), new_write_idx);
    }
  }

  /// Read f32 samples from the ring buffer (for render devices - app is consumer).
  ///
  /// Reads available bytes from the circular buffer and converts to f32.
  /// Returns the number of samples read.
  pub fn read_f32_samples(&mut self, out: &mut [f32]) -> usize {
    let buf_size = self.read_buf_size();
    if buf_size == 0 {
      return 0;
    }

    let data = self.data_ptr();
    let write_idx = unsafe { ptr::read_volatile(self.write_index_ptr()) };
    let read_idx = unsafe { ptr::read_volatile(self.read_index_ptr()) };

    // Memory barrier after reading indices
    std::sync::atomic::fence(std::sync::atomic::Ordering::Acquire);

    let available_bytes = (write_idx.wrapping_sub(read_idx)) as usize;
    let available_samples = available_bytes / 4;
    let samples_to_read = available_samples.min(out.len());

    if samples_to_read == 0 {
      return 0;
    }

    let bytes_to_read = samples_to_read * 4;
    let mut byte_buf = vec![0u8; bytes_to_read];

    for (i, byte) in byte_buf.iter_mut().enumerate() {
      let offset = ((read_idx + i as u64) % buf_size) as usize;
      *byte = unsafe { ptr::read_volatile(data.add(offset)) };
    }

    // Convert bytes to f32 samples
    for (i, sample) in out.iter_mut().take(samples_to_read).enumerate() {
      let offset = i * 4;
      *sample = f32::from_le_bytes([
        byte_buf[offset],
        byte_buf[offset + 1],
        byte_buf[offset + 2],
        byte_buf[offset + 3],
      ]);
    }

    // Memory barrier before updating read index
    std::sync::atomic::fence(std::sync::atomic::Ordering::Release);

    let new_read_idx = read_idx + bytes_to_read as u64;
    unsafe {
      ptr::write_volatile(self.read_index_ptr(), new_read_idx);
    }

    samples_to_read
  }
}
