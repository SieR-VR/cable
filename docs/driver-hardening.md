# Driver Hardening Notes (Kernel Safety)

This document records kernel-mode safety fixes applied to `driver/` with a focus on preventing BSoD-class memory and synchronization failures.

## Scope

- Target: `driver/Source/**`
- Risk class: memory lifetime, lock discipline, process-context-sensitive mapping APIs
- Policy change: dynamic device removal now returns `STATUS_DEVICE_BUSY` when in use

## Key Risks Identified

### 1) Ring buffer use-after-free between stream DPC path and control IOCTL path

- Stream code cached a raw `CableRingBuffer*` and used it in run-time audio callbacks.
- Control path (`IOCTL_CABLE_REMOVE_VIRTUAL_DEVICE`) could destroy the same object.
- Result: stale pointer dereference in kernel audio path -> bug check risk.

### 2) Notification list concurrent mutation during DPC iteration

- Event registration/unregistration and DPC notification traversal accessed `m_NotificationList` without synchronization.
- Result: `LIST_ENTRY` corruption / pool corruption risk under contention.

### 3) User mapping unmap safety and process ownership

- Unmap path accepted caller-provided address without strict validation against the active mapping.
- User-mode mappings created by `MmMapLockedPagesSpecifyCache(UserMode, ...)` require careful process-context handling.
- Result: wrong-address unmap or wrong-process unmap risk.

### 4) Lock IRQL bookkeeping issue in miniport format/mode lock

- Spin lock old IRQL was stored in a shared member (`m_DeviceFormatsAndModesIrql`).
- Result: nested/concurrent paths could restore incorrect IRQL.

### 5) Guard gaps in WaveRT stream math/mapping path

- Several code paths depended on non-zero DMA/notification sizes and non-null mapped DMA pointers.
- Result: divide/modulo-by-zero and null access risk.

## Hardening Changes

## A. Ring buffer lifetime and `STATUS_DEVICE_BUSY` removal policy

### Files

- `driver/Source/Utilities/CableRingBuffer.h`
- `driver/Source/Utilities/CableRingBuffer.cpp`
- `driver/Source/Main/common.cpp`
- `driver/Source/Inc/common.h`
- `driver/Source/Main/minwavertstream.cpp`

### Changes

- Added explicit external reference counting to `CableRingBuffer`:
  - `AddReference()`
  - `ReleaseReference()`
  - `GetReferenceCount()`
- `FindRingBufferByWaveName()` now returns a referenced ring buffer.
- Added `IAdapterCommon::ReleaseRingBufferReference(...)` and implementation.
- Stream destructor releases its held ring buffer reference before stream close/release.
- Dynamic device removal now enforces busy policy:
  - if mapped to user mode -> `STATUS_DEVICE_BUSY`
  - if ring buffer is still referenced by stream(s) -> `STATUS_DEVICE_BUSY`

### Busy policy

- `IOCTL_CABLE_REMOVE_VIRTUAL_DEVICE` can fail with `STATUS_DEVICE_BUSY` while the device is actively mapped or streaming.
- Caller must stop/close stream and unmap ring buffer before retrying remove.

## B. Virtual-device table synchronization

### Files

- `driver/Source/Main/common.cpp`

### Changes

- Added `FAST_MUTEX m_VirtualDeviceLock` to serialize virtual-device table operations.
- Protected these methods with the mutex:
  - `CreateVirtualDevice`
  - `RemoveVirtualDevice`
  - `MapRingBuffer`
  - `UnmapRingBuffer`
  - `FindRingBufferByWaveName`
  - `CleanupVirtualDevices`

## C. Mapping ownership validation and safe unmap helper

### Files

- `driver/Source/Main/common.cpp`

### Changes

- Added entry fields:
  - `pMappedUserAddress`
  - `pMappingProcess`
- On map success:
  - store mapped address
  - capture and reference current process as owner
- On unmap:
  - require non-null address
  - require address exact match with active mapping
  - require caller process == owner process
- Added helper to detach mapping and release process reference safely:
  - `CableUnmapRingBufferForEntry(...)`

## D. Notification list synchronization

### Files

- `driver/Source/Main/minwavertstream.h`
- `driver/Source/Main/minwavertstream.cpp`

### Changes

- Added `KSPIN_LOCK m_NotificationListLock`.
- Protected list operations in:
  - `RegisterNotificationEvent`
  - `UnregisterNotificationEvent`
  - `TimerNotifyRT` traversal path
- Destructor now drains and frees notification list entries under lock after DPC flush.

## E. WaveRT guard checks and DMA mapping checks

### Files

- `driver/Source/Main/minwavertstream.cpp`

### Changes

- Added non-zero checks for movement/packet math paths.
- Added non-zero/non-null guards for DMA buffer dependent paths.
- Added `MapAllocatedPages(...)` null checks in both allocation paths; if map fails, MDL pages are freed and allocation fails.

## F. Spin lock IRQL restore safety

### Files

- `driver/Source/Main/minwavert.h`
- `driver/Source/Main/minwavert.cpp`

### Changes

- Removed shared member IRQL storage for format/mode lock.
- Replaced with function-local `KIRQL oldIrql` in each call site.

## G. Auxiliary correctness fixes

### Files

- `driver/Source/Main/common.cpp`
- `driver/Source/Utilities/savedata.cpp`

### Changes

- Fixed `DisconnectTopologies` capture branch status variable assignment (`ntStatus2` path).
- Switched `SaveData` worker queueing from `CriticalWorkQueue` to `DelayedWorkQueue`.

## Microsoft Documentation Alignment

- `MmMapLockedPagesSpecifyCache`: UserMode mapping exceptions/process-context rules.
- `MmUnmapLockedPages`: unmap must occur in correct original process context.
- `KeWaitForSingleObject`: IRQL and blocking requirements.
- `IoAllocateWorkItem` / `IoQueueWorkItem`: lifecycle and queueing guidance.
- `ExAllocatePool2`: allocation and zero-init behavior.

## Operational Notes for User-mode Caller

- Remove can now return `STATUS_DEVICE_BUSY` in these cases:
  - ring buffer still mapped
  - active stream still holds ring buffer reference
- Recommended order before remove:
  1. stop runtime/streams
  2. unmap ring buffer
  3. close handles
  4. remove device

## Validation Checklist

- Static:
  - WDK Code Analysis clean on modified files
  - lock and IRQL warnings reviewed
- Runtime:
  - Driver Verifier (Special Pool, Pool Tracking, IRQL checking, Deadlock Detection)
- E2E/Stress:
  - repeated create/map/run/unmap/remove loops
  - wrong-address unmap must fail with invalid-parameter semantics
  - concurrent register/unregister notification churn while streaming
  - remove while streaming must fail with `STATUS_DEVICE_BUSY`
  - VM test harness must fail if any post-test bugcheck evidence exists (Event 1001 or new dump file)

## Reproducible Static Analysis Commands

Run from repository root (`C:\Users\nwh63\cable`).

```powershell
# 1) Clean driver build via project script (sanity gate)
./scripts/build.ps1 -Target Driver -Configuration Debug -Platform x64 -Clean

# 2) Project-by-project C/C++ code analysis
& "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\MSBuild\Current\Bin\MSBuild.exe" \
  "driver/Source/Main/Main.vcxproj" \
  /t:ClCompile /p:Configuration=Debug /p:Platform=x64 \
  /p:RunCodeAnalysis=true /p:CodeAnalysisTreatWarningsAsErrors=false /m /verbosity:minimal

& "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\MSBuild\Current\Bin\MSBuild.exe" \
  "driver/Source/Utilities/Utilities.vcxproj" \
  /t:ClCompile /p:Configuration=Debug /p:Platform=x64 \
  /p:RunCodeAnalysis=true /p:CodeAnalysisTreatWarningsAsErrors=false /m /verbosity:minimal

& "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\MSBuild\Current\Bin\MSBuild.exe" \
  "driver/Source/Filters/Filters.vcxproj" \
  /t:ClCompile /p:Configuration=Debug /p:Platform=x64 \
  /p:RunCodeAnalysis=true /p:CodeAnalysisTreatWarningsAsErrors=false /m /verbosity:minimal

& "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\MSBuild\Current\Bin\MSBuild.exe" \
  "driver/Source/Inc/Inc.vcxproj" \
  /t:ClCompile /p:Configuration=Debug /p:Platform=x64 \
  /p:RunCodeAnalysis=true /p:CodeAnalysisTreatWarningsAsErrors=false /m /verbosity:minimal

# 3) Optional: solution-wide static analysis compile
& "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\MSBuild\Current\Bin\MSBuild.exe" \
  "driver/CableAudio.sln" \
  /t:ClCompile /p:Configuration=Debug /p:Platform=x64 \
  /p:RunCodeAnalysis=true /p:CodeAnalysisTreatWarningsAsErrors=false /m /verbosity:minimal
```

Notes:
- `scripts/build.ps1` now fails early when MSBuild returns non-zero.
- Removed stale `tonegenerator.cpp` entry from `driver/Source/Utilities/Utilities.vcxproj` to unblock analysis.
