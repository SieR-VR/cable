# 노드 레퍼런스

Cable이 제공하는 모든 노드의 역할, 핸들 구성, 데이터 필드, IPC 명령을 정리한다. 각 노드는 Rust 측 구현(`crates/tauri/src/nodes/<name>.rs`)과 React 컴포넌트(`src/nodes/<Name>.tsx`)가 1:1로 대응된다. 새 노드를 추가하는 절차는 [`docs/adding-a-node.md`](adding-a-node.md)를 참고한다.

## 공통 사항

- 모든 노드는 `NodeTrait` (`crates/tauri/src/nodes/mod.rs`)를 구현한다.
- 노드 간 데이터는 `AudioBuffer { samples, channels, sample_rate, bits_per_sample }`로 전달된다(인터리브 f32).
- 모든 노드 데이터는 `serde(rename_all = "camelCase")`로 직렬화되어 프론트엔드 JSON과 매핑된다.
- 다중 입력/출력 노드는 React Flow 엣지의 `sourceHandle` / `targetHandle`을 통해 채널을 구분한다 (`fromHandle` / `toHandle`로 IPC 전달).
- 인스턴스 단위 명령은 `node_command(nodeId, data)` IPC → `NodeTrait::command(data)`로 디스패치된다. 현재 커스텀 `command()`를 구현한 노드는 **VST**뿐이다.

핸들 표기 규칙:
- 입력 핸들(`type="target"`): 노드 좌측
- 출력 핸들(`type="source"`): 노드 우측

---

## 1. AudioInputDevice (`audioInputDevice`)

물리 오디오 입력 장치(마이크, 라인 인 등)에서 cpal로 데이터를 캡처하는 **Source** 노드.

- 파일: `crates/tauri/src/nodes/audio_input_device.rs`, `src/nodes/AudioInputDevice.tsx`
- 핸들: 출력 1개 (`audioInputDevice-source`)
- 동작:
  - `init()`에서 cpal `Stream`을 열고 lock-free `ringbuf::HeapRb`의 producer/consumer로 분리. cpal 콜백 스레드는 producer로 push, 런타임 스레드는 consumer로 pop.
  - `process()`에서 ring buffer에서 샘플을 읽어 `AudioBuffer`로 포장해 출력 엣지에 흘려보냄.
  - 실제 사용된 샘플 레이트와 채널 수는 `default_input_config()` 결과를 그대로 따른다.
- 직렬화 데이터:

```rust
struct AudioInputDeviceNode {
  id: String,
  device: AudioDevice,   // { id, readableName, descriptions, frequency, channels, bitsPerSample }
}
```

---

## 2. AudioOutputDevice (`audioOutputDevice`)

물리 오디오 출력 장치(스피커, 헤드폰)로 cpal을 통해 재생하는 **Sink** 노드.

- 파일: `crates/tauri/src/nodes/audio_output_device.rs`, `src/nodes/AudioOutputDevice.tsx`
- 핸들: 입력 1개 (`audioOutputDevice-target`)
- 동작:
  - `init()`에서 cpal 출력 스트림과 ring buffer producer를 준비. cpal 콜백은 consumer로 pop하여 출력 버퍼를 채움.
  - `process()`에서 입력 엣지의 `AudioBuffer.samples`를 producer에 push. 버퍼가 가득 차면 초과 샘플은 드롭되며 200 틱마다 OVERFLOW 로그가 찍힌다(글리치 발생).
- 직렬화 데이터: `AudioInputDevice`와 동일 (`device: AudioDevice`).

---

## 3. VirtualAudioInput (`virtualAudioInput`) — Windows 전용

Cable 가상 **마이크(capture)** 장치에 데이터를 써넣는 **Sink** 노드. Windows 응용프로그램이 이 가상 마이크를 입력으로 선택하면 그래프에서 흘러온 신호를 수음한다.

- 파일: `crates/tauri/src/nodes/virtual_audio_input.rs`, `src/nodes/VirtualAudioInput.tsx`
- 핸들: 입력 1개
- 동작:
  - `init()`에서 `DriverHandle::map_ring_buffer(device_id)` 호출 → 커널에서 매핑한 공유 메모리(`RingBufferMapping`)로 직접 쓰기.
  - `process()`에서 입력 `AudioBuffer.samples`를 ring buffer에 write. 드라이버 측에서는 이 데이터를 PortCls capture 엔드포인트로 노출.
- 직렬화 데이터:

```rust
struct VirtualAudioInputNode {
  id: String,
  device_id: String,   // 메뉴에서 만든 가상 장치의 hex ID
  name: String,        // 표시용
}
```

- 사전 조건: `connect_driver`로 드라이버 핸들이 열려 있어야 하며, 메뉴에서 capture 타입 가상 장치를 먼저 생성해야 한다.

---

## 4. VirtualAudioOutput (`virtualAudioOutput`) — Windows 전용

Cable 가상 **스피커(render)** 장치에서 들어온 데이터를 그래프에 공급하는 **Source** 노드. Windows 응용프로그램이 이 가상 스피커를 출력으로 선택하면 그 오디오가 그래프로 들어온다.

- 파일: `crates/tauri/src/nodes/virtual_audio_output.rs`, `src/nodes/VirtualAudioOutput.tsx`
- 핸들: 출력 1개
- 동작:
  - `init()`에서 ring buffer 매핑.
  - `process()`에서 ring buffer를 읽어 `AudioBuffer`로 출력 엣지에 흘려보냄.
- 직렬화 데이터: `VirtualAudioInput`과 동일 구조.

> 명명 주의: UI상의 "Virtual Mic"는 capture(=`VirtualAudioInput`이 데이터를 쓴다), "Virtual Speaker"는 render(=`VirtualAudioOutput`이 데이터를 읽는다)이다. 이 비대칭은 그래프 내부 관점에서의 input/output(데이터 흐름 방향)과 OS 입장에서의 capture/render가 반대이기 때문이다.

---

## 5. AppAudioCapture (`appAudioCapture`) — Windows 전용

Windows WASAPI Application Loopback API를 사용해 **특정 프로세스의 오디오만** 캡처하는 **Source** 노드. Windows 10 20H1 / build 19041+ 필요.

- 파일: `crates/tauri/src/nodes/app_audio_capture.rs`, `src/nodes/AppAudioCapture.tsx`
- 핸들: 출력 1개
- 동작:
  - `init()`에서 캡처 스레드를 spawn → `ActivateAudioInterfaceAsync(VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK, target_process_id)`. 5초 하드 타임아웃 적용.
  - 캡처 스레드가 ring buffer에 push, `process()`가 pop하여 출력 엣지에 흘려보냄.
  - `GetMixFormat`으로 확인한 채널 수가 `channel_count` 공유 변수에 기록되어 `AudioBuffer.channels`에 반영됨(기본 2).
- 직렬화 데이터:

```rust
struct AppAudioCaptureNode {
  id: String,
  process_id: u32,        // get_window_list 결과에서 선택
  window_title: String,   // 표시용
}
```

- 비-Windows 빌드에서는 `init()`이 `Err("AppAudioCapture requires Windows")`를 반환한다.

---

## 6. SpectrumAnalyzer (`spectrumAnalyzer`)

FFT 기반 주파수 스펙트럼을 계산하는 **Passthrough** 시각화 노드. 입력을 그대로 출력에 전달하면서, 별도의 공유 버퍼에 magnitude bins를 갱신한다.

- 파일: `crates/tauri/src/nodes/spectrum_analyzer.rs`, `src/nodes/SpectrumAnalyzer.tsx`
- 핸들: 입력 1, 출력 1
- 동작:
  - `init()`에서 `rustfft::FftPlanner::plan_fft_forward(fft_size)`로 FFT plan 생성.
  - `process()`는 입력 샘플을 누적기에 모으다 `fft_size`에 도달하면 FFT를 수행하고, magnitude(`fft_size/2` bins)를 `Runtime.spectrum_buffers[id]`에 기록한다. 50% 오버랩으로 누적기 앞 절반을 버린다.
  - 입력은 그대로 출력 엣지에 forward.
- 프론트엔드는 `get_node_render_data` 폴링(30 fps)으로 `NodeRenderData::SpectrumAnalyzer { bins }`를 받아 그린다.
- 직렬화 데이터:

```rust
struct SpectrumAnalyzerNode {
  id: String,
  fft_size: usize,    // 2의 거듭제곱; 기본 1024
}
```

---

## 7. WaveformMonitor (`waveformMonitor`)

최근 N 샘플의 시간 도메인 파형을 보여주는 **Passthrough** 시각화 노드.

- 파일: `crates/tauri/src/nodes/waveform_monitor.rs`, `src/nodes/WaveformMonitor.tsx`
- 핸들: 입력 1, 출력 1
- 동작: 입력 샘플을 rolling 버퍼에 누적하고 가장 오래된 것부터 폐기. 입력은 그대로 출력으로 전달. 시각화 데이터는 `Runtime.waveform_buffers[id]`에 기록되어 IPC 폴링으로 전달된다.
- 직렬화 데이터:

```rust
struct WaveformMonitorNode {
  id: String,
  window_size: usize,   // 기본 2048
}
```

---

## 8. Mixer (`mixer`)

두 개의 명명된 입력(`input-a`, `input-b`)을 element-wise로 더하는 **Passthrough** 노드. 출력은 `[-1.0, 1.0]`로 클램프된다.

- 파일: `crates/tauri/src/nodes/mixer.rs`, `src/nodes/Mixer.tsx`
- 핸들:
  - 입력 2개: `input-a`, `input-b`
  - 출력 1개
- 동작: 두 입력 `AudioBuffer.samples`의 길이를 맞추어 `(a + b).clamp(-1.0, 1.0)`. 한쪽만 연결돼 있으면 그쪽 신호가 그대로 통과.
- 직렬화 데이터:

```rust
struct MixerNode { id: String }
```

---

## 9. Gain (`gain`)

선형 증폭/감쇠 후 클램프. 가장 단순한 이펙터이며 모든 채널에 동일하게 적용된다.

- 파일: `crates/tauri/src/nodes/gain.rs`, `src/nodes/Gain.tsx`
- 핸들: 입력 1, 출력 1
- 동작: `out[n] = (in[n] * gain).clamp(-1.0, 1.0)`
- 직렬화 데이터:

```rust
struct GainNode {
  id: String,
  gain: f32,    // 0.0 ~ 4.0; 기본 1.0
}
```

---

## 10. ChannelMerge (`channelMerge`)

여러 모노 입력을 하나의 인터리브된 멀티채널 출력으로 합치는 노드.

- 파일: `crates/tauri/src/nodes/channel_merge.rs`, `src/nodes/ChannelMerge.tsx`
- 핸들:
  - 입력 N개: `ch-0`, `ch-1`, ... `ch-(N-1)`
  - 출력 1개
- 동작: 각 입력 슬라이스를 인덱스에 맞춰 인터리브. 미연결 채널은 무음(0). `input_count`(2/4/6/8)만큼 채널 수가 결정된다.
- 직렬화 데이터:

```rust
struct ChannelMergeNode {
  id: String,
  input_count: u16,   // 2, 4, 6, 또는 8
}
```

---

## 11. ChannelSplit (`channelSplit`)

인터리브된 멀티채널 입력을 채널별 모노 출력으로 분리.

- 파일: `crates/tauri/src/nodes/channel_split.rs`, `src/nodes/ChannelSplit.tsx`
- 핸들:
  - 입력 1개
  - 출력 N개: `ch-0`, `ch-1`, ...
- 동작: 입력 `AudioBuffer`의 `channels` 값을 기준으로 `chunks_exact(channels)`로 프레임을 잘라 각 채널을 추출. 핸들이 없는(`fromHandle`이 비어 있는) 엣지에는 입력 전체가 그대로 전달된다(레거시 호환).
- 직렬화 데이터:

```rust
struct ChannelSplitNode { id: String }
```

---

## 12. Delay (`delay`)

피드백 없는 단순 딜레이. 지연된 신호만 출력하며 dry/wet 믹스는 없다(피드백/믹스가 필요하면 Echo 사용).

- 파일: `crates/tauri/src/nodes/delay.rs`, `src/nodes/Delay.tsx`
- 핸들: 입력 1, 출력 1
- 동작: 환형 버퍼에 입력을 쓰고 `delay_samples` 만큼 뒤의 위치에서 읽어 출력. 채널 수와 샘플 레이트가 변하면 버퍼를 재할당.
- 직렬화 데이터:

```rust
struct DelayNode {
  id: String,
  delay_ms: f32,    // 0 ~ 2000 ms; 기본 250 ms
}
```

---

## 13. Echo (`echo`)

피드백 에코(테이프 에코 스타일).

- 파일: `crates/tauri/src/nodes/echo.rs`, `src/nodes/Echo.tsx`
- 핸들: 입력 1, 출력 1
- 동작:

```
output[n] = (1 - wet) * input[n] + wet * delayed[n]
buffer[n] = (input[n] + feedback * delayed[n]).clamp(-1.0, 1.0)
```

`feedback`은 안전을 위해 내부에서 `[0.0, 0.95]`로 클램프된다.

- 직렬화 데이터:

```rust
struct EchoNode {
  id: String,
  delay_ms: f32,    // 0 ~ 2000 ms; 기본 375 ms
  feedback: f32,    // 0.0 ~ 0.95
  wet: f32,         // 0.0 (dry only) ~ 1.0 (wet only); 기본 0.5
}
```

---

## 14. Reverb (`reverb`)

Freeverb 스타일 스테레오 리버브 (8개 병렬 comb + 4개 직렬 all-pass, 좌/우 채널마다 STEREO_SPREAD만큼 오프셋).

- 파일: `crates/tauri/src/nodes/reverb.rs`, `src/nodes/Reverb.tsx`
- 핸들: 입력 1, 출력 1
- 직렬화 데이터:

```rust
struct ReverbNode {
  id: String,
  room_size: f32,   // comb feedback (0.0 ~ ~1.0)
  wet: f32,
  damp: f32,        // comb damping
}
```

- 참고: <https://ccrma.stanford.edu/~jos/pasp/Freeverb.html>

---

## 15. Compressor (`compressor`)

피크 엔벨로프 기반 feed-forward 컴프레서. 게인 계산은 dB 도메인에서 수행한다.

- 파일: `crates/tauri/src/nodes/compressor.rs`, `src/nodes/Compressor.tsx`
- 핸들: 입력 1, 출력 1
- 동작:
  1. 어택/릴리즈 계수로 피크 엔벨로프 추적
  2. 엔벨로프를 dB로 변환
  3. 임계값 초과분에 ratio 적용해 감쇠량 산출
  4. make-up gain 적용
- 직렬화 데이터:

```rust
struct CompressorNode {
  id: String,
  threshold_db: f32,   // -60 ~ 0; 기본 -12
  ratio: f32,          // 1:1 ~ ~20:1; 기본 4
  attack_ms: f32,      // 기본 5
  release_ms: f32,     // 기본 50
  make_up_db: f32,     // 기본 0
}
```

---

## 16. Vst (`vst`)

VST3 플러그인 호스트 노드. `libloading`으로 .vst3 DLL을 동적 로드하고 COM vtable을 통해 `IComponent` / `IAudioProcessor`를 호출한다. Windows에서는 별도의 에디터 스레드를 spawn하여 JUCE 등의 메시지 루프 요구사항을 만족시킨다.

- 파일: `crates/tauri/src/nodes/vst.rs`, `src/nodes/VstNode.tsx`, `crates/tauri/src/vst3_common.rs`
- 핸들: 입력 N개 (`vst-in-0` ... `vst-in-(num_inputs-1)`), 출력 N개 (`vst-out-0` ... `vst-out-(num_outputs-1)`)
  - 현재 처리 경로는 출력 버스 0만 사용한다([known-issues](known-issues.md) 참고).
- 직렬화 데이터:

```rust
struct VstNode {
  id: String,
  plugin_path: String,   // 절대 경로
  num_inputs: u16,
  num_outputs: u16,
  channels: u16,
  params: Vec<f64>,
}
```

### IPC 명령

`plugin_command(pluginType: "vst", data)`:

| op       | 인자 | 반환                | 설명                  |
| -------- | ---- | ------------------- | --------------------- |
| `"scan"` | 없음 | `Vec<VstPluginInfo>` | 시스템의 VST3 플러그인 스캔 |

`node_command(nodeId, data)` → `VstNode::command(data)`:

| op             | 인자                                | 반환             | 설명                             |
| -------------- | ----------------------------------- | ---------------- | -------------------------------- |
| `"getParams"`  | 없음                                | `Vec<VstParamInfo>` | 현재 파라미터 값 스냅샷         |
| `"setParam"`   | `paramId: u32, value: f64`          | `null`           | 파라미터 변경                    |
| `"openEditor"` | `pluginPath: string` (Windows 전용) | `null`           | 플러그인 GUI 윈도우 표시        |
| `"closeEditor"`| 없음 (Windows 전용)                 | `null`           | 에디터 윈도우 닫기              |

알려진 한계는 [`docs/known-issues.md`](known-issues.md)의 VST3 섹션을 참고한다.

---

## 노드 타입 빠른 분류

| 분류           | 노드                                                                                                           |
| -------------- | -------------------------------------------------------------------------------------------------------------- |
| Source         | `audioInputDevice`, `virtualAudioOutput`, `appAudioCapture`                                                    |
| Sink           | `audioOutputDevice`, `virtualAudioInput`                                                                       |
| Passthrough    | `mixer`, `gain`, `delay`, `echo`, `reverb`, `compressor`, `channelMerge`, `channelSplit`, `vst`                |
| Visualizer     | `spectrumAnalyzer`, `waveformMonitor` (Passthrough이지만 추가로 IPC 폴링 데이터 제공)                          |
| Windows 전용   | `virtualAudioInput`, `virtualAudioOutput`, `appAudioCapture`, `vst`(에디터)                                    |
| 드라이버 필요  | `virtualAudioInput`, `virtualAudioOutput`                                                                      |
