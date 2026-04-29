/**
 * Validation engine: walks a node graph, runs each node's validate() and
 * propagates resulting EdgeType / invalid flags through the graph until a
 * fixpoint.
 *
 * Pure functions over `(nodes, edges, validation)` snapshots so they can be
 * called from a Zustand reducer without owning store internals.
 */

import { Edge, Node } from "@xyflow/react";

import { handlesFor, runValidator } from ".";
import {
  EdgeType,
  NONE,
  NodeTypeRecord,
  ValidationResult,
  equalTypeRecord,
  isCompatible,
} from "./edge-type";
import { NodeDefinition } from "../node-definition";
import { AudioEdge } from "../types";

type AnyNode = Node<any, any>;
type AnyEdge = Edge<AudioEdge>;

export interface ValidationContext {
  nodes: AnyNode[];
  edges: AnyEdge[];
  validation: Record<string, ValidationResult>;
  getDef: (type: string | undefined) => NodeDefinition<AnyNode> | undefined;
}

export interface ValidationOutcome {
  nodes: AnyNode[];
  edges: AnyEdge[];
  validation: Record<string, ValidationResult>;
}

const VISITS_PER_NODE = 4;

function incomingTypes(node: AnyNode, edges: AnyEdge[]): NodeTypeRecord {
  const r: NodeTypeRecord = {};
  for (const e of edges) {
    if (e.target !== node.id || !e.targetHandle) continue;
    r[e.targetHandle] = e.data?.edgeType ?? NONE;
  }
  // Make sure declared input handles exist in the record even if no edge is
  // connected — keeps validator output stable across connect/disconnect.
  return r;
}

function withFallbackHandles(
  inputs: NodeTypeRecord,
  declared: string[],
): NodeTypeRecord {
  const out = { ...inputs };
  for (const h of declared) if (!(h in out)) out[h] = NONE;
  return out;
}

function patchEdge(e: AnyEdge, patch: Partial<AudioEdge>): AnyEdge {
  const base: AudioEdge = e.data ?? {
    id: e.id,
    from: e.source,
    to: e.target,
    fromHandle: e.sourceHandle ?? undefined,
    toHandle: e.targetHandle ?? undefined,
  };
  return { ...e, data: { ...base, ...patch } };
}

interface SingleStep {
  outcome: ValidationOutcome;
  /** Sinks whose `producedOutputs` was just changed and must re-validate. */
  cascadeFrom: string[];
}

function validateOne(ctx: ValidationContext, nodeId: string): SingleStep {
  const { nodes, edges, validation, getDef } = ctx;
  const node = nodes.find((n) => n.id === nodeId);
  if (!node) {
    return { outcome: { nodes, edges, validation }, cascadeFrom: [] };
  }

  const def = getDef(node.type);
  const declared = handlesFor(def);
  const incoming = withFallbackHandles(incomingTypes(node, edges), declared.inputs);
  const result = runValidator(def, node.data, incoming);

  const prev = validation[nodeId];
  const producedChanged =
    !prev || !equalTypeRecord(prev.producedOutputs, result.producedOutputs);
  const newValidation = { ...validation, [nodeId]: result };

  const newNodes = nodes.map((n) => {
    if (n.id !== nodeId) return n;
    if (result.ok) {
      // Strip stale invalid flag if the node is now valid; keep the rest.
      const { invalid: _drop, ...restData } = (n.data ?? {}) as Record<string, unknown>;
      return { ...n, data: restData } as AnyNode;
    }
    return { ...n, data: { ...n.data, invalid: true } } as AnyNode;
  });

  const newEdges = edges.map((e) => {
    if (e.source === nodeId && e.sourceHandle) {
      const newType: EdgeType = result.producedOutputs[e.sourceHandle] ?? NONE;
      // Re-evaluate sink invalid flag against the sink's previously-known
      // expected type. The sink itself runs again only if produced changed
      // (handled by cascadeFrom).
      const sinkRes = newValidation[e.target] ?? validation[e.target];
      const sinkExpected: EdgeType =
        e.targetHandle && sinkRes
          ? sinkRes.expectedInputs[e.targetHandle] ?? NONE
          : NONE;
      return patchEdge(e, {
        edgeType: newType,
        invalid: !isCompatible(newType, sinkExpected),
      });
    }
    if (e.target === nodeId && e.targetHandle) {
      const expected: EdgeType = result.expectedInputs[e.targetHandle] ?? NONE;
      const actual: EdgeType = e.data?.edgeType ?? NONE;
      return patchEdge(e, { invalid: !isCompatible(actual, expected) });
    }
    return e;
  });

  const cascadeFrom = producedChanged
    ? newEdges
        .filter((e) => e.source === nodeId)
        .map((e) => e.target)
    : [];

  return {
    outcome: { nodes: newNodes, edges: newEdges, validation: newValidation },
    cascadeFrom,
  };
}

/**
 * Re-validate the seed nodes and cascade downstream until the graph reaches
 * a fixpoint (no more produced-output changes). Cycle-safe via per-node
 * visit caps.
 */
export function runCascade(
  ctx: ValidationContext,
  seedIds: string[],
): ValidationOutcome {
  let cur: ValidationOutcome = {
    nodes: ctx.nodes,
    edges: ctx.edges,
    validation: ctx.validation,
  };
  const queue: string[] = [];
  for (const s of seedIds) if (!queue.includes(s)) queue.push(s);

  const visits: Record<string, number> = {};
  let totalVisits = 0;
  const totalLimit = Math.max(ctx.nodes.length, 1) * VISITS_PER_NODE;

  while (queue.length) {
    const id = queue.shift()!;
    visits[id] = (visits[id] ?? 0) + 1;
    totalVisits += 1;
    if (visits[id] > VISITS_PER_NODE) {
      console.warn(`[validation] node ${id} hit per-node cascade limit`);
      continue;
    }
    if (totalVisits > totalLimit) {
      console.warn("[validation] cascade global visit limit reached");
      break;
    }

    const step = validateOne({ ...ctx, ...cur }, id);
    cur = step.outcome;
    for (const next of step.cascadeFrom) {
      if (!queue.includes(next)) queue.push(next);
    }
  }

  return cur;
}

/**
 * Re-validate every node, starting from sources (nodes with no incoming
 * edges) so that produced-output cascades flow naturally downstream.
 * Falls back to "all nodes" as seeds when the graph is fully cyclic.
 */
export function runFullValidation(ctx: ValidationContext): ValidationOutcome {
  const incomingCounts: Record<string, number> = {};
  for (const n of ctx.nodes) incomingCounts[n.id] = 0;
  for (const e of ctx.edges) {
    if (incomingCounts[e.target] !== undefined) incomingCounts[e.target] += 1;
  }
  const sources = ctx.nodes.filter((n) => incomingCounts[n.id] === 0).map((n) => n.id);
  const seeds = sources.length > 0 ? sources : ctx.nodes.map((n) => n.id);
  return runCascade(ctx, seeds);
}
