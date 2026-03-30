# Cable - Audio Input/Output 노드 구현 계획

## 목표

`crates/tauri/src/nodes/` 하위의 `AudioInputDeviceNode`과 `AudioOutputDeviceNode`를 완전히 구현하여, cpal의 input/output 스트림을 통한 실시간 오디오 라우팅을 가능하게 한다.

## 설계 결정 사항

| 항목 | 결정 | 근거 |
|------|------|------|
| 스레드 간 오디오 데이터 전달 | `ringbuf` crate | lock-free, cpal 콜백 스레드에 적합 |
| NodeTrait::process() 시그니처 | `&mut self`로 변경 | ring buffer consumer를 노드 속성으로 저장 |
| 런타임 루프 | Timed sleep loop | buffer_size/sample_rate 기반 sleep으로 CPU 절약 |

## 현재 상태 분석

### 문제점

1. **Input stream 콜백이 비어있음** (`audio_input_device.rs:44`)
2. **Stream 객체가 저장되지 않음** - init()에서 생성 후 즉시 drop
3. **Output 노드 trait 미완성** - init(), dispose() 미구현 (컴파일 불가)
4. **스레드 간 데이터 전달 메커니즘 없음**
5. **process()가 &self** - 내부 상태 변경 불가
6. **App.tsx에서 setup_runtime 호출 시 host 누락**
7. **Runtime 종료 메커니즘 없음** - unpark()만 호출

## 구현 단계

### Phase 1: 의존성 추가 및 기반 구조 변경

#### 1.1 ringbuf crate 추가
- **파일**: `crates/tauri/Cargo.toml`
- **작업**: `ringbuf = "0.4"` 의존성 추가

#### 1.2 NodeTrait 시그니처 변경
- **파일**: `crates/tauri/src/nodes/mod.rs`
- **작업**: `process(&self, ...)` → `process(&mut self, ...)` 변경
- **영향**: `runtime.rs`의 process() 루프에서 `&dyn NodeTrait` → `&mut dyn NodeTrait`로 변경 필요

#### 1.3 Runtime 구조체에 종료 플래그 추가
- **파일**: `crates/tauri/src/runtime.rs`
- **작업**:
  - `Runtime`에 `running: Arc<AtomicBool>` 필드 추가
  - `process()` 루프에서 running 플래그 체크
  - buffer_size / sample_rate 기반 timed sleep 추가

#### 1.4 Runtime의 노드 순회를 &mut로 변경
- **파일**: `crates/tauri/src/runtime.rs`
- **작업**: `runtime_nodes: Vec<&dyn NodeTrait>` → `&mut dyn NodeTrait`로 변경
  - `AudioNode` enum에 `as_mut_node_trait()` 메서드 추가하거나
  - `process()`에서 직접 match로 &mut 참조 획득

### Phase 2: AudioInputDeviceNode 구현

#### 2.1 노드 구조체 확장
- **파일**: `crates/tauri/src/nodes/audio_input_device.rs`
- **작업**:
  ```rust
  pub(crate) struct AudioInputDeviceNode {
    id: String,
    device: AudioDevice,
    // 새로 추가
    stream: Option<cpal::Stream>,
    ring_consumer: Option<ringbuf::HeapCons<f32>>,
  }
  ```
  - `#[serde(skip)]`으로 직렬화에서 제외
  - `Clone` derive 제거 (Stream은 Clone 불가) 또는 수동 Clone 구현

#### 2.2 init() 구현
- **파일**: `crates/tauri/src/nodes/audio_input_device.rs`
- **작업**:
  1. `ringbuf::HeapRb::<f32>::new(buffer_size * channels * 4)` 로 링 버퍼 생성
  2. `(producer, consumer)` 분리
  3. `build_input_stream` 콜백에서 producer에 데이터 push
  4. consumer를 `self.ring_consumer`에 저장
  5. Stream을 `self.stream`에 저장하여 drop 방지
  6. `stream.play()` 호출하여 스트림 시작

  ```rust
  fn init(&mut self, runtime: &Runtime) -> Result<(), String> {
    let rb = HeapRb::<f32>::new(runtime.buffer_size as usize * self.device.channels as usize * 4);
    let (mut producer, consumer) = rb.split();

    let stream = device.build_input_stream(
      &config,
      move |data: &[f32], _: &cpal::InputCallbackInfo| {
        producer.push_slice(data);
      },
      move |err| { eprintln!("Input stream error: {}", err); },
      None,
    )?;
    stream.play()?;

    self.stream = Some(stream);
    self.ring_consumer = Some(consumer);
    Ok(())
  }
  ```

#### 2.3 process() 구현
- **파일**: `crates/tauri/src/nodes/audio_input_device.rs`
- **작업**:
  1. ring_consumer에서 사용 가능한 데이터 읽기
  2. 이 노드에 연결된 출력 엣지들의 ID를 `runtime.edges`에서 찾기
  3. 각 출력 엣지에 대해 읽은 오디오 데이터를 BTreeMap에 삽입하여 반환

  ```rust
  fn process(&mut self, runtime: &Runtime, _state: &RuntimeState) -> Result<BTreeMap<String, Vec<f32>>, String> {
    let consumer = self.ring_consumer.as_mut().ok_or("Not initialized")?;
    let available = consumer.occupied_len();
    if available == 0 {
      return Ok(BTreeMap::new());
    }

    let mut buffer = vec![0.0f32; available];
    consumer.pop_slice(&mut buffer);

    let mut output = BTreeMap::new();
    for edge in &runtime.edges {
      if edge.from == self.id {
        output.insert(edge.id.clone(), buffer.clone());
      }
    }
    Ok(output)
  }
  ```

#### 2.4 dispose() 구현
- **파일**: `crates/tauri/src/nodes/audio_input_device.rs`
- **작업**: stream과 ring_consumer를 drop
  ```rust
  fn dispose(&mut self, _runtime: &Runtime) -> Result<(), String> {
    self.stream.take(); // Drop하여 스트림 중지
    self.ring_consumer.take();
    Ok(())
  }
  ```

### Phase 3: AudioOutputDeviceNode 구현

#### 3.1 노드 구조체 확장
- **파일**: `crates/tauri/src/nodes/audio_output_device.rs`
- **작업**:
  ```rust
  pub(crate) struct AudioOutputDeviceNode {
    id: String,
    device: AudioDevice,
    // 새로 추가
    stream: Option<cpal::Stream>,
    ring_producer: Option<ringbuf::HeapProd<f32>>,
  }
  ```

#### 3.2 init() 구현
- **파일**: `crates/tauri/src/nodes/audio_output_device.rs`
- **작업**:
  1. 링 버퍼 생성, (producer, consumer) 분리
  2. `build_output_stream` 콜백에서 consumer로부터 데이터 pop
  3. producer를 `self.ring_producer`에 저장
  4. Stream 저장 및 play()

  ```rust
  fn init(&mut self, runtime: &Runtime) -> Result<(), String> {
    let rb = HeapRb::<f32>::new(runtime.buffer_size as usize * self.device.channels as usize * 4);
    let (producer, mut consumer) = rb.split();

    let stream = device.build_output_stream(
      &config,
      move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
        let read = consumer.pop_slice(data);
        // 데이터 부족 시 나머지를 silence로 채움
        for sample in &mut data[read..] {
          *sample = 0.0;
        }
      },
      move |err| { eprintln!("Output stream error: {}", err); },
      None,
    )?;
    stream.play()?;

    self.stream = Some(stream);
    self.ring_producer = Some(producer);
    Ok(())
  }
  ```

#### 3.3 process() 구현
- **파일**: `crates/tauri/src/nodes/audio_output_device.rs`
- **작업**:
  1. `state.edge_values`에서 이 노드로 향하는 엣지의 데이터를 읽기
  2. ring_producer에 데이터 push
  3. 빈 BTreeMap 반환 (출력 노드이므로 하류 엣지 없음)

  ```rust
  fn process(&mut self, runtime: &Runtime, state: &RuntimeState) -> Result<BTreeMap<String, Vec<f32>>, String> {
    let producer = self.ring_producer.as_mut().ok_or("Not initialized")?;

    for edge in &runtime.edges {
      if edge.to == self.id {
        if let Some(data) = state.edge_values.get(&edge.id) {
          producer.push_slice(data);
        }
      }
    }

    Ok(BTreeMap::new())
  }
  ```

#### 3.4 dispose() 구현
- **파일**: `crates/tauri/src/nodes/audio_output_device.rs`
- **작업**: stream과 ring_producer를 drop

### Phase 4: Runtime 수정

#### 4.1 노드 초기화/해제 호출 추가
- **파일**: `crates/tauri/src/lib.rs`
- **작업**:
  - `setup_runtime` 커맨드에서 Runtime 생성 후 각 노드의 `init()` 호출
  - `disable_runtime` 커맨드에서 각 노드의 `dispose()` 호출

#### 4.2 process() 루프에서 &mut 참조 사용
- **파일**: `crates/tauri/src/runtime.rs`
- **작업**: `process(&mut self)` 로 변경, 노드를 `&mut dyn NodeTrait`로 순회

#### 4.3 Timed sleep loop 구현
- **파일**: `crates/tauri/src/runtime.rs`, `crates/tauri/src/lib.rs`
- **작업**:
  ```rust
  // enable_runtime에서
  let running = Arc::new(AtomicBool::new(true));
  let running_clone = running.clone();
  let sleep_duration = Duration::from_secs_f64(
    runtime.buffer_size as f64 / sample_rate as f64
  );

  std::thread::spawn(move || {
    while running_clone.load(Ordering::Relaxed) {
      runtime.process().unwrap_or_else(|e| eprintln!("{}", e));
      std::thread::sleep(sleep_duration);
    }
  });
  ```

#### 4.4 RuntimeState.edge_values 가시성 변경
- **파일**: `crates/tauri/src/runtime.rs`
- **작업**: `edge_values` 필드를 `pub`으로 변경 (노드에서 접근 필요)

### Phase 5: Serde 호환성 해결

#### 5.1 AudioNode enum의 Clone 문제 해결
- **파일**: `crates/tauri/src/lib.rs`
- **작업**:
  - `AudioNode` enum에서 `Clone` derive 제거 (Stream은 Clone 불가)
  - 또는 노드 구조체에서 직렬화 불가능한 필드에 `#[serde(skip)]` 적용하고 수동 Clone 구현
  - `AudioInputDeviceNode`과 `AudioOutputDeviceNode`에서 `Clone` derive 제거, Deserialize만 유지

### Phase 6: 프론트엔드 버그 수정

#### 6.1 setup_runtime 호출 시 host 전달
- **파일**: `src/App.tsx`
- **작업**: `invoke("setup_runtime", { graph, bufferSize: 512 })` → `invoke("setup_runtime", { graph, host: selectedAudioHost, bufferSize: 512 })` 로 수정

## 수정 대상 파일 요약

| 파일 | 수정 유형 | 설명 |
|------|-----------|------|
| `crates/tauri/Cargo.toml` | 수정 | ringbuf 의존성 추가 |
| `crates/tauri/src/nodes/mod.rs` | 수정 | process() → &mut self |
| `crates/tauri/src/nodes/audio_input_device.rs` | 전면 재작성 | 스트림 생성, 링버퍼, process 구현 |
| `crates/tauri/src/nodes/audio_output_device.rs` | 전면 재작성 | 스트림 생성, 링버퍼, process 구현 |
| `crates/tauri/src/runtime.rs` | 수정 | &mut self, timed sleep, edge_values pub |
| `crates/tauri/src/lib.rs` | 수정 | init/dispose 호출, 종료 메커니즘, Clone 제거 |
| `src/App.tsx` | 수정 | host 인자 추가 |

## 스레드 모델 (구현 후)

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

## 주의사항

- cpal의 `Stream`은 `Send`이지만 `Sync`나 `Clone`이 아님. 스트림을 노드 구조체에 저장할 때 주의 필요.
- ringbuf의 Producer/Consumer는 각각 다른 스레드에서 사용 가능 (Send). Producer는 cpal 콜백으로 move, Consumer는 노드에 보관 (또는 반대).
- `dispose()` 시 `Stream`을 drop하면 cpal이 자동으로 스트림을 중지함.
- ring buffer 크기는 `buffer_size * channels * N` (N은 여유 배수, 4 정도)으로 설정하여 언더런/오버런 방지.
