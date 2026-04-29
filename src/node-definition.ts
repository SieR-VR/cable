import { ComponentType } from "react";
import { Node, NodeProps } from "@xyflow/react";

import { NodeTypeRecord, ValidationResult } from "./graph/edge-type";

/**
 * Default export type for each node file.
 * component: The component used for React Flow rendering.
 * toAudioNode: Function that serializes the node into an AudioNode shape for IPC.
 * validate:   (Optional in Phase 1) Type validation function. If omitted, the
 *             graph engine will use a passthrough fallback. Will become
 *             required once Phase 2 lands real validators for every node.
 */
export interface NodeDefinition<TNode extends Node<any, any>> {
  component: ComponentType<NodeProps<TNode>>;
  toAudioNode: (node: TNode) => { type: TNode["type"]; data: object };
  /**
   * Static metadata describing the handles this node owns. Used by the graph
   * engine to determine which input/output keys exist when running validation
   * against a freshly-created node before any edges have been connected.
   */
  handles?: {
    inputs: string[];
    outputs: string[];
  };
  validate?: (
    state: TNode["data"],
    inputs: NodeTypeRecord,
  ) => ValidationResult;
}

