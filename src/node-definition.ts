import { ComponentType } from "react";
import { Node, NodeProps } from "@xyflow/react";

/**
 * Default export type for each node file.
 * component: The component used for React Flow rendering.
 * toAudioNode: Function that serializes the node into an AudioNode shape for IPC.
 */
export interface NodeDefinition<TNode extends Node<any, any>> {
  component: ComponentType<NodeProps<TNode>>;
  toAudioNode: (node: TNode) => { type: TNode["type"]; data: object };
}
