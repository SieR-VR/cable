/*++

Module Name:

    cable_common.h

Abstract:

    Shared type definitions mirroring crates/common/src/lib.rs.
    These types define the IOCTL interface between the Cable kernel driver
    and the Cable Tauri application (user-mode).

    IMPORTANT: This file MUST be kept in sync with crates/common/src/lib.rs.
    Any changes to the Rust definitions must be reflected here, and vice versa.

--*/

#ifndef _CABLE_COMMON_H_
#define _CABLE_COMMON_H_

// When building as part of the kernel driver, ntdef.h is already included
// via definitions.h -> portcls.h. When building standalone (e.g., for
// verification), define the basic types we need.
#ifndef _NTDEF_
#include <stdint.h>
typedef uint8_t     UINT8;
typedef uint32_t    UINT32;
typedef uint64_t    UINT64;
typedef wchar_t     WCHAR;
typedef unsigned char BOOLEAN;
#ifndef CTL_CODE
#define CTL_CODE(DeviceType, Function, Method, Access) \
    (((DeviceType) << 16) | ((Access) << 14) | ((Function) << 2) | (Method))
#endif
#define METHOD_BUFFERED     0
#define FILE_ANY_ACCESS     0
#endif // _NTDEF_

#pragma pack(push, 1)

//=============================================================================
// Audio data type enumeration
// Mirrors: AudioDataType in lib.rs
//=============================================================================
typedef enum _CABLE_AUDIO_DATA_TYPE {
    CableAudioDataPcmInt16  = 0,
    CableAudioDataPcmInt24  = 1,
    CableAudioDataPcmInt32  = 2,
    CableAudioDataFloat32   = 3,
} CABLE_AUDIO_DATA_TYPE;

//=============================================================================
// Channel configuration enumeration
// Mirrors: ChannelConfig in lib.rs
//=============================================================================
typedef enum _CABLE_CHANNEL_CONFIG {
    CableChannelMono        = 1,
    CableChannelStereo      = 2,
    CableChannelQuad        = 4,
    CableChannelSurround51  = 6,
    CableChannelSurround71  = 8,
} CABLE_CHANNEL_CONFIG;

//=============================================================================
// Audio format descriptor
// Mirrors: AudioFormat in lib.rs
//=============================================================================
typedef struct _CABLE_AUDIO_FORMAT {
    UINT32                  SampleRate;
    CABLE_CHANNEL_CONFIG    Channels;       // UINT32
    CABLE_AUDIO_DATA_TYPE   DataType;       // UINT32
} CABLE_AUDIO_FORMAT, *PCABLE_AUDIO_FORMAT;

//=============================================================================
// Ring buffer header (placed at the start of shared memory section)
// Mirrors: RingBufferHeader in lib.rs
//
// Layout in shared memory:
//   [RingBufferHeader][...audio data buffer...]
//
// write_index and read_index are byte offsets into the audio data buffer
// (which starts immediately after the header).
//=============================================================================
typedef struct _CABLE_RING_BUFFER_HEADER {
    UINT64  WriteIndex;     // Driver write cursor (byte offset)
    UINT64  ReadIndex;      // App read cursor (byte offset)
    UINT32  BufferSize;     // Total audio data buffer size in bytes
    UINT32  Status;         // 0: OK, 1: Overrun, 2: Underrun
} CABLE_RING_BUFFER_HEADER, *PCABLE_RING_BUFFER_HEADER;

#define CABLE_RING_BUFFER_STATUS_OK         0
#define CABLE_RING_BUFFER_STATUS_OVERRUN    1
#define CABLE_RING_BUFFER_STATUS_UNDERRUN   2

//=============================================================================
// Device identifier (16-byte unique ID)
// Mirrors: DeviceId = [u8; 16] in lib.rs
//=============================================================================
#define CABLE_DEVICE_ID_SIZE    16
typedef UINT8 CABLE_DEVICE_ID[CABLE_DEVICE_ID_SIZE];

//=============================================================================
// Virtual device type (render vs capture)
//=============================================================================
typedef enum _CABLE_DEVICE_TYPE {
    CableDeviceTypeRender   = 0,    // Output device (speaker-like)
    CableDeviceTypeCapture  = 1,    // Input device (microphone-like)
} CABLE_DEVICE_TYPE;

//=============================================================================
// Maximum number of dynamically created virtual devices.
// Each device uses 2 subdevice slots (topology + wave).
//=============================================================================
#define CABLE_MAX_DYNAMIC_DEVICES       16
#define CABLE_MAX_DYNAMIC_SUBDEVICES    (CABLE_MAX_DYNAMIC_DEVICES * 2)

//=============================================================================
// Device control payload for create/remove/update commands
// Mirrors: DeviceControlPayload in lib.rs
//=============================================================================
typedef struct _CABLE_DEVICE_CONTROL_PAYLOAD {
    CABLE_DEVICE_ID     Id;                     // 16 bytes: unique device identifier
    WCHAR               FriendlyName[64];       // Wide char device name (matches u16 array)
    CABLE_DEVICE_TYPE   DeviceType;             // Render or Capture
    BOOLEAN             IsEnabled;              // Device activation state
    BOOLEAN             Persistent;             // Survives reboot if TRUE
} CABLE_DEVICE_CONTROL_PAYLOAD, *PCABLE_DEVICE_CONTROL_PAYLOAD;

//=============================================================================
// IOCTL request union
// Mirrors: IoctlRequest in lib.rs
//
// The union allows the same buffer to carry different payloads depending
// on the IOCTL code.
//=============================================================================
typedef union _CABLE_IOCTL_REQUEST {
    CABLE_DEVICE_CONTROL_PAYLOAD    DeviceControl;
    CABLE_AUDIO_FORMAT              FormatUpdate;
    UINT8                           RawData[256];   // Padding / future expansion
} CABLE_IOCTL_REQUEST, *PCABLE_IOCTL_REQUEST;

#pragma pack(pop)

//=============================================================================
// IOCTL codes
// Mirrors: IOCTL_* constants in lib.rs
//
// These use FILE_DEVICE_UNKNOWN (0x22) with METHOD_BUFFERED and FILE_ANY_ACCESS.
// CTL_CODE(DeviceType, Function, Method, Access)
//=============================================================================
#define CABLE_FILE_DEVICE_TYPE      0x00008000  // Custom device type for Cable

#define IOCTL_CABLE_CREATE_VIRTUAL_DEVICE   \
    CTL_CODE(CABLE_FILE_DEVICE_TYPE, 0x0001, METHOD_BUFFERED, FILE_ANY_ACCESS)

#define IOCTL_CABLE_REMOVE_VIRTUAL_DEVICE   \
    CTL_CODE(CABLE_FILE_DEVICE_TYPE, 0x0002, METHOD_BUFFERED, FILE_ANY_ACCESS)

#define IOCTL_CABLE_UPDATE_DEVICE_NAME      \
    CTL_CODE(CABLE_FILE_DEVICE_TYPE, 0x0003, METHOD_BUFFERED, FILE_ANY_ACCESS)

#define IOCTL_CABLE_SET_STREAM_FORMAT       \
    CTL_CODE(CABLE_FILE_DEVICE_TYPE, 0x0004, METHOD_BUFFERED, FILE_ANY_ACCESS)

#define IOCTL_CABLE_MAP_RING_BUFFER         \
    CTL_CODE(CABLE_FILE_DEVICE_TYPE, 0x0005, METHOD_BUFFERED, FILE_ANY_ACCESS)

#define IOCTL_CABLE_UNMAP_RING_BUFFER       \
    CTL_CODE(CABLE_FILE_DEVICE_TYPE, 0x0006, METHOD_BUFFERED, FILE_ANY_ACCESS)

//=============================================================================
// Ring buffer mapping request payload (input for MAP_RING_BUFFER IOCTL)
// Sent by user-mode to request mapping of a device's ring buffer.
//=============================================================================
#pragma pack(push, 1)
typedef struct _CABLE_RING_BUFFER_MAP_REQUEST {
    CABLE_DEVICE_ID     DeviceId;       // Which device's ring buffer to map
} CABLE_RING_BUFFER_MAP_REQUEST, *PCABLE_RING_BUFFER_MAP_REQUEST;

//=============================================================================
// Ring buffer mapping response payload (output for MAP_RING_BUFFER IOCTL)
// Returned to user-mode with the mapped address and size info.
//=============================================================================
typedef struct _CABLE_RING_BUFFER_MAP_RESPONSE {
    UINT64              UserAddress;    // User-mode virtual address of mapped region
    UINT32              TotalSize;      // Total mapped size (header + data buffer)
    UINT32              DataBufferSize; // Size of audio data portion
} CABLE_RING_BUFFER_MAP_RESPONSE, *PCABLE_RING_BUFFER_MAP_RESPONSE;

//=============================================================================
// Ring buffer unmap request payload (input for UNMAP_RING_BUFFER IOCTL)
//=============================================================================
typedef struct _CABLE_RING_BUFFER_UNMAP_REQUEST {
    CABLE_DEVICE_ID     DeviceId;       // Which device's ring buffer to unmap
    UINT64              UserAddress;    // The user-mode address to unmap
} CABLE_RING_BUFFER_UNMAP_REQUEST, *PCABLE_RING_BUFFER_UNMAP_REQUEST;
#pragma pack(pop)

//=============================================================================
// Computed IOCTL values for cross-reference with Rust side:
//
// IOCTL_CABLE_CREATE_VIRTUAL_DEVICE = CTL_CODE(0x8000, 0x0001, 0, 0)
//   = (0x8000 << 16) | (0 << 14) | (0x0001 << 2) | 0
//   = 0x80000004
//
// IOCTL_CABLE_REMOVE_VIRTUAL_DEVICE = 0x80000008
// IOCTL_CABLE_UPDATE_DEVICE_NAME    = 0x8000000C
// IOCTL_CABLE_SET_STREAM_FORMAT     = 0x80000010
// IOCTL_CABLE_MAP_RING_BUFFER       = 0x80000014
// IOCTL_CABLE_UNMAP_RING_BUFFER     = 0x80000018
//=============================================================================

#endif // _CABLE_COMMON_H_
