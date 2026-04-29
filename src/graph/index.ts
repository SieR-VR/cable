import {
  EdgeType,
  NodeTypeRecord,
  ValidationResult,
  defaultPassthroughValidator,
} from "./edge-type";
import { NodeDefinition } from "../node-definition";
import { Node } from "@xyflow/react";

/**
 * Look up handle IDs for a node from its NodeDefinition. If the definition
 * doesn't declare them, returns empty arrays so the fallback validator
 * produces no outputs (matches the behavior of source-only / sink-only nodes
 * with no real handles).
 */
export function handlesFor(
  def: NodeDefinition<Node<any, any>> | undefined,
): { inputs: string[]; outputs: string[] } {
  return def?.handles ?? { inputs: [], outputs: [] };
}

/**
 * Run validation for a node, falling back to the default passthrough validator
 * if the definition doesn't provide one yet. Phase 1 helper — Phase 2 fills
 * in real validators.
 */
export function runValidator(
  def: NodeDefinition<Node<any, any>> | undefined,
  state: unknown,
  inputs: NodeTypeRecord,
): ValidationResult {
  if (def?.validate) {
    return def.validate(state as never, inputs);
  }
  const { outputs } = handlesFor(def);
  return defaultPassthroughValidator(inputs, outputs);
}

/**
 * Re-export a couple of helpers so consumers can import everything from
 * `@/graph` without reaching into specific files.
 */
export type { EdgeType, NodeTypeRecord, ValidationResult };
