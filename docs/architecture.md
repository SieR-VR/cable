# Cable 아키텍처 문서

## 1. 프로젝트 개요

Cable은 실시간 오디오 라우팅을 위한 Tauri v2 데스크톱 애플리케이션이다. 사용자는 React Flow 기반의 비주얼 노드 그래프 UI에서 오디오 입력/출력 장치를 배치하고 연결하며, Rust 백엔드가 cpal을 통해 실제 오디오 스트림을 관리한다.

## 2. 레이어 구조

```
┌─────────────────────────────────────────────────────────────┐
│                    Frontend (React)                          │
│  @xyflow/react (React Flow) + Zustand + Tailwind            │
│  노드 그래프 UI, 장치 선택, 그래프 직렬화                       │
├─────────────────────────────────────────────────────────────┤
│                    IPC (Tauri Commands)                      │
│  invoke() 기반 동기 요청-응답 통신                              │
│  12개 커맨드: get_audio_hosts, get_audio_devices,             │
│              connect_driver, is_driver_connected,            │
│              list_virtual_devices, create_virtual_device,    │
│              remove_virtual_device, rename_virtual_device,   │
│              setup_runtime, enable_runtime, disable_runtime, │
│              open_devtools                                    │
├─────────────────────────────────────────────────────────────┤
│                    Backend (Rust/Tauri)                      │
│  Runtime: 오디오 그래프 처리 엔진                               │
│  Nodes: NodeTrait 구현체 (AudioInput/OutputDeviceNode,       │
│         VirtualAudioInput/OutputNode)                        │
│  DriverClient: CableAudio.sys IOCTL 통신                     │
│  cpal 0.17: 크로스 플랫폼 오디오 I/O                          │
├─────────────────────────────────────────────────────────────┤
│                    Common Crate (no_std)                     │
│  공유 데이터 타입 (RingBufferHeader, AudioFormat 등)           │
│  향후 Windows WDM 가상 오디오 드라이버용                        │
└─────────────────────────────────────────────────────────────┘
```

## 3. 프론트엔드 상세

### 3.1 파일 구조

```
src/
├── App.tsx                 # ReactFlow 캔버스, Apply/Enable 버튼
├── main.tsx                # React 엔트리포인트
├── state.ts                # Zustand 스토어: 앱 상태, IPC 호출
├── types.ts                # AudioDevice, AudioGraph, AudioNode, AudioEdge, VirtualDevice 타입
├── ipc.d.ts                # Tauri invoke() 타입 오버로드
├── lib/utils.ts            # cn(), formatAudioEdgeType() 유틸리티
├── nodes/
│   ├── NodeBase.tsx         # 빈 파일 (플레이스홀더)
│   ├── AudioInputDevice.tsx # 입력 장치 노드 컴포넌트
│   └── AudioOutputDevice.tsx# 출력 장치 노드 컴포넌트
└── components/
    ├── Menu.tsx             # 사이드 메뉴: 가상 장치 생성/이름 변경/삭제
    └── ContextMenu.tsx      # 우클릭 컨텍스트 메뉴 (미구현)
```

### 3.2 노드 타입 레지스트리

`types.ts`에서 React Flow의 `nodeTypes`를 정의하여 문자열 키를 컴포넌트에 매핑한다:

```typescript
// src/types.ts:19-22
export const nodeTypes = {
  audioInputDevice: AudioInputDevice,
  audioOutputDevice: AudioOutputDevice,
} satisfies NodeTypes;
```

### 3.3 상태 관리 (Zustand)

`state.ts`의 `useAppStore`가 전체 앱 상태를 관리한다:

- **nodes/edges**: React Flow 노드와 엣지 배열
- **availableAudioHosts/Devices**: IPC로 가져온 호스트/장치 목록
- **onConnect**: 엣지 연결 시 오디오 포맷 호환성 검증
  - `formatAudioEdgeType()`으로 생성된 문자열 (예: `"audio_48000Hz_2ch_32bit"`)을 비교
  - 포맷 불일치 시 연결 거부

### 3.4 그래프 직렬화

"Apply" 버튼 클릭 시 (`App.tsx:38-60`), React Flow의 노드/엣지를 `AudioGraph` 구조로 변환:

```typescript
// src/App.tsx:39-55
const graph: AudioGraph = {
  nodes: nodes.map((node) => ({
    type: node.type,              // "audioInputDevice" | "audioOutputDevice"
    data: {
      id: node.id,
      device: node.data.device,   // 선택된 AudioDevice 또는 null
    },
  })),
  edges: edges.map((edge) => ({
    id: edge.id,
    from: edge.source,
    to: edge.target,
    frequency: edge.data?.frequency,
    channels: edge.data?.channels,
    bitsPerSample: edge.data?.bitsPerSample,
  })),
};
```

이 `AudioGraph`가 `invoke("setup_runtime", { graph, bufferSize: 512 })`를 통해 Rust 백엔드로 전달된다.

## 4. IPC 레이어

### 4.1 Tauri 커맨드

모든 IPC는 Tauri v2의 `#[tauri::command]` + `invoke()` 패턴을 사용한다. 이벤트 시스템(`listen`/`emit`)은 사용하지 않는다.

| 커맨드 | Rust 위치 | 설명 | 인자 | 반환 |
|--------|-----------|------|------|------|
| `get_audio_hosts` | `lib.rs:95` | 사용 가능한 오디오 호스트 목록 | 없음 | `Vec<String>` |
| `get_audio_devices` | `lib.rs:103` | 특정 호스트의 입력/출력 장치 목록 | `host: String` | `(Vec<AudioDevice>, Vec<AudioDevice>)` |
| `connect_driver` | `lib.rs:158` | CableAudio.sys 드라이버 핸들 열기 | 없음 | `bool` |
| `is_driver_connected` | `lib.rs:184` | 드라이버 연결 상태 확인 | 없음 | `bool` |
| `list_virtual_devices` | `lib.rs:199` | 생성된 가상 장치 목록 | 없음 | `Vec<VirtualDevice>` |
| `create_virtual_device` | `lib.rs:209` | 가상 오디오 장치 생성 (IOCTL + 엔드포인트 이름 지정) | `name, device_type` | `VirtualDevice` |
| `remove_virtual_device` | `lib.rs:306` | 가상 오디오 장치 제거 | `device_id` | `()` |
| `rename_virtual_device` | `lib.rs:339` | 가상 장치 이름 변경 (elevated UAC) | `device_id, new_name` | `()` |
| `setup_runtime` | `lib.rs:757` | 오디오 그래프로 런타임 생성 | `graph, host, buffer_size` | `()` |
| `enable_runtime` | `lib.rs:811` | 런타임 처리 스레드 시작 | 없음 | `()` |
| `disable_runtime` | `lib.rs:849` | 런타임 처리 스레드 중지 | 없음 | `()` |
| `open_devtools` | `lib.rs:844` | WebView 개발자 도구 열기 | 없음 | `()` |

### 4.2 타입 안전성

`ipc.d.ts`에서 `@tauri-apps/api/core`의 `invoke` 함수를 오버로드하여 각 커맨드별 인자/반환 타입을 지정한다:

```typescript
// src/ipc.d.ts
declare module "@tauri-apps/api/core" {
  declare function invoke(cmd: "get_audio_hosts"): Promise<string[]>;
  declare function invoke(cmd: "get_audio_devices", args: { host: string }): Promise<[AudioDevice[], AudioDevice[]]>;
  declare function invoke(cmd: "setup_runtime", args: { graph: AudioGraph; buffer_size: number }): Promise<void>;
  // ...
}
```

### 4.3 공유 상태

`Mutex<AppData>`가 Tauri의 `State`로 관리된다 (`lib.rs:43`):

```rust
struct AppData {
  runtime: Option<runtime::Runtime>,
  runtime_thread: Option<std::thread::JoinHandle<()>>,
  runtime_running: Option<Arc<AtomicBool>>,
  #[cfg(windows)]
  driver_handle: Option<Arc<driver_client::DriverHandle>>,
  /// 메뉴 패널에서 생성한 가상 장치 (hex_device_id -> VirtualDevice).
  virtual_devices: BTreeMap<String, VirtualDevice>,
}
```

`VirtualDevice` 구조체 (`lib.rs:24`):

```rust
pub(crate) struct VirtualDevice {
  pub id: String,           // 드라이버 할당 16바이트 ID (hex 인코딩)
  pub name: String,         // 사용자 지정 표시 이름
  pub device_type: String,  // "render" | "capture"
  // 내부 필드 (프론트엔드로 직렬화되지 않음):
  pub wave_symbolic_link: String,  // KS 오디오 인터페이스 심볼릭 링크
  pub endpoint_id: String,         // Windows MM 엔드포인트 ID (캐시)
}
```

## 5. 백엔드 상세

### 5.1 파일 구조

```
crates/tauri/src/
├── main.rs         # 엔트리포인트: --rename-endpoint CLI 모드 처리 또는 ui::run() 호출
├── lib.rs          # Tauri 커맨드, 가상 장치 관리, COM 헬퍼, elevated rename
├── driver_client.rs# CableAudio.sys IOCTL 래퍼 (DriverHandle, CreatedDevice)
├── runtime.rs      # Runtime 구조체, RuntimeState, process() 루프
└── nodes/
    ├── mod.rs                      # NodeTrait 정의
    ├── audio_input_device.rs       # AudioInputDeviceNode
    ├── audio_output_device.rs      # AudioOutputDeviceNode
    ├── virtual_audio_input.rs      # VirtualAudioInputNode (driver ring buffer 읽기)
    └── virtual_audio_output.rs     # VirtualAudioOutputNode (driver ring buffer 쓰기)
```

### 5.2 핵심 타입

#### AudioNode (Tagged Enum)

```rust
// crates/tauri/src/lib.rs:35-40
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub(crate) enum AudioNode {
  AudioInputDevice(AudioInputDeviceNode),
  AudioOutputDevice(AudioOutputDeviceNode),
}
```

`#[serde(tag = "type", content = "data")]` (인접 태그 열거형)을 사용하여 프론트엔드의 `{ type: "audioInputDevice", data: {...} }` JSON 형태와 직접 매핑된다.

#### AudioEdge

```rust
// crates/tauri/src/lib.rs:42-52
struct AudioEdge {
  id: String,
  from: String,   // 소스 노드 ID
  to: String,     // 타겟 노드 ID
  frequency: Option<u32>,
  channels: Option<u16>,
  bits_per_sample: Option<usize>,
}
```

### 5.3 NodeTrait

```rust
// crates/tauri/src/nodes/mod.rs:8-17
pub(crate) trait NodeTrait {
  fn init(&mut self, runtime: &Runtime) -> Result<(), String>;
  fn dispose(&mut self, runtime: &Runtime) -> Result<(), String>;
  fn process(&mut self, runtime: &Runtime, state: &RuntimeState) -> Result<BTreeMap<String, Vec<f32>>, String>;
}
```

- `init()`: 리소스 할당 (예: cpal 스트림 생성, 링 버퍼 초기화)
- `dispose()`: 리소스 해제 (스트림 drop, 버퍼 정리)
- `process()`: 매 틱마다 호출. edge_id -> 오디오 샘플 버퍼 맵 반환

### 5.4 Runtime

```rust
// crates/tauri/src/runtime.rs
pub(crate) struct Runtime {
  pub buffer_size: u32,
  pub nodes: Vec<AudioNode>,
  pub edges: Vec<AudioEdge>,
  pub audio_host: Host,
}
```

`Runtime::process()`:
1. 모든 `AudioNode`을 `&mut dyn NodeTrait`로 변환
2. 순서대로 순회하며 `process()` 호출
3. 각 노드의 출력(`BTreeMap<String, Vec<f32>>`)을 `RuntimeState.edge_values`에 누적
4. 다음 노드는 이전 노드의 출력을 `state.edge_values`에서 읽을 수 있음

## 6. cpal 통합

### 6.1 장치 열거

`lib.rs`의 `get_audio_hosts()`와 `get_audio_devices()`에서 cpal의 `available_hosts()`, `host_from_id()`, `input_devices()`, `output_devices()`를 사용하여 시스템의 오디오 장치를 열거한다.

각 장치에서 추출하는 정보:
- `d.id()` - 장치 고유 ID
- `d.description().name()` - 사람이 읽을 수 있는 이름
- `d.description().extended()` - 확장 설명 목록
- `d.default_input_config()` / `d.default_output_config()` - 샘플 레이트, 채널 수, 비트 깊이

### 6.2 오디오 스트림 및 데이터 흐름

스레드 모델:

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│  cpal Input      │    │  Runtime Thread   │    │  cpal Output    │
│  Callback Thread │    │  (timed sleep)    │    │  Callback Thread│
│                  │    │                   │    │                  │
│  audio data ──── │──▶ │ ──ringbuf──▶      │    │                  │
│  → push to       │    │  InputNode        │    │                  │
│    ring buffer   │    │  .process()       │    │                  │
│                  │    │      │             │    │                  │
│                  │    │      ▼ edge_values │    │                  │
│                  │    │  OutputNode        │    │ ◀── pop from    │
│                  │    │  .process() ──────│──▶ │    ring buffer  │
│                  │    │  → push to        │    │    → fill output│
│                  │    │    ring buffer    │    │      buffer     │
└─────────────────┘    └──────────────────┘    └─────────────────┘
```

- **Input**: cpal 콜백 스레드에서 캡처한 오디오 데이터를 lock-free 링 버퍼(ringbuf)의 Producer로 push. Runtime 스레드에서 Consumer로 pop하여 `edge_values`에 기록.
- **Output**: Runtime 스레드에서 `edge_values`의 데이터를 링 버퍼 Producer로 push. cpal 출력 콜백 스레드에서 Consumer로 pop하여 출력 버퍼에 기록.

## 7. 전체 데이터 흐름

```
[사용자] 장치 선택 및 노드 연결
    │
    ▼
[React Flow UI] 노드 그래프 편집
    │
    ▼  "Apply" 클릭
[App.tsx] AudioGraph 직렬화
    │
    ▼  invoke("setup_runtime", { graph, host, bufferSize })
[lib.rs] setup_runtime 커맨드
    │  AudioGraph 역직렬화, Runtime 생성, 각 노드 init() 호출
    ▼
[Runtime] nodes + edges + cpal Host
    │
    ▼  "Enable Runtime" 클릭 → enable_runtime 커맨드
[std::thread::spawn] timed sleep loop { runtime.process() }
    │
    ▼  process() 매 틱마다
[InputNode::process()] ring buffer에서 데이터 읽기 → edge_values에 기록
    │
    ▼
[OutputNode::process()] edge_values에서 데이터 읽기 → ring buffer에 기록
    │
    ▼  "Disable Runtime" 클릭 → disable_runtime 커맨드
[AtomicBool] running = false → 루프 종료, 각 노드 dispose() 호출
```

## 8. Common Crate

`crates/common`은 `#![no_std]` 크레이트로, CableAudio.sys 커널 드라이버와 공유하는 데이터 타입을 정의한다:

- `AudioDataType`: PCM Int16/24/32, Float32
- `ChannelConfig`: Mono ~ 7.1 Surround
- `AudioFormat`: 샘플 레이트, 채널 구성, 데이터 타입
- `RingBufferHeader`: 커널-유저스페이스 공유 메모리 링 버퍼 헤더
- `DeviceControlPayload`: 가상 장치 생성/관리 명령 (662 bytes, `wave_symbolic_link: [u16; 256]` 포함)
- `IoctlRequest`: IOCTL 통신용 유니온 타입 (768 bytes)
- IOCTL 상수: `IOCTL_CREATE_VIRTUAL_DEVICE` 등

`cable-tauri` 크레이트의 `driver_client.rs`에서 IOCTL 송수신에 실제로 사용된다. C++ 미러: `driver/Source/Inc/cable_common.h`.

## 9. 의존성

### Rust (crates/tauri)

| 의존성 | 버전 | 용도 |
|--------|------|------|
| tauri | 2 | 프레임워크 |
| cpal | 0.17 | 크로스 플랫폼 오디오 I/O |
| ringbuf | 0.4 | lock-free 링 버퍼 (스레드 간 오디오 데이터 전달) |
| serde / serde_json | 1 | 직렬화 |
| windows | 0.62.2 | Win32 오디오/COM API |
| common | path | 공유 타입 |

### Frontend (package.json)

| 패키지 | 버전 | 용도 |
|--------|------|------|
| @xyflow/react | ^12.10.0 | 노드 그래프 UI |
| zustand | ^5.0.11 | 상태 관리 |
| @tauri-apps/api | ^2 | Tauri IPC |
| react | ^19.1.0 | UI 프레임워크 |
| tailwindcss | ^4.1.12 | CSS 프레임워크 |

## 10. 드라이버 안정성 노트

커널 모드 드라이버 안정성/하드닝 변경 사항은 `docs/driver-hardening.md`에 별도로 기록한다.

- 동적 장치 제거 정책: 사용 중 장치 제거 시 `STATUS_DEVICE_BUSY`
- 링 버퍼 수명 관리: 스트림 참조/해제 기반으로 UAF 방지
- 사용자 매핑 검증: map/unmap 주소/프로세스 소유자 검증 강화
