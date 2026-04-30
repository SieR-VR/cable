# Cable 아키텍처 문서

## 1. 프로젝트 개요

Cable은 실시간 오디오 라우팅을 위한 Tauri v2 데스크톱 애플리케이션이다. 사용자는 React Flow 기반의 비주얼 노드 그래프 UI에서 입력/출력 장치, 가상 오디오 장치, 이펙터, VST3 플러그인 등을 배치하고 연결한다. Rust 백엔드는 cpal로 실제 오디오 스트림을 다루고, Windows에서는 자체 커널 드라이버(`CableAudio.sys`)를 통해 가상 오디오 장치를 제공한다.

## 2. 레이어 구조

```
┌─────────────────────────────────────────────────────────────┐
│                    Frontend (React)                          │
│  @xyflow/react (React Flow) + Zustand + Tailwind             │
│  노드 그래프 UI, 장치/플러그인 선택, 그래프 직렬화/저장        │
├─────────────────────────────────────────────────────────────┤
│                    IPC (Tauri Commands)                      │
│  invoke() 기반 동기 요청-응답 통신 (이벤트 시스템 사용 안 함)  │
│  증분 그래프 변경(add_node/add_edge/...)과 일괄 교체           │
│  (replace_graph), 가상 장치 관리, 노드 명령 디스패치          │
├─────────────────────────────────────────────────────────────┤
│                    Backend (Rust/Tauri)                      │
│  Runtime: 토폴로지 정렬 기반 오디오 그래프 처리 엔진            │
│  Nodes: 16종의 NodeTrait 구현체 (입출력, 가상, 이펙터, VST,    │
│         시각화, 앱 캡처)                                       │
│  driver/: CableAudio.sys IOCTL 클라이언트, 엔드포인트 관리      │
│  cpal: 크로스 플랫폼 오디오 I/O                                │
├─────────────────────────────────────────────────────────────┤
│                    Common Crate (no_std)                     │
│  커널 드라이버와 공유하는 ABI 타입                              │
│  RingBufferHeader, AudioFormat, DeviceControlPayload 등         │
└─────────────────────────────────────────────────────────────┘
```

## 3. 프론트엔드 상세

### 3.1 파일 구조

```
src/
├── App.tsx                     # ReactFlow 캔버스, Apply/Enable 버튼, 컨텍스트 메뉴
├── main.tsx                    # React 엔트리포인트
├── state.ts                    # Zustand 스토어 (앱 상태, IPC 호출)
├── types.ts                    # NodeType / AudioNode / AudioEdge / nodeDefs 등 공통 타입
├── ipc.d.ts                    # Tauri invoke() 타입 오버로드
├── node-definition.ts          # NodeDefinition<TNode> 인터페이스
├── lib/                        # 공통 UI 유틸리티
├── graph/                      # 엣지 타입 / 검증 엔진 (runFullValidation, runCascade)
├── nodes/                      # 모든 React 노드 컴포넌트 (16개)
│   ├── AudioInputDevice.tsx
│   ├── AudioOutputDevice.tsx
│   ├── VirtualAudioInput.tsx / VirtualAudioOutput.tsx
│   ├── AppAudioCapture.tsx
│   ├── SpectrumAnalyzer.tsx / WaveformMonitor.tsx
│   ├── Mixer.tsx / Gain.tsx
│   ├── ChannelMerge.tsx / ChannelSplit.tsx
│   ├── Delay.tsx / Echo.tsx / Reverb.tsx / Compressor.tsx
│   └── VstNode.tsx
└── components/
    ├── Menu.tsx                # 사이드 메뉴 (가상 장치 생성/이름 변경/삭제)
    ├── ContextMenu.tsx         # 우클릭으로 노드 추가
    ├── AudioEdge.tsx           # 엣지 컴포넌트 + edgeTypes
    ├── AudioHandle.tsx         # 공통 입출력 핸들
    └── NodeShell.tsx           # 노드 공통 외곽
```

노드별 세부사항은 [`docs/nodes.md`](nodes.md)를 참고한다.

### 3.2 노드 타입 레지스트리

각 노드 파일은 `default export`로 `NodeDefinition<TNode>` 객체(`{ component, toAudioNode, handles?, validate? }`)를 내보낸다. `types.ts`의 `nodeDefs` 객체에 이 default export들을 모두 모아두면, React Flow의 `nodeTypes`와 IPC 직렬화 함수 `serializeNode()`가 자동으로 도출된다.

```ts
// src/types.ts (발췌)
export const nodeDefs = {
  audioInputDevice: audioInputDeviceDef,
  audioOutputDevice: audioOutputDeviceDef,
  virtualAudioInput: virtualAudioInputDef,
  virtualAudioOutput: virtualAudioOutputDef,
  spectrumAnalyzer: spectrumAnalyzerDef,
  waveformMonitor: waveformMonitorDef,
  appAudioCapture: appAudioCaptureDef,
  mixer: mixerDef,
  gain: gainDef,
  channelMerge: channelMergeDef,
  channelSplit: channelSplitDef,
  delay: delayDef,
  compressor: compressorDef,
  reverb: reverbDef,
  echo: echoDef,
  vst: vstNodeDef,
};

export const nodeTypes: NodeTypes = /* nodeDefs로부터 도출되는 lazy Proxy */;
export function serializeNode(node: NodeType): AudioNode { /* nodeDefs[type].toAudioNode */ }
```

### 3.3 상태 관리 (Zustand)

`src/state.ts`의 `useAppStore`가 전체 앱 상태를 관리한다. 모든 IPC 호출은 스토어 액션 안에 캡슐화된다.

주요 상태:
- `nodes` / `edges`: React Flow의 노드와 엣지
- `availableAudioHosts` / `availableAudioInputDevices` / `availableAudioOutputDevices`
- `driverConnected`, `virtualDevices`, `vstPluginList`
- `validation`: 노드별 타입 검증 결과 (`Record<string, ValidationResult>`)
- `nodeRenderData`: `get_node_render_data` 폴링 결과 (Spectrum / Waveform 시각화)

검증 흐름:
1. UI에서 노드/엣지가 변경되면 `applyCascade(seedIds)` 또는 `applyFullValidation()`이 실행된다.
2. `runCascade` / `runFullValidation` (`src/graph/validation.ts`)이 각 노드의 `validate()`를 호출하여 타입 호환성을 검증한다.
3. 모든 노드 검증이 통과하면 `pushToRuntimeIfValid`가 `replace_graph` IPC로 그래프 전체를 Rust 런타임에 보낸다. 검증 실패가 하나라도 있으면 푸시하지 않고 UI만 업데이트한다.

증분 변경(`add_node`/`update_node`/`add_edge` 등)은 현재 정의는 되어 있으나, 프론트엔드는 검증 후 `replace_graph`로 통째로 푸시하는 방식을 사용한다.

### 3.4 그래프 직렬화

각 노드 컴포넌트의 `toAudioNode()`가 React Flow 노드를 `{ type, data }` 형태의 `AudioNode`로 변환한다. 엣지는 `serializeEdge()`가 `AudioEdge`로 변환하며, 핸들 ID(`fromHandle` / `toHandle`)가 포함된다.

```ts
// 예: ChannelSplit → Mixer 연결
{
  id: "edge-1",
  from: "split-1",
  fromHandle: "ch-0",
  to: "mixer-1",
  toHandle: "input-a",
  edgeType: { kind: "audio", frequency: 48000, channels: 1, bitsPerSample: 32 },
  ...
}
```

## 4. IPC 레이어

### 4.1 Tauri 커맨드

모든 IPC는 Tauri v2의 `#[tauri::command]` + `invoke()` 패턴을 사용한다. 이벤트 시스템(`listen`/`emit`)은 사용하지 않는다.

| 커맨드                  | 위치                          | 설명                                                                |
| ----------------------- | ----------------------------- | ------------------------------------------------------------------- |
| `get_window_list`       | `lib.rs::get_window_list`     | (Windows) 보이는 윈도우 목록 — 앱 오디오 캡처용                     |
| `get_audio_hosts`       | `lib.rs::get_audio_hosts`     | cpal 호스트 목록                                                    |
| `get_audio_devices`     | `lib.rs::get_audio_devices`   | 특정 호스트의 입력/출력 장치 목록                                   |
| `connect_driver`        | `driver/commands.rs`          | CableAudio.sys 핸들 열기                                            |
| `is_driver_connected`   | `driver/commands.rs`          | 드라이버 연결 상태 확인                                             |
| `list_virtual_devices`  | `driver/commands.rs`          | 메뉴에서 만든 가상 장치 목록                                        |
| `create_virtual_device` | `driver/commands.rs`          | 가상 장치 생성 + Snapshot-Diff로 엔드포인트 식별 + elevated 이름 지정 |
| `remove_virtual_device` | `driver/commands.rs`          | 가상 장치 제거                                                      |
| `rename_virtual_device` | `driver/commands.rs`          | 가상 장치 이름 변경 (UAC elevated)                                  |
| `add_node`              | `runtime.rs::add_node`        | 단일 노드 추가/upsert (즉시 `init()` 호출)                          |
| `remove_node`           | `runtime.rs::remove_node`     | 단일 노드 제거 + 연결된 엣지 정리 + `dispose()`                     |
| `update_node`           | `runtime.rs::update_node`     | 노드 갱신 (실제로는 `add_node`의 upsert 경로 사용)                  |
| `add_edge`              | `runtime.rs::add_edge`        | 엣지 추가/대체                                                      |
| `remove_edge`           | `runtime.rs::remove_edge`     | 엣지 제거                                                           |
| `replace_graph`         | `runtime.rs::replace_graph`   | 그래프 전체 원자적 교체 (저장 파일 로드 / 검증 통과 푸시에 사용)    |
| `set_audio_config`      | `runtime.rs::set_audio_config`| 호스트/버퍼 크기 변경 (필요 시 런타임 정지/재시작)                  |
| `enable_runtime`        | `runtime.rs::enable_runtime`  | 오디오 처리 스레드 시작                                             |
| `disable_runtime`       | `runtime.rs::disable_runtime` | 오디오 처리 스레드 정지                                             |
| `get_node_render_data`  | `runtime.rs::get_node_render_data` | 시각화 노드(Spectrum/Waveform)의 최신 프레임 데이터              |
| `save_graph`            | `lib.rs::save_graph`          | rfd 다이얼로그로 그래프 JSON 저장                                   |
| `read_text_file`        | `lib.rs::read_text_file`      | 파일 텍스트 로드 (드롭으로 그래프 열기)                              |
| `plugin_command`        | `lib.rs::plugin_command`      | 플러그인 레벨 명령 디스패치 (예: `vst` 스캔)                        |
| `node_command`          | `lib.rs::node_command`        | 노드별 인스턴스 명령 디스패치 → `NodeTrait::command()`              |
| `open_devtools`         | `lib.rs::open_devtools`       | (debug 빌드) WebView 개발자 도구                                    |

### 4.2 타입 안전성

`src/ipc.d.ts`에서 `@tauri-apps/api/core`의 `invoke` 함수를 커맨드별로 오버로드해 인자/반환 타입을 강제한다. 새 커맨드를 추가하면 이 파일에도 오버로드를 추가해야 한다.

### 4.3 공유 상태 (`AppData`)

```rust
// crates/tauri/src/lib.rs
pub(crate) struct AppData {
  pub runtime: Arc<StdMutex<runtime::Runtime>>,
  pub runtime_thread: Option<std::thread::JoinHandle<()>>,
  pub runtime_running: Option<Arc<AtomicBool>>,
  #[cfg(windows)]
  pub driver_handle: Option<Arc<crate::driver::client::DriverHandle>>,
  pub virtual_devices: BTreeMap<String, VirtualDevice>,
}
```

`AppData`는 Tauri의 `State<Mutex<AppData>>` (tokio `Mutex`)로 관리된다. 그러나 `Runtime` 자체는 `Arc<StdMutex<Runtime>>`로 감싸여 있어 IPC 스레드와 오디오 처리 스레드가 동일 인스턴스를 공유한다. IPC 커맨드는 `AppData` 락을 잠시 잡아 `runtime` Arc를 복제한 뒤 즉시 풀고, `Runtime` 락을 별도로 잡아 작업한다(데드락 방지).

`VirtualDevice`(메뉴에서 생성한 가상 장치 메타데이터):

```rust
pub(crate) struct VirtualDevice {
  pub id: String,           // 드라이버 할당 16바이트 ID (hex)
  pub name: String,         // 사용자 표시 이름
  pub device_type: String,  // "render" | "capture"
  #[serde(skip)]
  pub endpoint_id: String,  // Windows MM 엔드포인트 ID (캐시)
}
```

## 5. 백엔드 상세

### 5.1 파일 구조

```
crates/tauri/src/
├── main.rs                       # 엔트리포인트
│                                 # --rename-endpoint CLI 모드 또는 cable_tauri::run() 호출
├── lib.rs                        # AppData, 일반 커맨드, 노드/플러그인 명령 디스패치
├── runtime.rs                    # Runtime, AudioNode enum, RuntimeState, IPC 커맨드, 오디오 스레드
├── vst3_common.rs                # VST3 COM 공통 타입
├── driver/                       # CableAudio.sys 클라이언트 + 엔드포인트 관리
│   ├── mod.rs
│   ├── client.rs                 # DriverHandle, IOCTL 래퍼, RingBufferMapping
│   ├── commands.rs               # connect/list/create/remove/rename Tauri 커맨드
│   ├── endpoint.rs               # snapshot/find_new_endpoint_id, set_endpoint_device_desc,
│   │                             # rename_endpoint_elevated, shell_quote, CoUninitGuard
│   └── types.rs                  # DeviceId 등
└── nodes/
    ├── mod.rs                    # NodeTrait, AudioBuffer
    ├── audio_input_device.rs / audio_output_device.rs
    ├── virtual_audio_input.rs / virtual_audio_output.rs
    ├── app_audio_capture.rs      # Windows WASAPI Application Loopback 기반 앱 오디오 캡처
    ├── spectrum_analyzer.rs / waveform_monitor.rs
    ├── mixer.rs / gain.rs
    ├── channel_merge.rs / channel_split.rs
    ├── delay.rs / echo.rs / reverb.rs / compressor.rs
    └── vst.rs                    # VST3 호스팅 + 에디터 윈도우
```

### 5.2 핵심 타입

#### AudioNode (인접 태그 enum)

```rust
// crates/tauri/src/runtime.rs
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub(crate) enum AudioNode {
  AudioInputDevice(AudioInputDeviceNode),
  AudioOutputDevice(AudioOutputDeviceNode),
  VirtualAudioInput(VirtualAudioInputNode),
  VirtualAudioOutput(VirtualAudioOutputNode),
  SpectrumAnalyzer(SpectrumAnalyzerNode),
  WaveformMonitor(WaveformMonitorNode),
  AppAudioCapture(AppAudioCaptureNode),
  Mixer(MixerNode),
  Gain(GainNode),
  ChannelMerge(ChannelMergeNode),
  ChannelSplit(ChannelSplitNode),
  Delay(DelayNode),
  Compressor(CompressorNode),
  Reverb(ReverbNode),
  Echo(EchoNode),
  Vst(VstNode),
}
```

`#[serde(tag = "type", content = "data")]`(인접 태그)을 사용하므로 프론트엔드의 `{ type: "audioInputDevice", data: {...} }` JSON과 직접 매핑된다.

#### AudioBuffer (노드 간 전달 데이터)

```rust
// crates/tauri/src/nodes/mod.rs
pub struct AudioBuffer {
  pub samples: Vec<f32>,          // 인터리브된 f32 샘플
  pub channels: u16,
  pub sample_rate: u32,
  pub bits_per_sample: u16,       // 처리는 항상 f32, 원본 포맷 정보로만 사용
}
```

이전에는 노드 간에 `Vec<f32>`만 전달했으나, 채널 수가 다른 신호(스테레오 ↔ 모노)와 채널 분리/병합을 정확히 지원하기 위해 `AudioBuffer`로 통일됐다.

#### AudioEdge

```rust
pub(crate) struct AudioEdge {
  pub id: String,
  pub from: String,
  pub from_handle: Option<String>,    // 다중 출력 노드용 (예: "ch-0", "vst-out-0")
  pub to: String,
  pub to_handle: Option<String>,      // 다중 입력 노드용 (예: "input-a", "ch-1")
  pub frequency: Option<u32>,
  pub channels: Option<u16>,
  pub bits_per_sample: Option<usize>,
}
```

### 5.3 NodeTrait

```rust
// crates/tauri/src/nodes/mod.rs
pub(crate) trait NodeTrait {
  fn id(&self) -> &str;
  fn init(&mut self, runtime: &Runtime) -> Result<(), String>;
  fn dispose(&mut self, runtime: &Runtime) -> Result<(), String>;
  fn process(
    &mut self,
    runtime: &Runtime,
    state: &RuntimeState,
  ) -> Result<BTreeMap<String, AudioBuffer>, String>;

  /// 노드 인스턴스에 대한 통합 IPC 진입점. 기본 구현은 Err.
  fn command(&mut self, _data: serde_json::Value) -> Result<serde_json::Value, String> {
    Err("command not supported by this node".into())
  }
}
```

- `init()` / `dispose()`: 리소스 획득/해제 (cpal 스트림, 링 버퍼, VST3 인스턴스, FFT plan 등)
- `process()`: 매 틱 호출. 출력 엣지 ID → `AudioBuffer` 맵 반환
- `command()`: `node_command` IPC가 `data["op"]` 등을 디스패치할 때 호출 (예: VST 파라미터 변경)

### 5.4 Runtime

```rust
pub(crate) struct Runtime {
  pub buffer_size: u32,                                 // 샘플 단위
  pub sample_rate: u32,
  pub host_name: String,
  pub nodes: Vec<AudioNode>,
  pub edges: Vec<AudioEdge>,
  pub audio_host: cpal::Host,
  #[cfg(windows)]
  pub driver_handle: Option<Arc<DriverHandle>>,
  // 시각화 노드와 IPC 사이의 공유 버퍼 (락 짧게 잡고 clone)
  pub spectrum_buffers: BTreeMap<String, Arc<Mutex<Vec<f32>>>>,
  pub waveform_buffers: BTreeMap<String, Arc<Mutex<Vec<f32>>>>,
}
```

`Runtime`은 앱 시작 시점에 `Runtime::new_default()`로 생성되어 항상 존재하며, `AppData.runtime`에 `Arc<StdMutex<Runtime>>` 형태로 보관된다. 오디오 처리 스레드는 동일한 Arc를 복제해 가져간다.

`Runtime::process()`(매 틱):
1. `compute_topological_order()`로 토폴로지 정렬 순서 계산. 사이클이 있으면 인덱스 순서로 폴백.
2. 정렬된 순서대로 각 노드의 `process()` 호출.
3. 각 노드 출력(`BTreeMap<edge_id, AudioBuffer>`)을 `RuntimeState.edge_values`에 누적.
4. 다음 노드는 자신의 입력 엣지 ID로 `state.edge_values`에서 데이터를 읽는다.

### 5.5 오디오 스레드 타이밍

오디오 처리 스레드는 `start_runtime_thread`에서 spawn된다. 매 틱 시간은 `buffer_size / sample_rate`(예: 512/48000 ≈ 10.67 ms)이며, 다음 틱까지 정확히 대기하기 위해 다음과 같은 하이브리드 방식을 사용한다.

```rust
let mut next_tick = Instant::now() + sleep_duration;
loop {
  // ... 그래프 처리 ...
  loop {
    let now = Instant::now();
    if now >= next_tick { break; }
    let remaining = next_tick - now;
    if remaining > Duration::from_millis(2) {
      thread::sleep(Duration::from_millis(1));   // 거친 슬립
    } else {
      std::hint::spin_loop();                    // 마지막 ~2ms는 spin
    }
  }
  next_tick += sleep_duration;
  // 실시간성 회복
  if next_tick < Instant::now() { next_tick = Instant::now() + sleep_duration; }
}
```

순수 `thread::sleep`만 사용하면 Windows 기본 타이머 분해능(~15.6 ms)이 작은 버퍼 크기에서 만성적인 언더런을 일으킨다. spin-loop를 마지막 구간에만 적용해 CPU 부하와 정확도를 절충한다.

## 6. cpal 통합

### 6.1 장치 열거

`get_audio_hosts()`와 `get_audio_devices()`가 cpal의 `available_hosts()`, `host_from_id()`, `input_devices()`, `output_devices()`를 사용한다. 각 장치에서 `id()`, `description().name()`, `description().extended()`, `default_input_config()` / `default_output_config()`(샘플 레이트/채널/비트 깊이)를 추출해 `AudioDevice` 구조체로 반환한다.

### 6.2 오디오 스트림 데이터 흐름 (입력/출력 장치 노드)

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│  cpal Input      │    │  Runtime Thread   │    │  cpal Output    │
│  Callback Thread │    │  (timed loop)     │    │  Callback Thread│
│                  │    │                   │    │                  │
│  audio data ─────│──▶ │ ──ringbuf──▶      │    │                  │
│  → ring buffer   │    │  InputNode        │    │                  │
│                  │    │  .process()       │    │                  │
│                  │    │      │             │    │                  │
│                  │    │      ▼ edge_values │    │                  │
│                  │    │  OutputNode        │    │ ◀── ring buffer │
│                  │    │  .process() ──────│──▶ │     → 출력 버퍼  │
└─────────────────┘    └──────────────────┘    └─────────────────┘
```

cpal 콜백 스레드와 Runtime 스레드 사이의 데이터 전달은 lock-free `ringbuf`를 사용해 락 충돌 없이 처리된다. 가상 오디오 노드는 cpal 대신 드라이버 ring buffer 매핑을 사용한다.

## 7. 전체 데이터 흐름

```
[사용자] 노드 추가/연결 (UI)
    │
    ▼
[React Flow] 변경 → Zustand 스토어
    │
    ▼  applyCascade / applyFullValidation
[graph/validation.ts] 노드별 validate() → ValidationResult 누적
    │
    ▼  모든 노드 valid이면
[invoke("replace_graph", { nodes, edges })]
    │
    ▼
[runtime::replace_graph] 기존 노드 dispose() → 새 노드 add_node(init() 호출)
    │
    ▼  사용자 "Enable Runtime"
[invoke("enable_runtime")] → start_runtime_thread
    │
    ▼  매 틱 (buffer_size/sample_rate 간격)
[Runtime::process()] 토폴로지 순서대로 각 NodeTrait::process()
    │   InputNode → ringbuf 읽기 → AudioBuffer
    │   FilterNode → 입력 AudioBuffer 처리 → 출력 AudioBuffer
    │   OutputNode → ringbuf로 cpal에 전달
    ▼
[ get_node_render_data 폴링 (30 fps) ]
    │   spectrum_buffers / waveform_buffers의 최신 프레임 → UI
    ▼
[사용자 "Disable Runtime"] → stop_runtime_thread
```

## 8. Common Crate

`crates/common`은 `#![no_std]` 크레이트로, CableAudio.sys 커널 드라이버와 공유하는 ABI 타입을 정의한다.

- `AudioDataType`: PCM Int16/24/32, Float32
- `ChannelConfig`: Mono ~ 7.1 Surround
- `AudioFormat`: 샘플 레이트 / 채널 / 데이터 타입
- `RingBufferHeader`: 커널-유저스페이스 공유 메모리 링 버퍼 헤더
- `DeviceControlPayload`: 가상 장치 생성/관리 페이로드 (662 bytes, `wave_symbolic_link: [u16; 256]` 포함)
- `IoctlRequest`: IOCTL 통신용 union (768 bytes)
- IOCTL 코드 상수 (`IOCTL_CABLE_CREATE_VIRTUAL_DEVICE` 등)

`cable-tauri` 크레이트의 `driver/client.rs`에서 IOCTL 송수신에 실제로 사용되며, C++ 미러는 `driver/Source/Inc/cable_common.h`이다. 모든 공유 구조체는 `#[repr(C, packed)]` / `#pragma pack(push, 1)`로 패딩 없이 정의된다.

## 9. 의존성

### Rust (`crates/tauri`)

| 의존성             | 용도                                                |
| ------------------ | --------------------------------------------------- |
| tauri              | 데스크톱 프레임워크                                 |
| cpal               | 크로스 플랫폼 오디오 I/O                            |
| ringbuf            | lock-free 링 버퍼 (cpal ↔ 런타임 스레드 통신)        |
| serde / serde_json | 직렬화 / IPC 데이터                                 |
| windows            | Win32 / COM / WASAPI / SetupDi                      |
| libloading         | VST3 DLL 동적 로드                                  |
| rustfft            | SpectrumAnalyzer FFT                                |
| rfd                | 파일 다이얼로그 (`save_graph`)                      |
| common             | 드라이버와 공유하는 ABI 타입                        |

### Frontend (`package.json`)

| 패키지          | 용도                                  |
| --------------- | ------------------------------------- |
| @xyflow/react   | 노드 그래프 UI                        |
| zustand         | 상태 관리                             |
| @tauri-apps/api | Tauri IPC                             |
| react           | UI 프레임워크                         |
| tailwindcss     | CSS                                   |
| lucide-react    | 아이콘                                |

## 10. 관련 문서

- [`docs/nodes.md`](nodes.md) — 노드별 세부사항
- [`docs/adding-a-node.md`](adding-a-node.md) — 새 노드 추가 가이드
- [`docs/virtual-driver.md`](virtual-driver.md) — CableAudio.sys IOCTL 인터페이스
- [`docs/driver-hardening.md`](driver-hardening.md) — 커널 안정성 변경 사항
- [`docs/endpoint-naming.md`](endpoint-naming.md) — 가상 장치 이름 지정 메커니즘
- [`docs/testing/vm.md`](testing/vm.md) — VMware 기반 E2E 테스트 환경
- [`docs/known-issues.md`](known-issues.md) — 알려진 한계와 잠재 이슈
