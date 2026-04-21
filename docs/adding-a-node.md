# 새 노드 추가 가이드

Cable에서 새 오디오 처리 노드를 추가하는 전체 과정을 설명한다.

---

## 1. 설계 결정

노드를 구현하기 전에 아래 질문에 답해야 한다.

| 질문 | 선택지 |
|------|--------|
| 노드의 역할 | **Source** — 출력 엣지만 가짐 (예: 오디오 입력 장치) |
| | **Sink** — 입력 엣지만 가짐 (예: 오디오 출력 장치) |
| | **Passthrough** — 입력과 출력 엣지 모두 가짐 (예: 이펙터, 분석기) |
| 무거운 연산 위치 | **Rust 백엔드** — 샘플 단위 실시간 연산, JS 스레드 차단 불가 시 |
| | **프론트엔드** — 시각화 전용이거나 연산 부하가 낮을 때 |
| 노드의 초기 설정값 | 프론트엔드에서 직렬화해 Rust `setup_runtime`에 전달할 필드 목록 |
| 오디오 스레드와 데이터 교환 필요 여부 | Tauri 커맨드에서 노드 내부 데이터를 읽어야 하면 `Arc<Mutex<T>>` 공유 버퍼 패턴 사용 |

---

## 2. 구현 체크리스트

노드를 추가할 때 **반드시** 수정해야 하는 파일 목록이다. 하나라도 빠지면 런타임 오류가 발생한다.

```
[ ] crates/tauri/Cargo.toml              — 필요한 외부 크레이트 추가 (없으면 생략)
[ ] crates/tauri/src/nodes/mod.rs        — pub mod <node_name> 등록
[ ] crates/tauri/src/nodes/<node_name>.rs — NodeTrait 구현체 (신규 파일)
[ ] crates/tauri/src/lib.rs
    - AudioNode 열거형에 variant 추가
    - AppData에 공유 상태 추가 (필요한 경우)
    - setup_runtime에 초기화 로직 추가
    - invoke_handler에 새 커맨드 등록 (추가 커맨드가 있는 경우)
[ ] crates/tauri/src/runtime.rs
    - Runtime 구조체에 공유 상태 추가 (필요한 경우)
    - Runtime::new() 파라미터 추가 (공유 상태가 있는 경우)
    - node_id(), init_nodes(), dispose_nodes(), process() 각 match 구문에 variant 추가
[ ] src/nodes/<NodeName>.tsx              — React 컴포넌트 (신규 파일)
[ ] src/types.ts
    - nodeTypes 객체에 등록
    - NodeType 유니온에 추가
    - AudioNode.type 유니온에 추가
    - AudioNode.data 유니온에 노드 data 타입 추가
[ ] src/ipc.d.ts                         — 새 커맨드 타입 오버로드 추가 (있는 경우)
[ ] src/App.tsx (onApply)                — 노드 데이터 직렬화 분기 추가
[ ] src/components/ContextMenu.tsx       — NODE_CATEGORIES 배열에 항목 추가
[ ] src/state.ts (addNodeAtContextMenu)  — 노드 타입 및 초기 data 처리 추가
```

> **주의**: `App.tsx`의 `onApply` 직렬화 분기를 빠뜨리면 기본 분기가 Rust 구조체가 기대하지 않는 필드를 전송하여 `"missing field '<fieldName>'"` 역직렬화 오류가 발생한다.

---

## 3. Rust 구현

### 3-1. 외부 크레이트 추가 (필요한 경우)

```toml
# crates/tauri/Cargo.toml
[dependencies]
some-crate = "1.0"
```

### 3-2. NodeTrait 구현체

`crates/tauri/src/nodes/<node_name>.rs` 를 신규 작성한다.

**핵심 구조체**

```rust
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MyNode {
  /// React Flow 노드 ID와 일치해야 한다.
  id: String,

  /// 프론트엔드에서 직렬화되어 전달되는 설정값 (필드명은 camelCase로 변환됨).
  some_setting: u32,

  /// 런타임에서만 사용되는 상태는 직렬화에서 제외한다.
  #[serde(skip)]
  runtime_state: Option<SomeType>,
}
```

`#[serde(skip)]` 필드는 IPC를 통해 전달되지 않으며, `init()` 호출 시 런타임이 값을 주입한다.

**NodeTrait 계약**

```rust
impl NodeTrait for MyNode {
  fn id(&self) -> &str {
    &self.id
  }

  fn init(&mut self, runtime: &Runtime) -> Result<(), String> {
    // 필요한 리소스 획득 또는 공유 상태 Arc 연결
    Ok(())
  }

  fn dispose(&mut self, _runtime: &Runtime) -> Result<(), String> {
    // 획득한 리소스 해제
    self.runtime_state = None;
    Ok(())
  }

  fn process(
    &mut self,
    runtime: &Runtime,
    state: &RuntimeState,
  ) -> Result<BTreeMap<String, Vec<f32>>, String> {
    // 입력 엣지에서 샘플 읽기 (Sink / Passthrough)
    let mut incoming: Vec<f32> = Vec::new();
    for edge in &runtime.edges {
      if edge.to == self.id {
        if let Some(samples) = state.edge_values.get(&edge.id) {
          incoming.extend_from_slice(samples);
        }
      }
    }

    // 출력 엣지에 샘플 쓰기 (Source / Passthrough)
    let mut output = BTreeMap::new();
    for edge in &runtime.edges {
      if edge.from == self.id {
        output.insert(edge.id.clone(), incoming.clone());
      }
    }

    Ok(output)
  }
}
```

### 3-3. 오디오 스레드와 데이터 교환이 필요한 경우 (선택)

Tauri 커맨드가 오디오 처리 스레드 내부 데이터를 읽어야 할 때 `Arc<Mutex<T>>` 공유 버퍼 패턴을 사용한다.

```rust
// AppData (lib.rs) — 커맨드 스레드에서 접근
struct AppData {
  shared_buffers: BTreeMap<String, Arc<Mutex<OutputData>>>,
}

// Runtime (runtime.rs) — 오디오 스레드에서 접근
pub(crate) struct Runtime {
  pub(crate) shared_buffers: BTreeMap<String, Arc<Mutex<OutputData>>>,
}
```

`setup_runtime` 커맨드에서의 초기화 순서:

```rust
// 1. 노드를 순회하며 Arc 생성
let mut shared_buffers = BTreeMap::new();
for node in &graph.nodes {
  if let AudioNode::MyNode(n) = node {
    shared_buffers.insert(n.id().to_string(), Arc::new(Mutex::new(OutputData::default())));
  }
}

// 2. AppData에 Arc 복제본 저장
app_state.shared_buffers = shared_buffers.clone();

// 3. AppData 락을 반드시 먼저 해제한 뒤 Runtime 생성
//    (Runtime::new 내부에서 오디오 스레드가 시작되므로 락 보유 상태에서 호출하면 데드락 발생)
drop(app_state);
let mut runtime = Runtime::new(..., shared_buffers);
```

### 3-4. Tauri 커맨드 추가 (선택)

```rust
#[tauri::command]
fn get_node_data(
  node_id: String,
  app_state: State<Mutex<AppData>>,
) -> Result<OutputData, String> {
  let state = app_state.lock().map_err(|e| e.to_string())?;
  let buf = state
    .shared_buffers
    .get(&node_id)
    .ok_or_else(|| format!("No data for node '{}'", node_id))?;
  Ok(buf.lock().map_err(|e| e.to_string())?.clone())
}
```

커맨드를 추가했으면 `lib.rs`의 `invoke_handler` 에도 등록한다.

```rust
.invoke_handler(tauri::generate_handler![
  // ...기존 커맨드...
  get_node_data,
])
```

---

## 4. 프론트엔드 구현

### 4-1. React 컴포넌트

`src/nodes/<NodeName>.tsx` 를 신규 작성한다.

```tsx
import { Handle, Node, NodeProps, Position } from "@xyflow/react";

export type MyNodeData = {
  someSetting: number;
  edgeType: string | null;
};

export type MyNodeType = Node<MyNodeData, "myNode">;

export default function MyNode({ id, data }: NodeProps<MyNodeType>) {
  return (
    <div className="bg-gray-700 rounded-lg flex flex-col text-white min-w-48">
      {/* 헤더 — drag-handle__custom 클래스가 있어야 드래그가 동작한다 */}
      <div className="w-full h-6 bg-blue-500 rounded-t-lg flex items-center text-sm font-bold p-2 drag-handle__custom">
        My Node
      </div>
      <div className="flex flex-col gap-2 p-2 relative">
        {/* 노드 본문 UI */}
      </div>
      {/* Passthrough / Sink: 왼쪽 입력 핸들 */}
      <Handle type="target" position={Position.Left} id="myNode-target" />
      {/* Passthrough / Source: 오른쪽 출력 핸들 */}
      <Handle type="source" position={Position.Right} id="myNode-source" />
    </div>
  );
}
```

### 4-2. types.ts 등록

```ts
import MyNode, { MyNodeType } from "./nodes/MyNode";

export const nodeTypes = {
  // ...기존 항목...
  myNode: MyNode,
} satisfies NodeTypes;

export type NodeType = ... | MyNodeType;

export type AudioNode = {
  type: ... | "myNode";
  data: ... | { someSetting: number; id: string };
};
```

### 4-3. ipc.d.ts 오버로드 (추가 커맨드가 있는 경우)

```ts
declare function invoke(
  cmd: "get_node_data",
  args: { nodeId: string },
): Promise<OutputData>;
```

### 4-4. App.tsx 직렬화 분기 (필수)

`onApply`의 노드 직렬화 핸들러에 **반드시** 분기를 추가해야 한다.
없으면 기본 분기(`{ id, device: null }`)로 처리되어 Rust 역직렬화 오류가 발생한다.

```ts
if (node.type === "myNode") {
  return {
    type: node.type,
    data: {
      id: node.id,
      someSetting: (node.data as any).someSetting ?? 0,
    },
  };
}
```

### 4-5. ContextMenu 등록

`src/components/ContextMenu.tsx` 의 `NODE_CATEGORIES` 배열에 항목을 추가한다.

```ts
const NODE_CATEGORIES: NodeCategory[] = [
  // ...기존 카테고리...
  {
    label: "My Category",
    // requiresDriver: true,  // 드라이버가 필요한 노드라면 설정
    items: [{ type: "myNode", label: "My Node" }],
  },
];
```

### 4-6. state.ts (addNodeAtContextMenu)

```ts
// 타입 시그니처에 노드 타입 추가
addNodeAtContextMenu: (
  type: "audioInputDevice" | ... | "myNode",
) => void;

// 초기 data 분기 추가
const isMyNode = type === "myNode";
const data = isMyNode
  ? { someSetting: 0, edgeType: null }
  : /* ...기존 분기... */;
```

---

## 5. 테스트

### 5-1. Rust 유닛 테스트

`nodes/<node_name>.rs` 하단의 `#[cfg(test)]` 블록에 작성한다.
`Runtime` 없이 독립적으로 테스트하려면 `#[serde(skip)]` 필드를 직접 초기화한다.

```rust
#[cfg(test)]
mod tests {
  use super::*;

  fn make_node() -> MyNode {
    MyNode {
      id: "test".to_string(),
      some_setting: 42,
      runtime_state: None,
    }
  }

  fn init_node(node: &mut MyNode) {
    // #[serde(skip)] 필드에 테스트용 값 주입
    node.runtime_state = Some(SomeType::default());
  }

  #[test]
  fn test_something() {
    let mut node = make_node();
    init_node(&mut node);
    // ...
  }
}
```

### 5-2. 프론트엔드 컴포넌트 테스트

`src/test/nodes/<NodeName>.test.tsx` 를 신규 작성한다.

`Handle` 컴포넌트는 React Flow 내부 Zustand 스토어에 접근하므로,
`ReactFlowProvider` 로 반드시 감싸야 한다.

```tsx
import { render } from "@testing-library/react";
import { ReactFlowProvider } from "@xyflow/react";
import MyNode from "@/nodes/MyNode";

function makeProps(id = "node-1"): Parameters<typeof MyNode>[0] {
  return {
    id,
    type: "myNode" as const,
    data: { someSetting: 0, edgeType: null },
    selected: false,
    isConnectable: true,
    zIndex: 0,
    xPos: 0,
    yPos: 0,
    positionAbsoluteX: 0,
    positionAbsoluteY: 0,
    dragging: false,
    draggable: true,
    selectable: true,
    deletable: true,
  } as Parameters<typeof MyNode>[0];
}

function renderInProvider(id?: string) {
  return render(
    <ReactFlowProvider>
      <MyNode {...makeProps(id)} />
    </ReactFlowProvider>
  );
}
```

> `makeProps`에는 `draggable`, `selectable`, `deletable`, `positionAbsoluteX`, `positionAbsoluteY` 를 포함해야 한다. React Flow의 `NodeProps` 타입이 이 필드들을 필수로 요구하며, 누락 시 `tsc --noEmit` 가 실패한다.

권장 테스트 케이스:

| 테스트 | 검증 내용 |
|--------|-----------|
| 핸들 렌더링 | target / source Handle 존재 여부 |
| 헤더 레이블 | 노드 이름 텍스트 렌더링 |
| 주요 UI 요소 | 노드 고유 UI 컴포넌트 |
| 인터랙션 | 버튼 클릭, invoke 호출 여부 등 |
| 언마운트 정리 | setInterval / 구독 해제 등 리소스 정리 |

---

## 6. 파일 구조

노드 추가 후 변경되는 파일 목록이다.

```
crates/tauri/
  Cargo.toml                         외부 크레이트 추가 (필요시)
  src/
    lib.rs                           AudioNode variant, AppData, 커맨드, invoke_handler
    runtime.rs                       Runtime struct, new(), match 구문
    nodes/
      mod.rs                         pub mod <node_name> 추가
      <node_name>.rs                 NodeTrait 구현체 + 유닛 테스트 (신규)

src/
  App.tsx                            onApply 직렬화 분기 추가
  ipc.d.ts                           커맨드 오버로드 추가 (필요시)
  types.ts                           nodeTypes, NodeType, AudioNode 등록
  state.ts                           addNodeAtContextMenu 타입 + 초기 data
  components/
    ContextMenu.tsx                  NODE_CATEGORIES 항목 추가
  nodes/
    <NodeName>.tsx                   React 컴포넌트 (신규)
  test/nodes/
    <NodeName>.test.tsx              컴포넌트 테스트 (신규)
```

---

## 7. 빌드 및 검증 명령

```powershell
# Rust 컴파일 + 유닛 테스트
cargo check --workspace
cargo test --workspace

# Release 빌드 확인 (CI 환경과 동일한 조건)
cargo check --release --workspace

# 프론트엔드 타입 체크 + 컴포넌트 테스트
npx tsc --noEmit
npx vitest run
```
