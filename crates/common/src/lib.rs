#![no_std]

/// Audio data type definition
#[repr(u32)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum AudioDataType {
  PcmInt16 = 0,
  PcmInt24 = 1,
  PcmInt32 = 2,
  Float32 = 3,
}

/// Audio channel configuration definition
#[repr(u32)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ChannelConfig {
  Mono = 1,
  Stereo = 2,
  Quad = 4,
  Surround51 = 6,
  Surround71 = 8,
}

/// Virtual device type (render vs capture)
/// Mirrors: CABLE_DEVICE_TYPE in cable_common.h
#[repr(u32)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum DeviceType {
  Render = 0,
  Capture = 1,
}

/// Audio stream format metadata
#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct AudioFormat {
  pub sample_rate: u32,
  pub channels: ChannelConfig,
  pub data_type: AudioDataType,
}

/// Ring buffer control header (placed at the start of shared memory)
#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct RingBufferHeader {
  /// Write cursor (byte offset) - updated by driver
  pub write_index: u64,
  /// Read cursor (byte offset) - updated by app
  pub read_index: u64,
  /// Total audio data buffer size in bytes
  pub buffer_size: u32,
  /// Buffer status flags (0: OK, 1: Overrun, 2: Underrun)
  pub status: u32,
  /// Active stream sample rate
  pub sample_rate: u32,
  /// Active stream channel count
  pub channels: u16,
  /// Active stream bit depth
  pub bits_per_sample: u16,
  /// Active stream sample type
  pub data_type: AudioDataType,
  /// Header magic ('CBRB')
  pub magic: u32,
}

pub const RING_BUFFER_MAGIC: u32 = 0x42524243;

/// Ring buffer status flags
pub const RING_BUFFER_STATUS_OK: u32 = 0;
pub const RING_BUFFER_STATUS_OVERRUN: u32 = 1;
pub const RING_BUFFER_STATUS_UNDERRUN: u32 = 2;

/// Device identifier (16-byte unique ID)
pub type DeviceId = [u8; 16];

/// Maximum number of dynamically created virtual devices
pub const CABLE_MAX_DYNAMIC_DEVICES: u32 = 16;

/// Virtual device create/remove/update command payload
/// Mirrors: CABLE_DEVICE_CONTROL_PAYLOAD in cable_common.h
///
/// Layout (packed):
///   Id:               [u8; 16]    = 16 bytes
///   FriendlyName:     [u16; 64]   = 128 bytes
///   DeviceType:       DeviceType  = 4 bytes (u32)
///   IsEnabled:        u8          = 1 byte
///   Persistent:       u8          = 1 byte
///   WaveSymbolicLink: [u16; 256]  = 512 bytes  (response-only, null-terminated)
///   Total: 662 bytes
#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct DeviceControlPayload {
  /// Target device unique ID
  pub id: DeviceId,
  /// Device name (Windows wide char - u16 array)
  pub friendly_name: [u16; 64],
  /// Device type: render (output) or capture (input)
  pub device_type: DeviceType,
  /// Device activation state
  pub is_enabled: u8,
  /// Persistence flag (true = survives reboot)
  pub persistent: u8,
  /// KS audio interface symbolic link returned by the driver after creation.
  /// Kernel form: `\??\SWD#MMDEVAPI#...#WaveCable_NN` (null-terminated UTF-16).
  /// Written by the driver in the CREATE response; zero in all other requests.
  pub wave_symbolic_link: [u16; 256],
}

/// IOCTL unified request packet
#[repr(C, packed)]
pub union IoctlRequest {
  pub device_control: DeviceControlPayload,
  pub format_update: AudioFormat,
  pub raw_data: [u8; 768], // Padding / future expansion (covers 662-byte DeviceControlPayload)
}

/// CTL_CODE(DeviceType, Function, Method, Access) calculation
/// = (DeviceType << 16) | (Access << 14) | (Function << 2) | Method
const fn ctl_code(device_type: u32, function: u32, method: u32, access: u32) -> u32 {
  (device_type << 16) | (access << 14) | (function << 2) | method
}

/// Custom device type for Cable driver
const CABLE_FILE_DEVICE_TYPE: u32 = 0x8000;
const METHOD_BUFFERED: u32 = 0;
const FILE_ANY_ACCESS: u32 = 0;

pub const IOCTL_CABLE_CREATE_VIRTUAL_DEVICE: u32 = ctl_code(
  CABLE_FILE_DEVICE_TYPE,
  0x0001,
  METHOD_BUFFERED,
  FILE_ANY_ACCESS,
);
pub const IOCTL_CABLE_REMOVE_VIRTUAL_DEVICE: u32 = ctl_code(
  CABLE_FILE_DEVICE_TYPE,
  0x0002,
  METHOD_BUFFERED,
  FILE_ANY_ACCESS,
);
pub const IOCTL_CABLE_SET_STREAM_FORMAT: u32 = ctl_code(
  CABLE_FILE_DEVICE_TYPE,
  0x0004,
  METHOD_BUFFERED,
  FILE_ANY_ACCESS,
);
pub const IOCTL_CABLE_MAP_RING_BUFFER: u32 = ctl_code(
  CABLE_FILE_DEVICE_TYPE,
  0x0005,
  METHOD_BUFFERED,
  FILE_ANY_ACCESS,
);
pub const IOCTL_CABLE_UNMAP_RING_BUFFER: u32 = ctl_code(
  CABLE_FILE_DEVICE_TYPE,
  0x0006,
  METHOD_BUFFERED,
  FILE_ANY_ACCESS,
);

/// Ring buffer mapping request (input for MAP_RING_BUFFER IOCTL)
/// Mirrors: CABLE_RING_BUFFER_MAP_REQUEST in cable_common.h
#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct RingBufferMapRequest {
  /// Which device's ring buffer to map
  pub device_id: DeviceId,
}

/// Ring buffer mapping response (output for MAP_RING_BUFFER IOCTL)
/// Mirrors: CABLE_RING_BUFFER_MAP_RESPONSE in cable_common.h
#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct RingBufferMapResponse {
  /// User-mode virtual address of mapped region
  pub user_address: u64,
  /// Total mapped size (header + data buffer)
  pub total_size: u32,
  /// Size of audio data portion
  pub data_buffer_size: u32,
}

/// Ring buffer unmap request (input for UNMAP_RING_BUFFER IOCTL)
/// Mirrors: CABLE_RING_BUFFER_UNMAP_REQUEST in cable_common.h
#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct RingBufferUnmapRequest {
  /// Which device's ring buffer to unmap
  pub device_id: DeviceId,
  /// The user-mode address to unmap
  pub user_address: u64,
}

#[cfg(test)]
mod tests {
  use super::*;
  use core::mem::size_of;

  #[test]
  fn abi_ring_buffer_header_size() {
    // Must match CABLE_RING_BUFFER_HEADER in cable_common.h (40 bytes)
    assert_eq!(size_of::<RingBufferHeader>(), 40);
  }

  #[test]
  fn abi_device_control_payload_size() {
    // Must match CABLE_DEVICE_CONTROL_PAYLOAD in cable_common.h (662 bytes)
    assert_eq!(size_of::<DeviceControlPayload>(), 662);
  }

  #[test]
  fn abi_ioctl_request_size() {
    // Must match CABLE_IOCTL_REQUEST in cable_common.h (768 bytes)
    assert_eq!(size_of::<IoctlRequest>(), 768);
  }

  #[test]
  fn abi_audio_format_size() {
    // Must match CABLE_AUDIO_FORMAT in cable_common.h (12 bytes)
    assert_eq!(size_of::<AudioFormat>(), 12);
  }

  #[test]
  fn abi_ring_buffer_map_request_size() {
    assert_eq!(size_of::<RingBufferMapRequest>(), 16);
  }

  #[test]
  fn abi_ring_buffer_map_response_size() {
    assert_eq!(size_of::<RingBufferMapResponse>(), 16);
  }

  #[test]
  fn abi_ring_buffer_unmap_request_size() {
    assert_eq!(size_of::<RingBufferUnmapRequest>(), 24);
  }

  #[test]
  fn ioctl_code_values() {
    // CTL_CODE(0x8000, 0x0001, 0, 0)  = 0x80000004
    assert_eq!(IOCTL_CABLE_CREATE_VIRTUAL_DEVICE, 0x8000_0004);
    // CTL_CODE(0x8000, 0x0002, 0, 0)  = 0x80000008
    assert_eq!(IOCTL_CABLE_REMOVE_VIRTUAL_DEVICE, 0x8000_0008);
    // CTL_CODE(0x8000, 0x0004, 0, 0)  = 0x80000010
    assert_eq!(IOCTL_CABLE_SET_STREAM_FORMAT, 0x8000_0010);
    // CTL_CODE(0x8000, 0x0005, 0, 0)  = 0x80000014
    assert_eq!(IOCTL_CABLE_MAP_RING_BUFFER, 0x8000_0014);
    // CTL_CODE(0x8000, 0x0006, 0, 0)  = 0x80000018
    assert_eq!(IOCTL_CABLE_UNMAP_RING_BUFFER, 0x8000_0018);
  }

  #[test]
  fn ring_buffer_magic_value() {
    // 'CBRB' in little-endian
    assert_eq!(RING_BUFFER_MAGIC, 0x42524243);
  }

  #[test]
  fn device_type_discriminants() {
    assert_eq!(DeviceType::Render as u32, 0);
    assert_eq!(DeviceType::Capture as u32, 1);
  }

  #[test]
  fn audio_data_type_discriminants() {
    assert_eq!(AudioDataType::PcmInt16 as u32, 0);
    assert_eq!(AudioDataType::PcmInt24 as u32, 1);
    assert_eq!(AudioDataType::PcmInt32 as u32, 2);
    assert_eq!(AudioDataType::Float32 as u32, 3);
  }

  #[test]
  fn channel_config_discriminants() {
    assert_eq!(ChannelConfig::Mono as u32, 1);
    assert_eq!(ChannelConfig::Stereo as u32, 2);
    assert_eq!(ChannelConfig::Quad as u32, 4);
    assert_eq!(ChannelConfig::Surround51 as u32, 6);
    assert_eq!(ChannelConfig::Surround71 as u32, 8);
  }
}
