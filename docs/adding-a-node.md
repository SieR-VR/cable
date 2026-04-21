# 새 노드 추가 가이드

Cable에서 새 오디오 처리 노드를 추가하는 전체 과정을 Spectrum Analyzer 구현을 예시로 설명한다.

---

## 1. 설계 결정

노드를 구현하기 전에 아래 질문에 답해야 한다.

| 질문 | 선택지 |
|------|--------|
| 노드의 역할 | Source(출력만), Sink(입력만), Passthrough(입출력 모두) |
| 시각화/연산 위치 | Rust 백엔드 vs 프론트엔드 |
| 노드가 필요한 초기 데이터 | 프론트엔드에서 직렬화해 Rust에 전달할 필드 목록 |

**Spectrum Analyzer의 선택**

- **Passthrough**: 오디오를 통과시키면서 주파수 분석 결과를 시각화한다.
- **FFT 연산 위치**: Rust 백엔드. 샘플 단위 실시간 연산이므로 JS 스레드 차단을 피해야 한다.
- **공유 버퍼**: `Arc<Mutex<Vec<f32>>>` 를 통해 오디오 스레드와 Tauri 커맨드 스레드가 안전하게 데이터를 교환한다.
- **프론트엔드 폴링**: `setInterval(33ms)` 로 ~30fps 속도로 `get_spectrum_data` 커맨드를 호출한다.

---

## 2. 구현 체크리스트

노드를 추가할 때 **반드시** 수정해야 하는 파일 목록이다. 하나라도 빠지면 런타임 오류가 발생한다.

```
[ ] crates/tauri/Cargo.toml          — 필요한 크레이트 추가
[ ] crates/tauri/src/nodes/mod.rs    — pub mod 등록
[ ] crates/tauri/src/nodes/<name>.rs — NodeTrait 구현체 (신규 파일)
[ ] crates/tauri/src/lib.rs
    - AudioNode 열거형에 variant 추가
    - AppData에 공유 상태 추가 (있는 경우)
    - setup_runtime에 초기화 로직 추가
    - invoke_handler에 새 커맨드 등록
[ ] crates/tauri/src/runtime.rs
    - Runtime 구조체에 공유 상태 추가 (있는 경우)
    - Runtime::new() 파라미터 추가
    - node_id(), init_nodes(), dispose_nodes(), process() 각 match 구문에 variant 추가
[ ] src/nodes/<Name>.tsx              — React 컴포넌트 (신규 파일)
[ ] src/types.ts
    - nodeTypes 객체에 등록
    - NodeType 유니온에 추가
    - AudioNode.type 유니온에 추가
[ ] src/ipc.d.ts                     — 새 커맨드 타입 오버로드 추가
[ ] src/App.tsx (onApply)            — 노드 데이터 직렬화 분기 추가
[ ] src/components/ContextMenu.tsx   — NODE_CATEGORIES 배열에 항목 추가
[ ] src/state.ts (addNodeAtContextMenu) — 노드 타입 및 초기 data 처리 추가
```

> **중요**: `App.tsx`의 `onApply` 직렬화 분기를 빠뜨리면 "missing field `<fieldName>`" 오류가 발생한다 (Spectrum Analyzer 구현 중 실제로 마주친 버그).

---

## 3. Rust 구현

### 3-1. 크레이트 추가

```toml
# crates/tauri/Cargo.toml
[dependencies]
rustfft = "6"
```

### 3-2. NodeTrait 구현체

`crates/tauri/src/nodes/spectrum_analyzer.rs` 를 신규 작성한다.

**핵심 구조체**

```rust
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SpectrumAnalyzerNode {
  id: String,
  fft_size: usize,           // 프론트엔드에서 직렬화되어 전달됨

  #[serde(skip)]             // 런타임에서만 사용; 직렬화 제외
  spectrum_out: Option<Arc<Mutex<Vec<f32>>>>,
  #[serde(skip)]
  fft: Option<Arc<dyn rustfft::Fft<f32>>>,
  #[serde(skip)]
  sample_accumulator: Vec<f32>,
}
```

`#[serde(skip)]` 필드들은 IPC로 전달되지 않고 `init()` 시 런타임이 주입한다.

**NodeTrait 계약**

```rust
impl NodeTrait for SpectrumAnalyzerNode {
  fn id(&self) -> &str { &self.id }

  fn init(&mut self, runtime: &Runtime) -> Result<(), String> {
    // Runtime에서 공유 Arc 꺼내기
    let arc = runtime.spectrum_buffers.get(&self.id).ok_or(...)?.clone();
    self.spectrum_out = Some(arc);
    // FFT 플랜 캐싱
    self.fft = Some(FftPlanner::new().plan_fft_forward(self.fft_size));
    Ok(())
  }

  fn dispose(&mut self, _: &Runtime) -> Result<(), String> {
    self.spectrum_out = None;
    self.fft = None;
    Ok(())
  }

  fn process(&mut self, runtime: &Runtime, state: &RuntimeState)
    -> Result<BTreeMap<String, Vec<f32>>, String>
  {
    // 1. 업스트림 샘플 수집
    // 2. 충분히 쌓이면 FFT 실행 → Arc 버퍼에 결과 기록
    // 3. 원본 샘플을 다운스트림 엣지로 그대로 전달 (passthrough)
  }
}
```

**FFT 알고리즘 세부 사항**

- 윈도우: Hann window (`0.5 * (1 - cos(2π·i/(N-1)))`)
- 오버랩: 50% (FFT 후 앞쪽 `fft_size / 2` 샘플 drain)
- 출력: 양의 주파수 성분 `fft_size / 2` 개 magnitude bin, `fft_size` 로 정규화

### 3-3. 공유 버퍼 등록 (lib.rs + runtime.rs)

오디오 스레드(`Runtime`)와 Tauri 커맨드 스레드(`get_spectrum_data`) 사이에서 데이터를 교환하려면 `Arc<Mutex<Vec<f32>>>` 를 두 곳 모두에 등록해야 한다.

```rust
// AppData (lib.rs)
struct AppData {
  // ...
  spectrum_buffers: BTreeMap<String, Arc<Mutex<Vec<f32>>>>,
}

// Runtime (runtime.rs)
pub(crate) struct Runtime {
  // ...
  pub(crate) spectrum_buffers: BTreeMap<String, Arc<Mutex<Vec<f32>>>>,
}
```

`setup_runtime` 커맨드에서 초기화 순서:

```rust
// 1. nodes를 순회하여 SpectrumAnalyzer마다 Arc 생성
let mut spectrum_buffers = BTreeMap::new();
for node in &graph.nodes {
  if let AudioNode::SpectrumAnalyzer(n) = node {
    spectrum_buffers.insert(n.id().to_string(), Arc::new(Mutex::new(vec![])));
  }
}
// 2. AppData에도 동일한 Arc 저장 (커맨드 스레드가 읽을 용도)
app_state.spectrum_buffers = spectrum_buffers.clone();
// 3. AppData 락 해제 후 Runtime 생성 (오디오 스레드에 Arc 전달)
drop(app_state);
let mut runtime = Runtime::new(..., spectrum_buffers);
```

> `AppData` 락을 `drop` 하기 전에 `Runtime::new()` 를 호출하면 오디오 스레드가 시작되기 전에 데드락이 발생할 수 있으므로 반드시 락을 먼저 해제한다.

### 3-4. Tauri 커맨드

```rust
#[tauri::command]
fn get_spectrum_data(
  node_id: String,
  app_state: State<Mutex<AppData>>,
) -> Result<Vec<f32>, String> {
  let state = app_state.lock().map_err(|e| e.to_string())?;
  let buf = state.spectrum_buffers
    .get(&node_id)
    .ok_or_else(|| format!("No spectrum buffer for node '{}'", node_id))?;
  Ok(buf.lock().map_err(|e| e.to_string())?.clone())
}
```

---

## 4. 프론트엔드 구현

### 4-1. React 컴포넌트

`src/nodes/SpectrumAnalyzer.tsx`

```tsx
export type SpectrumAnalyzerNodeData = {
  fftSize: number;
  edgeType: string | null;
};
export type SpectrumAnalyzerNode = Node<SpectrumAnalyzerNodeData, "spectrumAnalyzer">;

export default function SpectrumAnalyzer({ id }: NodeProps<SpectrumAnalyzerNode>) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const intervalId = setInterval(async () => {
      try {
        const bins = await invoke<number[]>("get_spectrum_data", { nodeId: id });
        drawSpectrum(canvasRef.current, bins);
      } catch { /* 노드 초기화 전 무시 */ }
    }, 33); // ~30fps

    return () => clearInterval(intervalId); // 언마운트 시 정리
  }, [id]);

  return (
    <div className="bg-gray-700 rounded-lg ...">
      <Handle type="target" position={Position.Left} ... />
      <canvas ref={canvasRef} width={240} height={80} />
      <Handle type="source" position={Position.Right} ... />
    </div>
  );
}
```

### 4-2. types.ts 등록

```ts
export const nodeTypes = {
  // ...기존 항목...
  spectrumAnalyzer: SpectrumAnalyzer,
} satisfies NodeTypes;

export type NodeType = ... | SpectrumAnalyzerNode;

export type AudioNode = {
  type: ... | "spectrumAnalyzer";
  data: ... | { fftSize: number; id: string };
};
```

### 4-3. ipc.d.ts 오버로드

```ts
declare function invoke(
  cmd: "get_spectrum_data",
  args: { nodeId: string },
): Promise<number[]>;
```

### 4-4. App.tsx 직렬화 분기 (필수)

`onApply`에서 각 노드 타입별로 Rust가 기대하는 필드만 골라 전달한다.

```ts
if (node.type === "spectrumAnalyzer") {
  return {
    type: node.type,
    data: {
      id: node.id,
      fftSize: (node.data as any).fftSize ?? 1024,
    },
  };
}
```

이 분기가 없으면 기본 분기가 `{ id, device: null }` 을 전송하고 Rust에서
`"missing field 'fftSize'"` 오류가 발생한다.

### 4-5. ContextMenu 등록

`NODE_CATEGORIES` 배열에 노드를 추가한다.

```ts
const NODE_CATEGORIES: NodeCategory[] = [
  // ...
  {
    label: "Visualizers",
    items: [{ type: "spectrumAnalyzer", label: "Spectrum Analyzer" }],
  },
];
```

### 4-6. state.ts (addNodeAtContextMenu)

```ts
// 타입 시그니처에 추가
addNodeAtContextMenu: (
  type: "audioInputDevice" | ... | "spectrumAnalyzer",
) => void;

// 초기 데이터 분기 추가
const data = isSpectrumAnalyzer
  ? { fftSize: 1024, edgeType: null }
  : ...;
```

---

## 5. 테스트

### 5-1. Rust 유닛 테스트

`spectrum_analyzer.rs` 하단의 `#[cfg(test)]` 블록에 작성한다.
Runtime 없이 테스트하려면 `Arc<Mutex<Vec<f32>>>` 와 FFT 플랜을 직접 주입한다.

```rust
fn init_node(node: &mut SpectrumAnalyzerNode) {
  node.spectrum_out = Some(Arc::new(Mutex::new(Vec::new())));
  node.fft = Some(FftPlanner::new().plan_fft_forward(node.fft_size));
  node.sample_accumulator = Vec::with_capacity(node.fft_size * 2);
}
```

포함한 테스트:

| 테스트 | 검증 내용 |
|--------|-----------|
| `test_fft_output_length` | 출력 bin 개수 = `fft_size / 2` |
| `test_spectrum_nonnegative` | 모든 magnitude ≥ 0 |
| `test_accumulator_no_update_below_fft_size` | 샘플 부족 시 스펙트럼 미갱신 |
| `test_50_percent_overlap_drain` | 50% 오버랩 후 누산기 길이 검증 |

### 5-2. 프론트엔드 컴포넌트 테스트

`@xyflow/react` 의 `Handle` 컴포넌트는 내부에서 React Flow Zustand 스토어에 접근하므로,
**`ReactFlowProvider`** 로 감싸지 않으면 테스트가 크래시된다.

```tsx
function renderInProvider(id?: string) {
  return render(
    <ReactFlowProvider>
      <SpectrumAnalyzer {...makeProps(id)} />
    </ReactFlowProvider>
  );
}
```

`makeProps` 에는 React Flow `NodeProps` 가 요구하는 모든 필드를 포함해야 한다
(`draggable`, `selectable`, `deletable`, `positionAbsoluteX`, `positionAbsoluteY`).

포함한 테스트:

| 테스트 | 검증 내용 |
|--------|-----------|
| `renders target and source handles` | Handle 2개 렌더링 |
| `renders header label` | "Spectrum Analyzer" 텍스트 |
| `renders canvas element` | canvas 요소 존재 |
| `starts polling on mount` | setInterval 호출 |
| `polls at ~30fps` | 33ms 간격 |
| `clears interval on unmount` | clearInterval 호출 |

---

## 6. 발생한 버그 및 해결

### Bug 1: `missing field 'fftSize'`

**증상**: "Apply" 버튼 클릭 시 `invalid args 'graph' for command 'setup_runtime': missing field 'fftSize'`

**원인**: `App.tsx`의 `onApply` 직렬화 핸들러에 `spectrumAnalyzer` 분기가 없어
기본 분기(`{ id, device: null }`)로 처리됨.

**수정**: `App.tsx`에 `spectrumAnalyzer` 전용 분기 추가.

### Bug 2: 테스트 타입 오류 (`NodeProps` 불완전한 캐스트)

**증상**: `tsc --noEmit` 실패 — `NodeProps<SpectrumAnalyzerNode>`에 `draggable`, `selectable`, `deletable`, `positionAbsoluteX`, `positionAbsoluteY` 필드 누락.

**원인**: React Flow v12의 `NodeProps` 타입이 확장됨.

**수정**: `makeProps` 헬퍼에 누락 필드 추가.

---

## 7. 완성된 파일 구조

```
crates/tauri/
  Cargo.toml                          rustfft = "6" 추가
  src/
    lib.rs                            AudioNode variant, AppData, 커맨드, invoke_handler
    runtime.rs                        Runtime struct, new(), match 구문
    nodes/
      mod.rs                          pub mod spectrum_analyzer
      spectrum_analyzer.rs            SpectrumAnalyzerNode 구현 + 유닛 테스트 (4개)

src/
  App.tsx                             onApply 직렬화 분기 추가
  ipc.d.ts                            get_spectrum_data 오버로드
  types.ts                            nodeTypes, NodeType, AudioNode 등록
  state.ts                            addNodeAtContextMenu 타입 + 초기 data
  components/
    ContextMenu.tsx                   NODE_CATEGORIES에 Visualizers 추가
  nodes/
    SpectrumAnalyzer.tsx              React 컴포넌트
  test/nodes/
    SpectrumAnalyzer.test.tsx         컴포넌트 테스트 (6개)
```

---

## 8. 빌드 및 검증 명령

```powershell
# Rust 컴파일 + 테스트
cargo check --workspace
cargo test --workspace

# Release 빌드 확인 (CI와 동일한 조건)
cargo check --release --workspace

# 프론트엔드 타입 체크 + 테스트
npx tsc --noEmit
npx vitest run
```
