import { Node, NodeProps, Position } from "@xyflow/react";

import { AudioHandle } from "@/components/AudioHandle";
import { NODE_ACCENTS, NodeShell } from "@/components/NodeShell";
import { useAppStore } from "@/state";
import { NodeDefinition } from "@/node-definition";

export type EchoNodeData = {
  /** Delay time in milliseconds (0 – 2000). Default: 375 */
  delayMs: number;
  /** Feedback coefficient (0.0 – 0.95). Default: 0.4 */
  feedback: number;
  /** Wet mix (0.0 – 1.0). Default: 0.5 */
  wet: number;
  edgeType: string | null;
};

export type EchoNodeType = Node<EchoNodeData, "echo">;

export function Echo({ id, data }: NodeProps<EchoNodeType>) {
  const updateNode = useAppStore((s) => s.updateNode);

  const delayMs = data.delayMs ?? 375;
  const feedback = data.feedback ?? 0.4;
  const wet = data.wet ?? 0.5;

  return (
    <NodeShell accent={NODE_ACCENTS.echo} title="Echo">
      <div className="flex items-center gap-2">
        <label className="text-xs text-gray-300 w-14 shrink-0">Time</label>
        <input
          type="range"
          min={0}
          max={2000}
          step={1}
          value={delayMs}
          className="flex-1 accent-violet-400"
          onChange={(e) => updateNode(id, { delayMs: parseInt(e.target.value, 10) })}
        />
        <span className="text-xs text-gray-300 w-14 text-right tabular-nums">
          {delayMs} ms
        </span>
      </div>
      <div className="flex items-center gap-2">
        <label className="text-xs text-gray-300 w-14 shrink-0">Feedback</label>
        <input
          type="range"
          min={0}
          max={0.95}
          step={0.01}
          value={feedback}
          className="flex-1 accent-violet-400"
          onChange={(e) => updateNode(id, { feedback: parseFloat(e.target.value) })}
        />
        <span className="text-xs text-gray-300 w-8 text-right tabular-nums">
          {feedback.toFixed(2)}
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
          className="flex-1 accent-violet-400"
          onChange={(e) => updateNode(id, { wet: parseFloat(e.target.value) })}
        />
        <span className="text-xs text-gray-300 w-8 text-right tabular-nums">
          {wet.toFixed(2)}
        </span>
      </div>
      <AudioHandle type="target" position={Position.Left} id="Echo-target" />
      <AudioHandle type="source" position={Position.Right} id="Echo-source" />
    </NodeShell>
  );
}

const definition: NodeDefinition<EchoNodeType> = {
  component: Echo,
  toAudioNode: (node) => ({
    type: "echo",
    data: {
      id: node.id,
      delayMs: node.data.delayMs ?? 375,
      feedback: node.data.feedback ?? 0.4,
      wet: node.data.wet ?? 0.5,
    },
  }),
};

export default definition;
