# 엔드포인트 이름 지정 메커니즘

가상 오디오 장치를 생성하거나 이름을 변경할 때 Windows Sound 설정에 원하는 이름이 표시되도록 하는 전체 흐름을 설명한다.

## 배경: 왜 이름 지정이 어려운가?

CableAudio.sys 드라이버는 동적 가상 장치를 생성할 때 `FriendlyName` 속성을 드라이버 레벨에서 설정한다. 그러나 Windows Multimedia(MM) 레이어는 이 값을 **AudioEndpointBuilder 서비스**가 관리하는 별도의 property store에 캐시하며, 비관리자 프로세스에서는 이 store에 쓰기가 불가능하다.

결론적으로:

- **드라이버 IOCTL `IOCTL_CABLE_UPDATE_DEVICE_NAME`**: Tauri 앱에서 사용하지 않는다. 드라이버 내부 `FriendlyName` 필드를 갱신하지만 MM 엔드포인트 FriendlyName에는 반영되지 않는다.
- **`PKEY_Device_FriendlyName` (pid=14)**: AudioEndpointBuilder COM 서버가 관리자 권한으로도 외부 쓰기를 차단한다.
- **`PKEY_Device_DeviceDesc` (pid=2)**: 관리자 권한으로 `IPropertyStore::SetValue`를 호출하면 쓰기 가능하며, Windows Audio가 이 값을 FriendlyName으로 노출한다.

## 엔드포인트 식별: Snapshot-Diff 방식

새로 생성된 가상 장치에 해당하는 MM 엔드포인트 ID를 찾기 위해 **스냅샷-차분(Snapshot-Diff)** 방식을 사용한다.

```
IOCTL 전                          IOCTL 후
──────────────────                ──────────────────────────
snapshot_endpoint_ids()           find_new_endpoint_id(pre_snapshot)
  IMMDeviceEnumerator               IMMDeviceEnumerator 폴링
  eAll, DEVICE_STATE_ACTIVE         새로 나타난 ID = pre_snapshot에 없는 ID
  → HashSet<String>                 → 해당 ID가 새 장치의 엔드포인트 ID
```

**과거에 실패한 접근법들:**

| 접근법                                  | 실패 원인                                                                              |
| --------------------------------------- | -------------------------------------------------------------------------------------- |
| `PKEY_AudioEndpoint_Path` 직접 비교     | SWD 경로로, KS 경로(`ROOT#MEDIA#0000`)와 무관                                          |
| `CM_Get_Parent` PnP 트리 탐색           | `CONFIGRET(13)` = `CR_INVALID_DEVNODE` — SWD 엔드포인트 노드는 표준 PnP 부모 링크 없음 |
| `PKEY_Device_InstanceId` (pid=256) 비교 | `SWD\MMDEVAPI\{ep-guid}` 형태로 KS 필터 경로 포함 안 함                                |

Snapshot-Diff는 PnP 트리 탐색이 전혀 필요 없으며, 완전히 신뢰할 수 있다.

### 구현 위치

```
crates/tauri/src/lib.rs
  snapshot_endpoint_ids()     line ~408   — 현재 엔드포인트 ID HashSet 반환
  find_new_endpoint_id()      line ~473   — 폴링으로 새 엔드포인트 탐색 (최대 15회, 300ms 간격)
```

## 이름 쓰기: Elevated IPropertyStore

`PKEY_Device_DeviceDesc`에 쓰려면 관리자 권한이 필요하다. Tauri 앱 전체를 관리자로 실행하는 대신, **이름 변경이 필요한 순간에만** UAC 프롬프트를 표시하는 임시 elevated 프로세스를 사용한다.

### 흐름

```
Tauri 앱 (비관리자)                    elevated 자식 프로세스
────────────────────────────           ────────────────────────────────────
elevated_set_endpoint_device_desc()
  ShellExecuteExW(
    verb="runas",
    file=<현재 exe 경로>,        →      main.rs: --rename-endpoint <id> <name>
    params="--rename-endpoint              rename_endpoint_elevated()
            <endpoint_id>                    set_endpoint_device_desc()
            <new_name>")                       IMMDeviceEnumerator::GetDevice()
                                               IMMDevice::OpenPropertyStore(READWRITE)
  WaitForSingleObject(hProcess)  ←      IPropertyStore::SetValue(pid=2, VT_LPWSTR)
  GetExitCodeProcess()                   IPropertyStore::Commit()
                                         프로세스 종료 (exit code 0 = 성공)
```

### 인수 형식

```
cable.exe --rename-endpoint {0.0.0.00000000}.{guid} "My Device Name"
```

- `endpoint_id`: Windows MM 엔드포인트 ID 문자열 (중괄호 포함, 공백 없음 — 인용부호 불필요)
- `new_name`: 공백이 포함된 경우 큰따옴표로 감쌈 (`shell_quote()` 참고, `lib.rs:747`)

### 구현 위치

```
crates/tauri/src/main.rs
  main()                          -- --rename-endpoint 인수 파싱 → rename_endpoint_elevated() 호출

crates/tauri/src/lib.rs
  set_endpoint_device_desc()      line ~544  — COM IPropertyStore 쓰기 (elevated 컨텍스트에서 실행)
  CoUninitGuard                   line ~632  — RAII CoUninitialize 가드
  rename_endpoint_elevated()      line ~653  — main.rs에서 호출되는 public 진입점
  elevated_set_endpoint_device_desc() line ~664 — ShellExecuteExW runas + 대기
  shell_quote()                   line ~747  — 공백 포함 이름 인용부호 처리
```

## 장치 생성 시 전체 흐름

```
create_virtual_device (Tauri 커맨드)
  │
  ├─ 1. snapshot_endpoint_ids()           ← IOCTL 전 현재 엔드포인트 스냅샷
  │
  ├─ 2. DriverHandle::create_virtual_device(name, type)
  │       IOCTL_CABLE_CREATE_VIRTUAL_DEVICE
  │       → 드라이버가 PortCls 서브디바이스 생성
  │       → 응답: { id[16], WaveSymbolicLink[256], ... }
  │
  ├─ 3. find_new_endpoint_id(pre_snapshot, retries=15, delay=300ms)
  │       IMMDeviceEnumerator 폴링
  │       → 새 엔드포인트 ID 반환 (예: {0.0.0.00000000}.{guid})
  │
  ├─ 4. elevated_set_endpoint_device_desc(endpoint_id, name)
  │       ShellExecuteExW("runas", "--rename-endpoint ...")
  │       UAC 프롬프트 표시 → 사용자 승인
  │       elevated 자식: set_endpoint_device_desc() → PKEY_Device_DeviceDesc 쓰기
  │
  └─ 5. VirtualDevice { id, name, device_type, wave_symbolic_link, endpoint_id } 저장
```

## 이름 변경(Rename) 흐름

```
rename_virtual_device (Tauri 커맨드)
  │
  ├─ 1. AppData에서 endpoint_id 조회 (생성 시 캐시된 값)
  │
  ├─ 2. elevated_set_endpoint_device_desc(endpoint_id, new_name)
  │       (위와 동일한 UAC 흐름)
  │
  └─ 3. AppData.virtual_devices[device_id].name = new_name 갱신
```

## UAC 취소 처리

사용자가 UAC 프롬프트를 취소하면 `ShellExecuteExW`가 오류를 반환하며, Tauri 커맨드가 오류를 프론트엔드로 전파한다. 장치 자체는 정상적으로 동작하지만 기본 이름("Cable Virtual Audio Device" 등)으로 표시된다.

`Menu.tsx`에서 UAC 취소 시 에러 메시지를 UI에 표시한다.

## 관련 파일

| 파일                       | 역할                                               |
| -------------------------- | -------------------------------------------------- |
| `crates/tauri/src/lib.rs`  | 엔드포인트 탐색, COM 쓰기, elevated 헬퍼 전체 구현 |
| `crates/tauri/src/main.rs` | `--rename-endpoint` CLI 모드 진입점                |
| `src/components/Menu.tsx`  | 프론트엔드: 이름 변경 UI, UAC 취소 에러 표시       |
| `docs/virtual-driver.md`   | IOCTL 코드 및 데이터 구조 상세                     |
