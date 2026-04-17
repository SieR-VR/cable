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
  AudioDataType, DeviceControlPayload, DeviceId, DeviceType, IOCTL_CABLE_CREATE_VIRTUAL_DEVICE,
  IOCTL_CABLE_MAP_RING_BUFFER, IOCTL_CABLE_REMOVE_VIRTUAL_DEVICE, IOCTL_CABLE_UNMAP_RING_BUFFER,
  RING_BUFFER_MAGIC, RingBufferHeader, RingBufferMapRequest, RingBufferMapResponse,
  RingBufferUnmapRequest,
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
/// The header contains write_index, read_index, buffer_size, status, and
/// stream format metadata (sample_rate, channels, bits_per_sample, data_type, magic).
///
/// For RENDER devices (output from Windows apps):
///   - Driver writes audio data and advances write_index.
///   - App reads data starting at read_index and advances read_index.
///
/// For CAPTURE devices (input to Windows apps):
///   - App writes audio data and advances write_index.
///   - Driver reads data starting at read_index and advances read_index.
///
/// # Safety note on packed struct field access
///
/// `RingBufferHeader` is `#[repr(C, packed)]`, so taking a reference to any of
/// its fields is undefined behaviour (the field may not be naturally aligned).
/// All field reads and writes must go through `ptr::addr_of!` combined with
/// `ptr::read_unaligned` / `ptr::write_unaligned`.  Where volatile semantics
/// are required (to prevent the compiler from caching across loop iterations)
/// the two concerns are split:
///   1. Compute the raw field pointer via `ptr::addr_of!`.
///   2. Perform a `ptr::read_volatile` / `ptr::write_volatile` through that
///      pointer after casting to the concrete type.
/// This is valid because `read_volatile`/`write_volatile` do not require the
/// pointer to be aligned – they only require it to be non-null and in-bounds.
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
  /// Size of the ring buffer header in bytes.
  ///
  /// `RingBufferHeader` is always placed at the very start of the mapping.
  /// We use `mem::size_of` so that if the struct grows this stays correct
  /// automatically.  The old heuristic (total_size – data_buffer_size) was
  /// fragile because the driver fills in `total_size` / `data_buffer_size`
  /// independently, and any off-by-one between the C and Rust definitions
  /// would have caused all subsequent field reads to land at the wrong offset.
  #[inline]
  fn header_size() -> usize {
    mem::size_of::<RingBufferHeader>()
  }

  /// Pointer to the first byte of the audio data buffer (immediately after the header).
  #[inline]
  fn data_ptr(&self) -> *mut u8 {
    unsafe { self.user_address.add(Self::header_size()) }
  }

  /// Typed pointer to the RingBufferHeader at the start of the mapping.
  ///
  /// Callers must never dereference this pointer directly or take a reference
  /// to its fields.  Use `ptr::addr_of!((*hdr).field)` + `read/write_unaligned`.
  #[inline]
  fn header_ptr(&self) -> *mut RingBufferHeader {
    self.user_address as *mut RingBufferHeader
  }

  /// Read `buffer_size` from the header (volatile, unaligned-safe).
  fn read_buf_size(&self) -> u64 {
    unsafe {
      let hdr = self.header_ptr();
      ptr::read_volatile(ptr::addr_of!((*hdr).buffer_size) as *const u32) as u64
    }
  }

  /// Read `write_index` from the header (volatile, unaligned-safe).
  fn read_write_index(&self) -> u64 {
    unsafe {
      let hdr = self.header_ptr();
      ptr::read_volatile(ptr::addr_of!((*hdr).write_index) as *const u64)
    }
  }

  /// Read `read_index` from the header (volatile, unaligned-safe).
  fn read_read_index(&self) -> u64 {
    unsafe {
      let hdr = self.header_ptr();
      ptr::read_volatile(ptr::addr_of!((*hdr).read_index) as *const u64)
    }
  }

  /// Write `write_index` to the header (volatile, unaligned-safe).
  fn write_write_index(&self, value: u64) {
    unsafe {
      let hdr = self.header_ptr();
      ptr::write_volatile(ptr::addr_of_mut!((*hdr).write_index) as *mut u64, value);
    }
  }

  /// Write `read_index` to the header (volatile, unaligned-safe).
  fn write_read_index(&self, value: u64) {
    unsafe {
      let hdr = self.header_ptr();
      ptr::write_volatile(ptr::addr_of_mut!((*hdr).read_index) as *mut u64, value);
    }
  }

  /// Read stream format metadata from the header.
  ///
  /// Returns `Some((sample_rate, channels, bits_per_sample, data_type))` if
  /// the magic cookie is present and the core fields are non-zero, otherwise
  /// `None` (header not yet initialised by the driver stream).
  pub fn read_stream_format_metadata(&self) -> Option<(u32, u16, u16, AudioDataType)> {
    unsafe {
      let hdr = self.header_ptr();
      let magic = ptr::read_unaligned(ptr::addr_of!((*hdr).magic) as *const u32);
      if magic != RING_BUFFER_MAGIC {
        return None;
      }

      let sample_rate = ptr::read_unaligned(ptr::addr_of!((*hdr).sample_rate) as *const u32);
      let channels = ptr::read_unaligned(ptr::addr_of!((*hdr).channels) as *const u16);
      let bits = ptr::read_unaligned(ptr::addr_of!((*hdr).bits_per_sample) as *const u16);
      let data_type = ptr::read_unaligned(ptr::addr_of!((*hdr).data_type) as *const AudioDataType);

      if sample_rate == 0 || channels == 0 || bits == 0 {
        return None;
      }

      Some((sample_rate, channels, bits, data_type))
    }
  }

  /// Return `(write_index, read_index, buffer_size, status)` for diagnostics.
  pub fn debug_ring_stats(&self) -> (u64, u64, u32, u32) {
    unsafe {
      let hdr = self.header_ptr();
      let w = ptr::read_volatile(ptr::addr_of!((*hdr).write_index) as *const u64);
      let r = ptr::read_volatile(ptr::addr_of!((*hdr).read_index) as *const u64);
      let size = ptr::read_volatile(ptr::addr_of!((*hdr).buffer_size) as *const u32);
      let status = ptr::read_volatile(ptr::addr_of!((*hdr).status) as *const u32);
      (w, r, size, status)
    }
  }

  /// Write f32 samples into the ring buffer (capture path – app is the producer).
  ///
  /// The samples are reinterpreted as raw bytes and written into the circular
  /// data buffer starting at the current `write_index`.  After writing,
  /// `write_index` is advanced and a `Release` fence is issued so the kernel
  /// consumer sees the data before the updated index.
  pub fn write_f32_samples(&mut self, samples: &[f32]) {
    let buf_size = self.read_buf_size();
    if buf_size == 0 {
      return;
    }

    let data = self.data_ptr();
    let bytes: &[u8] =
      unsafe { std::slice::from_raw_parts(samples.as_ptr() as *const u8, samples.len() * 4) };

    let write_idx = self.read_write_index();

    for (i, &byte) in bytes.iter().enumerate() {
      let offset = ((write_idx + i as u64) % buf_size) as usize;
      unsafe {
        ptr::write_volatile(data.add(offset), byte);
      }
    }

    // Release fence: all data writes must be visible before the index update.
    std::sync::atomic::fence(std::sync::atomic::Ordering::Release);

    self.write_write_index(write_idx + bytes.len() as u64);
  }

  /// Read f32 samples from the ring buffer (render path – app is the consumer).
  ///
  /// Reads however many complete samples are available (up to `out.len()`),
  /// converts them from the negotiated wire format to `f32`, and returns the
  /// number of samples placed in `out`.
  ///
  /// # Overrun handling
  ///
  /// If the driver has produced more data than the ring buffer can hold
  /// (`write_index - read_index > buf_size`), the read cursor is snapped
  /// forward to `write_index - buf_size` so that we begin consuming the
  /// most recent data instead of stale data from a previous revolution.
  /// This mirrors the overrun recovery in `CableRingBuffer::Read()`.
  pub fn read_f32_samples(&mut self, out: &mut [f32]) -> usize {
    let buf_size = self.read_buf_size();
    if buf_size == 0 {
      return 0;
    }

    let (bits_per_sample, bytes_per_sample, data_type) = match self.read_stream_format_metadata() {
      Some((_sr, _ch, bits, dt)) if bits == 16 || bits == 24 || bits == 32 => {
        let bps = (bits / 8) as usize;
        (bits, bps, dt)
      }
      _ => (32u16, 4usize, AudioDataType::Float32),
    };

    // Acquire fence: ensure we see all data written by the kernel before
    // we read write_index or any ring buffer contents. The kernel issues a
    // Release fence after advancing write_index, so this pairs with that.
    std::sync::atomic::fence(std::sync::atomic::Ordering::Acquire);

    let write_idx = self.read_write_index();

    let mut read_idx = self.read_read_index();

    // Overrun recovery: if the producer has lapped us (written more than
    // one full ring buffer ahead), the oldest data is already gone.
    // Snap the read cursor forward to the most recent data so we don't
    // read stale/corrupted bytes.  We leave one tick's worth of data
    // so the next process() call has something fresh to consume.
    let available_bytes = write_idx.saturating_sub(read_idx);
    let mut needs_fade_in = false;
    if available_bytes > buf_size {
      let target_bytes = (out.len() * bytes_per_sample) as u64;
      read_idx = write_idx.saturating_sub(target_bytes);
      self.write_read_index(read_idx);
      needs_fade_in = true;
    }

    // Re-read available_bytes after potential snap.
    let available_bytes = write_idx.saturating_sub(read_idx) as usize;
    let available_samples = available_bytes / bytes_per_sample;
    let samples_to_read = available_samples.min(out.len());

    if samples_to_read == 0 {
      return 0;
    }

    let bytes_to_read = samples_to_read * bytes_per_sample;
    let data = self.data_ptr();
    let mut byte_buf = vec![0u8; bytes_to_read];

    for (i, byte) in byte_buf.iter_mut().enumerate() {
      let offset = ((read_idx + i as u64) % buf_size) as usize;
      *byte = unsafe { ptr::read_volatile(data.add(offset)) };
    }

    // Convert wire bytes to normalised f32 samples.
    match (bits_per_sample, data_type) {
      (16, _) | (_, AudioDataType::PcmInt16) => {
        for (i, sample) in out.iter_mut().take(samples_to_read).enumerate() {
          let o = i * 2;
          let s = i16::from_le_bytes([byte_buf[o], byte_buf[o + 1]]);
          *sample = (s as f32) / 32768.0;
        }
      }
      (24, _) | (_, AudioDataType::PcmInt24) => {
        for (i, sample) in out.iter_mut().take(samples_to_read).enumerate() {
          let o = i * 3;
          let raw = (byte_buf[o] as u32)
            | ((byte_buf[o + 1] as u32) << 8)
            | ((byte_buf[o + 2] as u32) << 16);
          // Sign-extend from 24-bit.
          let signed = if raw & 0x0080_0000 != 0 {
            (raw | 0xFF00_0000) as i32
          } else {
            raw as i32
          };
          *sample = (signed as f32) / 8_388_608.0;
        }
      }
      (_, AudioDataType::PcmInt32) => {
        for (i, sample) in out.iter_mut().take(samples_to_read).enumerate() {
          let o = i * 4;
          let s = i32::from_le_bytes([
            byte_buf[o],
            byte_buf[o + 1],
            byte_buf[o + 2],
            byte_buf[o + 3],
          ]);
          *sample = (s as f32) / 2_147_483_648.0;
        }
      }
      _ => {
        // Float32 (default)
        for (i, sample) in out.iter_mut().take(samples_to_read).enumerate() {
          let o = i * 4;
          *sample = f32::from_le_bytes([
            byte_buf[o],
            byte_buf[o + 1],
            byte_buf[o + 2],
            byte_buf[o + 3],
          ]);
        }
      }
    }

    // Apply a short fade-in after overrun recovery to mask discontinuities.
    if needs_fade_in && samples_to_read > 0 {
      let fade_len = 64.min(samples_to_read);
      for i in 0..fade_len {
        out[i] *= i as f32 / fade_len as f32;
      }
    }

    // Release fence: data reads must complete before we advance the index.
    std::sync::atomic::fence(std::sync::atomic::Ordering::Release);

    self.write_read_index(read_idx + bytes_to_read as u64);

    samples_to_read
  }
}
