import { Node, NodeProps, Position } from "@xyflow/react";

import { AudioHandle } from "@/components/AudioHandle";
import { NODE_ACCENTS, NodeShell } from "@/components/NodeShell";
import { useAppStore } from "@/state";
import { NodeDefinition } from "@/node-definition";
import { passthroughValidator } from "@/graph/edge-type";

export type ReverbNodeData = {
  /** Room size (0.0 – 1.0). Controls comb-filter feedback. Default: 0.5 */
  roomSize: number;
  /** Wet mix level (0.0 – 1.0). Default: 0.33 */
  wet: number;
  /** Damping factor for high-frequency absorption (0.0 – 1.0). Default: 0.5 */
  damp: number;
  edgeType: string | null;
};

export type ReverbNodeType = Node<ReverbNodeData, "reverb">;

export function Reverb({ id, data }: NodeProps<ReverbNodeType>) {
  const updateNode = useAppStore((s) => s.updateNode);

  const roomSize = data.roomSize ?? 0.5;
  const wet = data.wet ?? 0.33;
  const damp = data.damp ?? 0.5;

  return (
    <NodeShell accent={NODE_ACCENTS.reverb} title="Reverb">
      <div className="flex items-center gap-2">
        <label className="text-xs text-gray-300 w-14 shrink-0">Room</label>
        <input
          type="range"
          min={0}
          max={1}
          step={0.01}
          value={roomSize}
          className="flex-1 accent-cyan-400"
          onChange={(e) => updateNode(id, { roomSize: parseFloat(e.target.value) })}
        />
        <span className="text-xs text-gray-300 w-8 text-right tabular-nums">
          {roomSize.toFixed(2)}
        </span>
      </div>
      <div className="flex items-center gap-2">
        <label className="text-xs text-gray-300 w-14 shrink-0">Wet</label>
        <input
          type="range"
          min={0}
          max={1}
          step={0.01}
          value={wet}
          className="flex-1 accent-cyan-400"
          onChange={(e) => updateNode(id, { wet: parseFloat(e.target.value) })}
        />
        <span className="text-xs text-gray-300 w-8 text-right tabular-nums">
          {wet.toFixed(2)}
        </span>
      </div>
      <div className="flex items-center gap-2">
        <label className="text-xs text-gray-300 w-14 shrink-0">Damp</label>
        <input
          type="range"
          min={0}
          max={1}
          step={0.01}
          value={damp}
          className="flex-1 accent-cyan-400"
          onChange={(e) => updateNode(id, { damp: parseFloat(e.target.value) })}
        />
        <span className="text-xs text-gray-300 w-8 text-right tabular-nums">
          {damp.toFixed(2)}
        </span>
      </div>
      <AudioHandle type="target" position={Position.Left} id="Reverb-target" />
      <AudioHandle type="source" position={Position.Right} id="Reverb-source" />
    </NodeShell>
  );
}

const definition: NodeDefinition<ReverbNodeType> = {
  component: Reverb,
  toAudioNode: (node) => ({
    type: "reverb",
    data: {
      id: node.id,
      roomSize: node.data.roomSize ?? 0.5,
      wet: node.data.wet ?? 0.33,
      damp: node.data.damp ?? 0.5,
    },
  }),
  handles: { inputs: ["Reverb-target"], outputs: ["Reverb-source"] },
  validate: passthroughValidator("Reverb-target", "Reverb-source"),
};

export default definition;
