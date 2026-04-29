import { Node, NodeProps, Position } from "@xyflow/react";

import { AudioHandle } from "@/components/AudioHandle";
import { NODE_ACCENTS, NodeShell } from "@/components/NodeShell";
import { useAppStore } from "@/state";
import { NodeDefinition } from "@/node-definition";
import { passthroughValidator } from "@/graph/edge-type";

export type GainNodeData = {
  /** Linear gain multiplier (0.0 – 4.0). Default: 1.0 */
  gain: number;
  edgeType: string | null;
};

export type GainNodeType = Node<GainNodeData, "gain">;

export function Gain({ id, data }: NodeProps<GainNodeType>) {
  const updateNode = useAppStore((s) => s.updateNode);

  return (
    <NodeShell accent={NODE_ACCENTS.gain} title="Gain">
      <div className="flex items-center gap-2">
        <label className="text-xs text-gray-300 w-10 shrink-0">Gain</label>
        <input
          type="range"
          min={0}
          max={4}
          step={0.01}
          value={data.gain ?? 1.0}
          className="flex-1 accent-amber-400"
          onChange={(e) => updateNode(id, { gain: parseFloat(e.target.value) })}
        />
        <span className="text-xs text-gray-300 w-8 text-right tabular-nums">
          {(data.gain ?? 1.0).toFixed(2)}
        </span>
      </div>
      <AudioHandle type="target" position={Position.Left} id="Gain-target" />
      <AudioHandle type="source" position={Position.Right} id="Gain-source" />
    </NodeShell>
  );
}

const definition: NodeDefinition<GainNodeType> = {
  component: Gain,
  toAudioNode: (node) => ({
    type: "gain",
    data: { id: node.id, gain: node.data.gain ?? 1.0 },
  }),
  handles: { inputs: ["Gain-target"], outputs: ["Gain-source"] },
  validate: passthroughValidator("Gain-target", "Gain-source"),
};

export default definition;
