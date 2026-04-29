import { Node, NodeProps, Position } from "@xyflow/react";
import { useCallback } from "react";

import { AudioHandle } from "@/components/AudioHandle";
import { NODE_ACCENTS, NodeShell } from "@/components/NodeShell";
import { NodeDefinition } from "@/node-definition";
import { EdgeType, NONE, NodeTypeRecord, audioType } from "@/graph/edge-type";
import { useAppStore } from "@/state";

export type ChannelSplitNodeData = {
  outputCount: 2 | 4 | 6 | 8;
};

export type ChannelSplitNodeType = Node<ChannelSplitNodeData, "channelSplit">;

const COUNTS = [2, 4, 6, 8] as const;
const ROW_HEIGHT = 24;

export function ChannelSplit({ id, data }: NodeProps<ChannelSplitNodeType>) {
  const updateNode = useAppStore((s) => s.updateNode);

  const setCount = useCallback(
    (count: 2 | 4 | 6 | 8) => {
      updateNode(id, { outputCount: count });
    },
    [id, updateNode],
  );

  const count = data.outputCount ?? 2;
  const outputHandles = Array.from({ length: count }, (_, i) => ({
    id: `ch-${i}`,
    label: count === 2 ? (i === 0 ? "L / Ch 0" : "R / Ch 1") : `Ch ${i}`,
  }));

  return (
    <NodeShell accent={NODE_ACCENTS.channelSplit} title="Channel Split" invalid={(data as any)?.invalid}>
      {/* Segmented control for output count */}
      <div className="flex rounded overflow-hidden border border-gray-600 text-xs self-center">
        {COUNTS.map((c) => (
          <button
            key={c}
            className={[
              "px-2 py-0.5 font-mono transition-colors",
              c === count
                ? "bg-gray-500 text-white"
                : "bg-transparent text-gray-400 hover:bg-gray-700",
            ].join(" ")}
            onClick={() => setCount(c)}
          >
            {c}ch
          </button>
        ))}
      </div>

      {/* Output channel handles */}
      <div className="relative -mx-2" style={{ height: count * ROW_HEIGHT }}>
        {outputHandles.map((handle, i) => {
          const top = i * ROW_HEIGHT + ROW_HEIGHT / 2;
          return (
            <div key={handle.id}>
              <span
                className="absolute text-xs text-gray-300"
                style={{ top, right: 16, transform: "translateY(-50%)" }}
              >
                {handle.label}
              </span>
              <AudioHandle
                type="source"
                position={Position.Right}
                id={handle.id}
                style={{ top }}
              />
            </div>
          );
        })}
      </div>

      <AudioHandle type="target" position={Position.Left} id="ChannelSplit-target" />
    </NodeShell>
  );
}

const definition: NodeDefinition<ChannelSplitNodeType> = {
  component: ChannelSplit,
  toAudioNode: (node) => ({
    type: "channelSplit",
    data: { id: node.id },
  }),
  handles: {
    inputs: ["ChannelSplit-target"],
    outputs: ["ch-0", "ch-1", "ch-2", "ch-3", "ch-4", "ch-5", "ch-6", "ch-7"],
  },
  validate: (state, inputs) => {
    const count = (state as ChannelSplitNodeData).outputCount ?? 2;
    const incoming: EdgeType = inputs["ChannelSplit-target"] ?? NONE;
    const outputs: NodeTypeRecord = {};
    for (let i = 0; i < count; i++) {
      outputs[`ch-${i}`] =
        incoming.kind === "audio"
          ? audioType(1, incoming.frequency, incoming.bitsPerSample)
          : NONE;
    }
    return {
      expectedInputs: { "ChannelSplit-target": incoming },
      producedOutputs: outputs,
      ok: incoming.kind !== "audio" || incoming.channels >= count,
    };
  },
};

export default definition;
