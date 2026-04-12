# CableAudio Virtual Audio Driver

Windows kernel-mode virtual audio driver (PortCls WaveRT). Single monolithic `.sys` file containing both the bus driver logic and the PortCls miniport implementation.

- **WDK version**: 10.0.26100.0
- **Architecture**: x64 only
- **Solution**: `driver/CableAudio.sln`
- **Build output**: `driver/x64/Debug/package/` (CableAudio.sys, CableAudio.inf, cableaudio.cat)

## Architecture

```
+------------------------------------------------------------------+
|  CableAudio.sys                                                  |
|                                                                  |
|  DriverEntry                                                     |
|    |-- PcInitializeAdapterDriver (PortCls setup)                 |
|    |-- Hook IRP_MJ_DEVICE_CONTROL -> DeviceControlHandler        |
|    |-- Hook IRP_MJ_CREATE         -> CreateHandler               |
|    +-- Hook IRP_MJ_CLOSE          -> CloseHandler                |
|                                                                  |
|  StartDevice                                                     |
|    |-- CAdapterCommon::Init                                      |
|    |-- IoRegisterDeviceInterface(GUID_CABLE_CONTROL_INTERFACE)   |
|    |-- InstallAllRenderFilters  (static: Speakers)               |
|    +-- InstallAllCaptureFilters (static: Microphone Array)       |
|                                                                  |
|  DeviceControlHandler                                            |
|    |-- Cable IOCTL -> CAdapterCommon methods                     |
|    +-- Other IOCTL -> gPCDeviceControlHandler (PortCls)          |
+------------------------------------------------------------------+
```

### IRP Hooking

PortCls owns `IRP_MJ_DEVICE_CONTROL`, `IRP_MJ_CREATE`, `IRP_MJ_CLOSE`. The driver hooks these at `DriverEntry` to intercept Cable-specific IOCTLs before PortCls sees them.

- **CreateHandler**: If the file open has an empty filename and the control interface is enabled, tags `FileObject->FsContext` with `CABLE_CONTROL_FSCONTEXT` (`0x4341424C`, ASCII `'CABL'`) and completes the IRP immediately. Otherwise forwards to PortCls.
- **CloseHandler**: If `FsContext == CABLE_CONTROL_FSCONTEXT`, completes the IRP. Otherwise forwards to PortCls.
- **DeviceControlHandler**: Switches on Cable IOCTL codes. For unknown IOCTLs, checks if the handle is a Cable control handle (FsContext marker) and rejects them to prevent PortCls from crashing on unrecognized control handles.

### Device Interface

The control channel is exposed as a device interface with GUID:

```
GUID_CABLE_CONTROL_INTERFACE = {A3F2E8B1-7C4D-4F5A-9E6B-1D2C3A4B5E6F}
```

User-mode opens this interface via `CreateFile` on the symbolic link path. The Tauri app discovers it using `CM_Get_Device_Interface_List`.

## Static Endpoints

Two endpoints are created at driver start (PnP `StartDevice`):

| Endpoint | Type | Topo Name | Wave Name | Template |
|---|---|---|---|---|
| Speakers (Cable Virtual Audio Device) | Render | `TopologySpeaker` | `WaveSpeaker` | `SpeakerMiniports` |
| Microphone Array (Cable Virtual Audio Device) | Capture | `TopologyMicArray1` | `WaveMicArray1` | `MicArray1Miniports` |

These always exist while the driver is loaded. They appear in Windows Sound Settings as regular audio devices.

## Dynamic Virtual Devices

Up to **16 additional virtual audio devices** can be created at runtime via IOCTL. Each device occupies 2 subdevice slots (topology + wave), so up to 32 dynamic subdevices.

```c
#define CABLE_MAX_DYNAMIC_DEVICES       16
#define CABLE_MAX_DYNAMIC_SUBDEVICES    32   // 16 * 2
```

The total subdevice capacity registered with PortCls at `AddDevice`:

```c
g_MaxMiniports = (g_cRenderEndpoints + g_cCaptureEndpoints) * 2 + CABLE_MAX_DYNAMIC_SUBDEVICES
               = (1 + 1) * 2 + 32
               = 36
```

### Internal Tracking

Dynamic devices are tracked in `CAdapterCommon::m_VirtualDevices[16]`, an array of `CABLE_VIRTUAL_DEVICE_ENTRY`:

| Field | Type | Description |
|---|---|---|
| `InUse` | BOOLEAN | Slot is occupied |
| `Id` | UINT8[16] | Unique device ID |
| `DeviceType` | CABLE_DEVICE_TYPE | Render (0) or Capture (1) |
| `FriendlyName` | WCHAR[64] | Display name |
| `TopoName` | WCHAR[MAX_PATH] | e.g. `"TopologyCable_03"` |
| `WaveName` | WCHAR[MAX_PATH] | e.g. `"WaveCable_03"` |
| `UnknownTopology` | PUNKNOWN | Topology port interface |
| `UnknownWave` | PUNKNOWN | Wave port interface |
| `pRingBuffer` | CableRingBuffer* | Shared memory ring buffer |
| `pMappedUserAddress` | PVOID | Current user-mode mapping (or NULL) |

### Creation Flow (Work Item Pattern)

PortCls port initialization (`port->Init()`) must run at PASSIVE_LEVEL in a proper thread context. Since IOCTL handlers may run in an arbitrary thread context, device creation uses a system work item:

```
IOCTL thread                          System worker thread
     |                                       |
     |-- Allocate IO_WORKITEM                |
     |-- Fill context struct                 |
     |-- KeInitializeEvent(CompletionEvent)  |
     |-- Mark slot InUse                     |
     |-- IoQueueWorkItem(DelayedWorkQueue)---|-->  CableInstallEndpointWorkItem()
     |                                       |      |-- PcNewPort(CLSID_PortTopology)
     |-- KeWaitForSingleObject(...)          |      |-- miniport->Init(NULL IRP)
     |      (blocks)                         |      |-- PcRegisterSubdevice(TopoName)
     |                                       |      |-- PcNewPort(CLSID_PortWaveRT)
     |                                       |      |-- miniport->Init(NULL IRP)
     |                                       |      |-- PcRegisterSubdevice(WaveName)
     |                                       |      |-- ConnectTopologies(topo, wave)
     |                                       |      +-- KeSetEvent(CompletionEvent)
     |<-------(event signaled)---------------|
     |-- Check status                        |
     |-- IoFreeWorkItem                      |
     +-- Complete IRP                        |
```

### ID Generation

When creating a device with an all-zero ID, the driver generates one:

```
Bytes  0-2:  'C', 'B', 'L'          (magic)
Byte   3:    0x00
Bytes  4-7:  slot index              (ULONG, little-endian)
Bytes  8-11: device type             (ULONG, 0=render, 1=capture)
Bytes 12-15: incrementing counter    (InterlockedIncrement)
```

### Removal Flow

1. Find slot by device ID
2. `DisconnectTopologies` (remove physical connections)
3. `RemoveCachedSubdevice` + `UnregisterSubdevice` + `Release` for wave and topology
4. Clean up ring buffer (unmap, free)
5. Zero the slot, decrement device count

## IOCTL Interface

### IOCTL Codes

Custom device type: `CABLE_FILE_DEVICE_TYPE = 0x00008000`

All IOCTLs use `METHOD_BUFFERED` and `FILE_ANY_ACCESS`.

| IOCTL | Function | Code | Input | Output | Notes |
|---|---|---|---|---|---|
| `IOCTL_CABLE_CREATE_VIRTUAL_DEVICE` | 0x0001 | `0x80000004` | `CABLE_IOCTL_REQUEST` (768 bytes) | `CABLE_DEVICE_CONTROL_PAYLOAD` (662 bytes) | Response includes `WaveSymbolicLink` |
| `IOCTL_CABLE_REMOVE_VIRTUAL_DEVICE` | 0x0002 | `0x80000008` | `CABLE_IOCTL_REQUEST` (768 bytes) | None | |
| `IOCTL_CABLE_UPDATE_DEVICE_NAME` | 0x0003 | `0x8000000C` | `CABLE_IOCTL_REQUEST` (768 bytes) | None | Tauri 앱에서 사용하지 않음 — 이름 변경은 elevated IPropertyStore COM 호출로 수행 (`docs/endpoint-naming.md` 참고) |
| `IOCTL_CABLE_SET_STREAM_FORMAT` | 0x0004 | `0x80000010` | `CABLE_IOCTL_REQUEST` (768 bytes) | None | |
| `IOCTL_CABLE_MAP_RING_BUFFER` | 0x0005 | `0x80000014` | `CABLE_RING_BUFFER_MAP_REQUEST` (16 bytes) | `CABLE_RING_BUFFER_MAP_RESPONSE` (16 bytes) | |
| `IOCTL_CABLE_UNMAP_RING_BUFFER` | 0x0006 | `0x80000018` | `CABLE_RING_BUFFER_UNMAP_REQUEST` (24 bytes) | None | |

### IOCTL Code Calculation

```
CTL_CODE(DeviceType, Function, Method, Access)
= (DeviceType << 16) | (Access << 14) | (Function << 2) | Method

Example: CREATE_VIRTUAL_DEVICE
= (0x8000 << 16) | (0 << 14) | (1 << 2) | 0
= 0x80000000 | 0 | 4 | 0
= 0x80000004
```

### IOCTL Flow

```
User-mode (Tauri app)                   Kernel (CableAudio.sys)
     |                                       |
     |-- CreateFile(GUID_CABLE_CONTROL_      |
     |              INTERFACE symlink)  ---->  CreateHandler: tag FsContext = 'CABL'
     |                                       |
     |-- DeviceIoControl(IOCTL_CABLE_        |
     |   CREATE_VIRTUAL_DEVICE,              |
      |   inBuf=CABLE_IOCTL_REQUEST,          |
      |   outBuf=662 bytes)              ---->  DeviceControlHandler
      |                                       |   -> CableIoctl_CreateVirtualDevice
      |                                       |   -> CAdapterCommon::CreateVirtualDevice
      |                                       |   -> (work item creates subdevices)
      |  <-- outBuf filled with payload       |   <- IRP completed, Information=662
      |      (ID, name, type,                 |
      |       WaveSymbolicLink)               |
     |                                       |
     |-- DeviceIoControl(IOCTL_CABLE_        |
     |   MAP_RING_BUFFER,                    |
     |   inBuf=MAP_REQUEST(DeviceId),        |
     |   outBuf=16 bytes)               ---->  CableIoctl_MapRingBuffer
     |                                       |   -> CAdapterCommon::MapRingBuffer
     |                                       |   -> CableRingBuffer::MapToUserMode
     |  <-- outBuf = MAP_RESPONSE            |   <- MmMapLockedPagesSpecifyCache
     |      (UserAddress, TotalSize,         |
     |       DataBufferSize)                 |
     |                                       |
     |-- (read/write shared memory           |
     |    directly via UserAddress)          |
     |                                       |
     |-- DeviceIoControl(IOCTL_CABLE_        |
     |   UNMAP_RING_BUFFER,                  |
     |   inBuf=UNMAP_REQUEST)           ---->  CableRingBuffer::UnmapFromUserMode
     |                                       |   -> MmUnmapLockedPages
     |                                       |
     |-- CloseHandle                    ---->  CloseHandler: clear FsContext
```

## Data Structures

All structures use `#pragma pack(push, 1)` (no padding). Rust counterparts use `#[repr(C, packed)]`.

### CABLE_DEVICE_ID

```c
typedef UINT8 CABLE_DEVICE_ID[16];    // 16 bytes
```

### CABLE_DEVICE_CONTROL_PAYLOAD -- 662 bytes

Used for Create and Remove; also returned as the output buffer of Create. The `WaveSymbolicLink` field is populated by the driver only in the Create response — it is zeroed in all request payloads.

| Offset | Field | Type | Size | Notes |
|---|---|---|---|---|
| 0 | `Id` | UINT8[16] | 16 | |
| 16 | `FriendlyName` | WCHAR[64] | 128 | |
| 144 | `DeviceType` | UINT32 | 4 | |
| 148 | `IsEnabled` | UINT8 (BOOLEAN) | 1 | |
| 149 | `Persistent` | UINT8 (BOOLEAN) | 1 | |
| 150 | `WaveSymbolicLink` | WCHAR[256] | 512 | **Create response only.** Kernel-form KS audio interface path, e.g. `\??\ROOT#MEDIA#0000#{6994ad04-93ef-11d0-a3cc-00a0c9223196}\WaveCable_NN` (null-terminated). Used by the Tauri app to locate the new MM audio endpoint. |
| **Total** | | | **662** | |

Rust mirror: `common::DeviceControlPayload` (`crates/common/src/lib.rs`), field `wave_symbolic_link: [u16; 256]`.

### CABLE_IOCTL_REQUEST -- 768 bytes (union)

```c
typedef union {
    CABLE_DEVICE_CONTROL_PAYLOAD DeviceControl;  // 662 bytes
    CABLE_AUDIO_FORMAT           FormatUpdate;   //  12 bytes
    UINT8                        RawData[768];   // 768 bytes
} CABLE_IOCTL_REQUEST;
```

### CABLE_AUDIO_FORMAT -- 12 bytes

| Offset | Field | Type | Size |
|---|---|---|---|
| 0 | `SampleRate` | UINT32 | 4 |
| 4 | `Channels` | CABLE_CHANNEL_CONFIG (UINT32) | 4 |
| 8 | `DataType` | CABLE_AUDIO_DATA_TYPE (UINT32) | 4 |

### CABLE_RING_BUFFER_MAP_REQUEST -- 16 bytes

| Offset | Field | Type | Size |
|---|---|---|---|
| 0 | `DeviceId` | UINT8[16] | 16 |

### CABLE_RING_BUFFER_MAP_RESPONSE -- 16 bytes

| Offset | Field | Type | Size |
|---|---|---|---|
| 0 | `UserAddress` | UINT64 | 8 |
| 8 | `TotalSize` | UINT32 | 4 |
| 12 | `DataBufferSize` | UINT32 | 4 |

### CABLE_RING_BUFFER_UNMAP_REQUEST -- 24 bytes

| Offset | Field | Type | Size |
|---|---|---|---|
| 0 | `DeviceId` | UINT8[16] | 16 |
| 16 | `UserAddress` | UINT64 | 8 |

### CABLE_RING_BUFFER_HEADER -- 24 bytes

| Offset | Field | Type | Size |
|---|---|---|---|
| 0 | `WriteIndex` | UINT64 | 8 |
| 8 | `ReadIndex` | UINT64 | 8 |
| 16 | `BufferSize` | UINT32 | 4 |
| 20 | `Status` | UINT32 | 4 |

Status values:

| Value | Name |
|---|---|
| 0 | `CABLE_RING_BUFFER_STATUS_OK` |
| 1 | `CABLE_RING_BUFFER_STATUS_OVERRUN` |
| 2 | `CABLE_RING_BUFFER_STATUS_UNDERRUN` |

### Enumerations

**CABLE_DEVICE_TYPE** (UINT32):

| Value | Name |
|---|---|
| 0 | `CableDeviceTypeRender` (speakers) |
| 1 | `CableDeviceTypeCapture` (microphone) |

**CABLE_AUDIO_DATA_TYPE** (UINT32):

| Value | Name |
|---|---|
| 0 | `CableAudioDataPcmInt16` |
| 1 | `CableAudioDataPcmInt24` |
| 2 | `CableAudioDataPcmInt32` |
| 3 | `CableAudioDataFloat32` |

**CABLE_CHANNEL_CONFIG** (UINT32):

| Value | Name |
|---|---|
| 1 | `CableChannelMono` |
| 2 | `CableChannelStereo` |
| 4 | `CableChannelQuad` |
| 6 | `CableChannelSurround51` |
| 8 | `CableChannelSurround71` |

## Ring Buffer

Shared memory between kernel and user-mode for lock-free audio data transfer.

> Hardening update: detailed kernel safety patches and busy-removal policy are documented in `docs/driver-hardening.md`.

### Memory Layout

```
+----------------------------+--------------------------------------+
| CABLE_RING_BUFFER_HEADER   |          Audio Data Buffer           |
|        (24 bytes)          |   (CABLE_DEFAULT_RING_BUFFER_SIZE)   |
+----------------------------+--------------------------------------+
^                            ^
|                            |
m_pHeader                    m_pDataBuffer
```

Default data size: **65,536 bytes** (64 KB).
Total mapped size: **65,560 bytes** (header + data).

At 48 kHz / 16-bit / stereo (192,000 bytes/sec), this provides ~341 ms of buffer headroom.

### Allocation and Mapping

1. **Kernel allocation**: `ExAllocatePool2(POOL_FLAG_NON_PAGED, totalSize, tag)` -- non-paged pool for DISPATCH_LEVEL access.
2. **MDL creation**: `IoAllocateMdl` + `MmBuildMdlForNonPagedPool` -- describes physical pages.
3. **User-mode mapping**: `MmMapLockedPagesSpecifyCache(mdl, UserMode, MmCached, NULL, FALSE, NormalPagePriority | MdlMappingNoExecute)`.
4. **Unmapping**: `MmUnmapLockedPages(userAddr, mdl)`.

### Lock-Free Protocol

The ring buffer uses monotonically increasing indices (not modular). The actual buffer position is `index % BufferSize`.

- **WriteIndex**: Only written by the kernel (render path) or user-mode producer.
- **ReadIndex**: Only written by the user-mode consumer or kernel (capture path).
- **Memory barriers**: `KeMemoryBarrier()` after index updates for cross-core visibility.

**Write path** (kernel, DISPATCH_LEVEL safe):
```
writePos = WriteIndex % BufferSize
copy data to m_pDataBuffer[writePos..] (wrapping at boundary)
KeMemoryBarrier()
WriteIndex += bytesWritten    // monotonically increasing
```

**Read path** (kernel or user-mode):
```
available = WriteIndex - ReadIndex
if available > BufferSize:
    overrun detected, reset ReadIndex = WriteIndex - BufferSize
readPos = ReadIndex % BufferSize
copy data from m_pDataBuffer[readPos..] (wrapping at boundary)
KeMemoryBarrier()
ReadIndex += bytesRead        // monotonically increasing
```

## Key Source Files

| File | Description |
|---|---|
| `driver/Source/Inc/cable_common.h` | Shared IOCTL codes, structs, enums (mirrors Rust `crates/common/src/lib.rs`) |
| `driver/Source/Inc/definitions.h` | GUIDs, pool tags, constants, PortCls device extension layout |
| `driver/Source/Main/adapter.cpp` | DriverEntry, IRP hooks, IOCTL dispatch, PnP handler |
| `driver/Source/Main/common.cpp` | CAdapterCommon: virtual device CRUD, ring buffer management, subdevice cache |
| `driver/Source/Utilities/CableRingBuffer.h` | Ring buffer class declaration |
| `driver/Source/Utilities/CableRingBuffer.cpp` | Ring buffer: allocate, map, read, write, cleanup |
| `driver/Source/Main/minwavert.cpp` | WaveRT miniport stream implementation |
| `driver/Source/Inc/minipairs.h` | Static endpoint templates (SpeakerMiniports, MicArray1Miniports) |
| `docs/driver-hardening.md` | Kernel hardening changes, risk analysis, and validation checklist |

## Build

```powershell
& "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\MSBuild\Current\Bin\MSBuild.exe" `
    "C:\Users\nwh63\cable\driver\CableAudio.sln" `
    /p:Configuration=Debug /p:Platform=x64 /p:Inf2CatUseLocalTime=true /t:Build
```

Output files in `driver/x64/Debug/package/`:
- `CableAudio.sys` -- the driver binary
- `CableAudio.inf` -- installation INF
- `cableaudio.cat` -- signed catalog

## Installation (Test Signing)

```powershell
# Enable test signing (requires reboot)
bcdedit /set testsigning on

# Import test certificate to Root and TrustedPublisher stores
# (use .NET X509Store API -- certutil may fail on some systems)

# Install driver
devcon install CableAudio.inf ROOT\CableAudio

# Update existing driver (if already loaded)
devcon update CableAudio.inf ROOT\CableAudio
```
