#![no_std]

/// 오디오 데이터 타입 정의
#[repr(u32)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum AudioDataType {
  PcmInt16 = 0,
  PcmInt24 = 1,
  PcmInt32 = 2,
  Float32 = 3,
}

/// 오디오 채널 구성 정의
#[repr(u32)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ChannelConfig {
  Mono = 1,
  Stereo = 2,
  Quad = 4,
  Surround51 = 6,
  Surround71 = 8,
}

/// 오디오 스트림의 상세 메타데이터
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct AudioFormat {
  pub sample_rate: u32,
  pub channels: ChannelConfig,
  pub data_type: AudioDataType,
}

/// 링 버퍼 제어를 위한 헤더 (공유 메모리 최상단에 위치)
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct RingBufferHeader {
  /// 드라이버가 쓴 마지막 위치 (Write Cursor)
  pub write_index: u64,
  /// 앱이 읽은 마지막 위치 (Read Cursor)
  pub read_index: u64,
  /// 전체 버퍼 크기 (Bytes)
  pub buffer_size: u32,
  /// 버퍼 상태 플래그 (0: OK, 1: Overrun, 2: Underrun)
  pub status: u32,
}

/// 장치 식별자 (고유 ID)
pub type DeviceId = [u8; 16];

/// 가상 장치 생성 및 관리 명령 구조체
#[repr(C)]
#[derive(Copy, Clone)]
pub struct DeviceControlPayload {
  /// 대상 장치의 고유 ID
  pub id: DeviceId,
  /// 장치 이름 (Windows Wide Char 대응을 위한 u16 배열)
  pub friendly_name: [u16; 64],
  /// 장치 활성화 상태
  pub is_enabled: bool,
  /// 지속성 여부 (true면 재부팅 후에도 유지)
  pub persistent: bool,
}

/// IOCTL 통신을 위한 통합 요청 패킷
#[repr(C)]
pub union IoctlRequest {
  pub device_control: DeviceControlPayload,
  pub format_update: AudioFormat,
  pub raw_data: [u8; 256], // 패딩 및 미래 확장용
}

pub const IOCTL_CREATE_VIRTUAL_DEVICE: u32 = 0x8001;
pub const IOCTL_REMOVE_VIRTUAL_DEVICE: u32 = 0x8002;
pub const IOCTL_UPDATE_DEVICE_NAME: u32 = 0x8003;
pub const IOCTL_SET_STREAM_FORMAT: u32 = 0x8004;
