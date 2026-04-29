import { Node, NodeProps, Position } from "@xyflow/react";

import { AudioHandle } from "@/components/AudioHandle";
import { NODE_ACCENTS, NodeShell } from "@/components/NodeShell";
import { useAppStore } from "@/state";
import { NodeDefinition } from "@/node-definition";
import { passthroughValidator } from "@/graph/edge-type";

export type DelayNodeData = {
  /** Delay time in milliseconds (0 – 2000). Default: 250 */
  delayMs: number;
  edgeType: string | null;
};

export type DelayNodeType = Node<DelayNodeData, "delay">;

export function Delay({ id, data }: NodeProps<DelayNodeType>) {
  const updateNode = useAppStore((s) => s.updateNode);

  return (
    <NodeShell accent={NODE_ACCENTS.delay} title="Delay">
      <div className="flex items-center gap-2">
        <label className="text-xs text-gray-300 w-10 shrink-0">Time</label>
        <input
          type="range"
          min={0}
          max={2000}
          step={1}
          value={data.delayMs ?? 250}
          className="flex-1 accent-sky-400"
          onChange={(e) => updateNode(id, { delayMs: parseInt(e.target.value, 10) })}
        />
        <span className="text-xs text-gray-300 w-14 text-right tabular-nums">
          {data.delayMs ?? 250} ms
        </span>
      </div>
      <AudioHandle type="target" position={Position.Left} id="Delay-target" />
      <AudioHandle type="source" position={Position.Right} id="Delay-source" />
    </NodeShell>
  );
}

const definition: NodeDefinition<DelayNodeType> = {
  component: Delay,
  toAudioNode: (node) => ({
    type: "delay",
    data: { id: node.id, delayMs: node.data.delayMs ?? 250 },
  }),
  handles: { inputs: ["Delay-target"], outputs: ["Delay-source"] },
  validate: passthroughValidator("Delay-target", "Delay-source"),
};

export default definition;
