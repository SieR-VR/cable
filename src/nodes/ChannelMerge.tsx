import { Node, NodeProps, Position } from "@xyflow/react";
import { useCallback } from "react";

import { AudioHandle } from "@/components/AudioHandle";
import { NODE_ACCENTS, NodeShell } from "@/components/NodeShell";
import { NodeDefinition } from "@/node-definition";
import { EdgeType, NONE, NodeTypeRecord, audioType, equalEdgeType } from "@/graph/edge-type";
import { useAppStore } from "@/state";

export type ChannelMergeNodeData = {
  inputCount: 2 | 4 | 6 | 8;
};

export type ChannelMergeNodeType = Node<ChannelMergeNodeData, "channelMerge">;

const COUNTS = [2, 4, 6, 8] as const;
const ROW_HEIGHT = 24;

export function ChannelMerge({ id, data }: NodeProps<ChannelMergeNodeType>) {
  const updateNode = useAppStore((s) => s.updateNode);

  const setCount = useCallback(
    (count: 2 | 4 | 6 | 8) => {
      updateNode(id, { inputCount: count });
    },
    [id, updateNode],
  );

  const count = data.inputCount ?? 2;
  const inputHandles = Array.from({ length: count }, (_, i) => ({
    id: `ch-${i}`,
    label: count === 2 ? (i === 0 ? "L" : "R") : `Ch ${i}`,
  }));

  return (
    <NodeShell
      accent={NODE_ACCENTS.channelMerge}
      title="Channel Merge"
      invalid={(data as any)?.invalid}
    >
      {/* Segmented control for input count */}
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

      {/* Input channel handles */}
      <div className="relative -mx-2" style={{ height: count * ROW_HEIGHT }}>
        {inputHandles.map((handle, i) => {
          const top = i * ROW_HEIGHT + ROW_HEIGHT / 2;
          return (
            <div key={handle.id}>
              <AudioHandle
                type="target"
                position={Position.Left}
                id={handle.id}
                style={{ top }}
              />
              <span
                className="absolute text-xs text-gray-300"
                style={{ top, left: 16, transform: "translateY(-50%)" }}
              >
                {handle.label}
              </span>
            </div>
          );
        })}
      </div>

      <AudioHandle type="source" position={Position.Right} id="ChannelMerge-source" />
    </NodeShell>
  );
}

const definition: NodeDefinition<ChannelMergeNodeType> = {
  component: ChannelMerge,
  toAudioNode: (node) => ({
    type: "channelMerge",
    data: { id: node.id, inputCount: node.data.inputCount ?? 2 },
  }),
  handles: {
    inputs: ["ch-0", "ch-1", "ch-2", "ch-3", "ch-4", "ch-5", "ch-6", "ch-7"],
    outputs: ["ChannelMerge-source"],
  },
  validate: (state, inputs) => {
    const count = (state as ChannelMergeNodeData).inputCount ?? 2;
    const inputIds = Array.from({ length: count }, (_, i) => `ch-${i}`);

    // Collect all connected non-NONE inputs.
    const connected = inputIds
      .map((id) => inputs[id])
      .filter((t): t is EdgeType => !!t && t.kind !== "none");

    // Determine output format from the first connected audio input.
    const first = connected.find((t) => t.kind === "audio");
    const allMatch =
      connected.length === 0 ||
      connected.every((t) => t.kind === "audio" && first && equalEdgeType(t, first));

    const produced: NodeTypeRecord = {};
    if (first && first.kind === "audio") {
      produced["ChannelMerge-source"] = audioType(count, first.frequency, first.bitsPerSample);
    } else {
      produced["ChannelMerge-source"] = NONE;
    }

    const expectedInputs: NodeTypeRecord = {};
    for (const id of inputIds) {
      // Expected format matches produced output channel type but mono.
      expectedInputs[id] =
        first && first.kind === "audio"
          ? audioType(1, first.frequency, first.bitsPerSample)
          : NONE;
    }

    return {
      expectedInputs,
      producedOutputs: produced,
      ok: allMatch,
    };
  },
};

export default definition;
