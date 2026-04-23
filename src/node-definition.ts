import { ComponentType } from "react";
import { Node, NodeProps } from "@xyflow/react";

/**
 * 각 노드 파일의 default export 타입.
 * component: React Flow 렌더링에 사용되는 컴포넌트.
 * toAudioNode: 해당 노드를 IPC용 AudioNode 형태로 직렬화하는 함수.
 */
export interface NodeDefinition<TNode extends Node<any, any>> {
  component: ComponentType<NodeProps<TNode>>;
  toAudioNode: (node: TNode) => { type: TNode["type"]; data: object };
}
