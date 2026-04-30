# 새 노드 추가 가이드

Cable에 새 오디오 노드를 추가하기 위해 수정해야 하는 모든 위치와 핵심 패턴을 정리한다. 기존 노드 동작은 [`docs/nodes.md`](nodes.md), 전체 아키텍처는 [`docs/architecture.md`](architecture.md)를 먼저 읽으면 도움이 된다.

---

## 1. 설계 결정

| 질문 | 선택지 |
|------|--------|
| 노드의 역할 | **Source** — 출력 엣지만 가짐 (예: 입력 장치) |
| | **Sink** — 입력 엣지만 가짐 (예: 출력 장치) |
| | **Passthrough** — 입력과 출력을 모두 가짐 (예: 이펙터, 시각화) |
| 다중 입출력? | 채널 분리/병합처럼 핸들이 여러 개라면 `fromHandle` / `toHandle`로 구분 |
| 무거운 연산 위치 | **Rust 백엔드** — 샘플 단위 실시간 연산은 반드시 백엔드 |
| | **프론트엔드** — 시각화/UI 전용 처리 |
| 초기 설정값 | 프론트엔드 `data`에 들어가고 Rust 노드 구조체로 직렬화되는 필드 |
| 인스턴스 단위 IPC 명령 필요? | `NodeTrait::command()`를 구현해 `node_command` IPC를 디스패치 (예: VST 파라미터) |
| 시각화 데이터를 UI에 보낼 필요? | `Runtime` 공유 버퍼 + `get_node_render_data` 패턴 사용 (Spectrum/Waveform 참고) |

---

## 2. 구현 체크리스트

| 위치 | 변경 내용 |
|------|-----------|
| `crates/tauri/Cargo.toml` | 외부 크레이트 추가 (필요한 경우만) |
| `crates/tauri/src/nodes/mod.rs` | `pub mod <node_name>;` 등록 |
| `crates/tauri/src/nodes/<node_name>.rs` | `NodeTrait` 구현체 (신규 파일) |
| `crates/tauri/src/runtime.rs` | `AudioNode` 열거형에 variant 추가 + `id()` / `init()` / `dispose()` / `process()` / `command()` 각 match 분기 추가 |
| `crates/tauri/src/runtime.rs` | 노드 전용 공유 상태가 있다면 `Runtime` 구조체에 필드 추가 (`spectrum_buffers` 등 참고) |
| `src/nodes/<NodeName>.tsx` | React 컴포넌트 + `toAudioNode` + `NodeDefinition` default export (신규 파일) |
| `src/types.ts` | `nodeDefs` 객체에 import + 등록 (`NodeType` / `AudioNode` 유니온은 자동으로 도출됨) |
| `src/components/ContextMenu.tsx` | `NODE_CATEGORIES`에 항목 추가 (필요 시 `requiresDriver: true`) |
| `src/state.ts` | `addNodeAtContextMenu`의 타입 유니온과 초기 `data` 분기 추가 |
| `src/ipc.d.ts` | 새 IPC 커맨드를 추가했다면 오버로드 추가 |

> **주의**: `nodeDefs`에 등록을 빠뜨리면 `serializeNode`가 해당 노드를 직렬화하지 못하고 런타임 오류가 발생한다.

---

## 3. Rust 구현

### 3-1. 외부 크레이트 추가 (선택)

```toml
# crates/tauri/Cargo.toml
[dependencies]
some-crate = "1.0"
```

### 3-2. NodeTrait 구현체

`crates/tauri/src/nodes/<node_name>.rs` 신규 작성.

**핵심 구조체**

```rust
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
  nodes::{AudioBuffer, NodeTrait},
  runtime::{Runtime, RuntimeState},
};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MyNode {
  /// React Flow 노드 ID와 일치해야 한다.
  id: String,

  /// 프론트엔드에서 직렬화되어 전달되는 설정값 (필드명은 camelCase로 변환됨).
  some_setting: f32,

  /// 런타임에서만 사용되는 상태는 직렬화에서 제외한다.
  #[serde(skip)]
  runtime_state: Option<SomeType>,
}
```

`#[serde(skip)]` 필드는 IPC를 통해 전달되지 않으며 `init()` 호출 시 값을 채운다.

**NodeTrait 계약**

```rust
impl NodeTrait for MyNode {
  fn id(&self) -> &str {
    &self.id
  }

  fn init(&mut self, runtime: &Runtime) -> Result<(), String> {
    // 리소스 획득 / 공유 버퍼 Arc 연결 등
    // runtime.sample_rate, runtime.buffer_size 사용 가능
    Ok(())
  }

  fn dispose(&mut self, _runtime: &Runtime) -> Result<(), String> {
    // 리소스 해제
    self.runtime_state = None;
    Ok(())
  }

  fn process(
    &mut self,
    runtime: &Runtime,
    state: &RuntimeState,
  ) -> Result<BTreeMap<String, AudioBuffer>, String> {
    // 입력 엣지에서 AudioBuffer 읽기 (Sink / Passthrough)
    let incoming: Option<&AudioBuffer> = runtime
      .edges
      .iter()
      .find(|e| e.to == self.id)
      .and_then(|e| state.edge_values.get(&e.id));

    // 다중 입력 노드는 to_handle로 매칭
    // let buf_a = runtime.edges.iter()
    //   .find(|e| e.to == self.id && e.to_handle.as_deref() == Some("input-a"))
    //   .and_then(|e| state.edge_values.get(&e.id));

    // 출력 엣지마다 AudioBuffer 쓰기 (Source / Passthrough)
    let mut output = BTreeMap::new();
    for edge in &runtime.edges {
      if edge.from == self.id {
        // 다중 출력은 edge.from_handle로 분기
        output.insert(edge.id.clone(), AudioBuffer { /* ... */ });
      }
    }

    Ok(output)
  }
}
```

### 3-3. 시각화 / 공유 버퍼가 필요한 경우

오디오 스레드 외부(예: IPC `get_node_render_data`)에서 노드 내부 데이터를 읽어야 할 때는 `Arc<Mutex<T>>` 패턴을 사용한다. 기존 `spectrum_buffers` / `waveform_buffers`와 동일한 형태로 `Runtime`에 새 맵을 추가하고, `Runtime::add_node()`의 사전-할당 분기, `replace_graph` / `remove_node` / `set_audio_config`의 정리 분기, 그리고 노드의 `init()`에서 Arc 클론을 받아오면 된다.

읽기 측 (`get_node_render_data` 같은 IPC 커맨드)에서는 락을 짧게 잡고 `clone()` 한 뒤 즉시 풀어 오디오 스레드를 차단하지 않는다.

### 3-4. AudioNode enum 등록

`crates/tauri/src/runtime.rs`의 `AudioNode` 열거형과 `id`/`command`/`init`/`dispose`/`process` 각 match 구문에 새 variant를 추가한다.

```rust
pub(crate) enum AudioNode {
  // ...
  MyNode(MyNode),
}

impl AudioNode {
  pub fn id(&self) -> &str {
    match self {
      // ...
      AudioNode::MyNode(n) => n.id(),
    }
  }
  // command(), init(), dispose(), process() 모두 동일 패턴
}
```

### 3-5. 인스턴스 IPC 명령 (선택)

`NodeTrait::command()`를 오버라이드하면 `node_command(nodeId, data)` IPC가 자동으로 디스패치된다. 별도의 Tauri 커맨드를 새로 만들 필요는 없다.

```rust
fn command(&mut self, data: serde_json::Value) -> Result<serde_json::Value, String> {
  let op = data.get("op").and_then(|v| v.as_str()).ok_or("missing op")?;
  match op {
    "doSomething" => { /* ... */ Ok(serde_json::Value::Null) }
    other => Err(format!("unknown op: {other}")),
  }
}
```

플러그인 단위(노드 인스턴스가 아직 없을 때 호출하는) 명령은 `lib.rs::plugin_command`의 디스패치 분기에 새 plugin type을 추가한다 (`vst` 사례 참고).

---

## 4. 프론트엔드 구현

### 4-1. React 컴포넌트 + NodeDefinition

`src/nodes/<NodeName>.tsx` 신규 작성. 한 파일에 다음 세 가지를 모두 포함한다.

1. **named export** 컴포넌트 함수 (`export function MyNode`)
2. **로컬** `toAudioNode` 함수 (export 없음)
3. **default export** `NodeDefinition` 객체 (`{ component, toAudioNode, handles?, validate? }`)

```tsx
import { Handle, Node, NodeProps, Position } from "@xyflow/react";
import { NodeDefinition } from "@/node-definition";

export type MyNodeData = {
  someSetting: number;
  edgeType: unknown;       // 검증 엔진이 사용하는 추론된 엣지 타입 슬롯
};

export type MyNodeType = Node<MyNodeData, "myNode">;

export function MyNode({ id, data }: NodeProps<MyNodeType>) {
  return (
    <div className="bg-gray-700 rounded-lg flex flex-col text-white min-w-48">
      {/* 드래그 핸들 클래스가 있어야 노드가 잡혀서 이동 가능 */}
      <div className="w-full h-6 bg-blue-500 rounded-t-lg flex items-center text-sm font-bold p-2 drag-handle__custom">
        My Node
      </div>
      <div className="flex flex-col gap-2 p-2 relative">
        {/* 노드 본문 */}
      </div>
      <Handle type="target" position={Position.Left} id="myNode-target" />
      <Handle type="source" position={Position.Right} id="myNode-source" />
    </div>
  );
}

// Rust AudioNode variant의 data 구조와 일치해야 한다
function toAudioNode(node: MyNodeType) {
  return {
    type: "myNode" as const,
    data: { id: node.id, someSetting: node.data.someSetting ?? 0 },
  };
}

const definition: NodeDefinition<MyNodeType> = {
  component: MyNode,
  toAudioNode,
  // 필요 시 핸들 메타데이터 / validate 추가
};
export default definition;
```

### 4-2. types.ts 등록

```ts
import myNodeDef, { MyNodeType } from "./nodes/MyNode";

export const nodeDefs = {
  // ...기존 항목
  myNode: myNodeDef,
};
```

`nodeTypes` (React Flow에 넘기는 컴포넌트 맵)와 `serializeNode`, 그리고 `NodeType` / `AudioNode` 유니온 타입은 모두 `nodeDefs`로부터 자동 도출되므로 별도 수정이 필요 없다.

### 4-3. ContextMenu 등록

```ts
// src/components/ContextMenu.tsx
const NODE_CATEGORIES: NodeCategory[] = [
  // ...기존 카테고리
  {
    label: "My Category",
    // requiresDriver: true,   // CableAudio.sys 핸들이 있어야만 추가 가능한 경우
    items: [{ type: "myNode", label: "My Node" }],
  },
];
```

### 4-4. state.ts (`addNodeAtContextMenu`)

`type` 파라미터의 유니온에 `"myNode"`를 추가하고, 새 노드의 초기 `data`를 결정하는 분기를 추가한다.

### 4-5. ipc.d.ts (선택)

새 IPC 커맨드(`replace_graph`/`add_node` 외에 별도)를 추가했다면 `src/ipc.d.ts`에 오버로드를 추가한다. `node_command` / `plugin_command`의 ad-hoc op만 사용한다면 변경 불필요.

---

## 5. 검증 / 테스트

### 5-1. Rust 유닛 테스트

`nodes/<node_name>.rs` 하단의 `#[cfg(test)]` 모듈에 작성한다. 가능하면 `Runtime` 없이 핵심 DSP 로직 단위로 테스트한다(`echo.rs`, `delay.rs`, `compressor.rs` 참고).

```rust
#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn applies_setting_correctly() {
    // ... DSP 함수만 단독으로 호출해 검증
  }
}
```

### 5-2. 프론트엔드 컴포넌트 테스트

`src/test/nodes/<NodeName>.test.tsx`. `Handle` 컴포넌트는 React Flow 내부 store에 접근하므로 반드시 `<ReactFlowProvider>`로 감싸 렌더링한다.

`NodeProps` 타입은 `draggable`, `selectable`, `deletable`, `positionAbsoluteX`, `positionAbsoluteY` 등을 필수로 요구한다. 누락 시 `tsc --noEmit`이 실패한다.

검증 권장 항목:
- target / source `Handle`이 모두 렌더되는지
- 헤더 라벨과 주요 UI 요소
- 사용자 인터랙션(버튼 클릭, invoke 호출 여부)
- 언마운트 시 폴링/타이머 등 리소스 정리

---

## 6. 그래프 적용 흐름 요약

새로 추가한 노드는 다음 흐름을 그대로 따라간다.

```
사용자가 ContextMenu에서 "My Node" 추가
  → state.addNodeAtContextMenu(...)         // 노드 객체 생성
  → React Flow nodes 갱신
  → applyCascade(...) / applyFullValidation()
       └─ 모든 노드가 valid → invoke("replace_graph", { nodes, edges })
            └─ runtime::replace_graph
                 ├─ 기존 노드 dispose()
                 └─ 새 노드들에 add_node() — 여기서 MyNode::init() 호출
사용자가 "Enable Runtime" → enable_runtime
  → 매 틱 Runtime::process() — MyNode::process() 호출
사용자가 "Disable Runtime" → disable_runtime
```

증분 변경(`add_node` / `update_node` / `add_edge` / `remove_edge`) IPC도 정의되어 있지만, 현재 프론트엔드는 검증 결과에 기반해 항상 `replace_graph`로 통째로 푸시한다.

---

## 7. 빌드 및 검증 명령

```powershell
# Rust 컴파일 + 유닛 테스트
cargo check --workspace
cargo test --workspace

# Release 빌드 확인
cargo check --release --workspace

# 프론트엔드 타입 체크 + 컴포넌트 테스트
pnpm exec tsc --noEmit
pnpm exec vitest run

# Tauri 통합 동작
pnpm tauri dev
```
