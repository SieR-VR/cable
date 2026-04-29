import { Node, NodeProps, Position } from "@xyflow/react";

import { AudioHandle } from "@/components/AudioHandle";
import { NODE_ACCENTS, NodeShell } from "@/components/NodeShell";
import { NodeDefinition } from "@/node-definition";
import { EdgeType, NONE, equalEdgeType } from "@/graph/edge-type";

export type MixerNodeData = {
  edgeType: string | null;
};

export type MixerNodeType = Node<MixerNodeData, "mixer">;

const INPUTS = [
  { id: "input-a", label: "A" },
  { id: "input-b", label: "B" },
] as const;
const ROW_HEIGHT = 24;

export function Mixer({ id }: NodeProps<MixerNodeType>) {
  void id;
  return (
    <NodeShell accent={NODE_ACCENTS.mixer} title="Mixer">
      {/*
        Negative horizontal margin cancels NodeShell's `p-2` body padding so
        the relative container spans the full card width. Handle dots then sit
        flush with the card's left/right edges (matching every other handle
        in the app).
      */}
      <div
        className="relative -mx-2"
        style={{ height: INPUTS.length * ROW_HEIGHT }}
      >
        {INPUTS.map((input, i) => {
          const top = i * ROW_HEIGHT + ROW_HEIGHT / 2;
          return (
            <div key={input.id}>
              <AudioHandle
                type="target"
                position={Position.Left}
                id={input.id}
                style={{ top }}
              />
              <span
                className="absolute text-xs text-gray-300"
                style={{ top, left: 16, transform: "translateY(-50%)" }}
              >
                {input.label}
              </span>
            </div>
          );
        })}
      </div>
      <AudioHandle type="source" position={Position.Right} id="Mixer-source" />
    </NodeShell>
  );
}

const definition: NodeDefinition<MixerNodeType> = {
  component: Mixer,
  toAudioNode: (node) => ({
    type: "mixer",
    data: { id: node.id },
  }),
  handles: { inputs: ["input-a", "input-b"], outputs: ["Mixer-source"] },
  validate: (_state, inputs) => {
    const a: EdgeType = inputs["input-a"] ?? NONE;
    const b: EdgeType = inputs["input-b"] ?? NONE;
    // Pick the first non-none input as the produced type. If both inputs are
    // concrete and disagree, the mix is invalid but we still propagate `a`
    // downstream so the rest of the graph can keep validating.
    const produced: EdgeType =
      a.kind !== "none" ? a : b.kind !== "none" ? b : NONE;
    const ok =
      a.kind === "none" || b.kind === "none" || equalEdgeType(a, b);
    return {
      expectedInputs: { "input-a": produced, "input-b": produced },
      producedOutputs: { "Mixer-source": produced },
      ok,
    };
  },
};

export default definition;
