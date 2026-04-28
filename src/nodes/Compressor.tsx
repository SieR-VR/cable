import { Node, NodeProps, Position } from "@xyflow/react";

import { AudioHandle } from "@/components/AudioHandle";
import { NODE_ACCENTS, NodeShell } from "@/components/NodeShell";
import { useAppStore } from "@/state";
import { NodeDefinition } from "@/node-definition";

export type CompressorNodeData = {
  /** Threshold in dB (−60 to 0). Default: −12 */
  thresholdDb: number;
  /** Compression ratio (1 – 20). Default: 4 */
  ratio: number;
  /** Attack time in ms (0.1 – 100). Default: 5 */
  attackMs: number;
  /** Release time in ms (10 – 1000). Default: 50 */
  releaseMs: number;
  /** Make-up gain in dB (0 – 24). Default: 0 */
  makeUpDb: number;
  edgeType: string | null;
};

export type CompressorNodeType = Node<CompressorNodeData, "compressor">;

function Row({
  label,
  value,
  displayValue,
  min,
  max,
  step,
  accentClass,
  onChange,
}: {
  label: string;
  value: number;
  displayValue: string;
  min: number;
  max: number;
  step: number;
  accentClass: string;
  onChange: (v: number) => void;
}) {
  return (
    <div className="flex items-center gap-2">
      <label className="text-xs text-gray-300 w-16 shrink-0">{label}</label>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        className={`flex-1 ${accentClass}`}
        onChange={(e) => onChange(parseFloat(e.target.value))}
      />
      <span className="text-xs text-gray-300 w-14 text-right tabular-nums">{displayValue}</span>
    </div>
  );
}

export function Compressor({ id, data }: NodeProps<CompressorNodeType>) {
  const updateNode = useAppStore((s) => s.updateNode);

  const threshold = data.thresholdDb ?? -12;
  const ratio = data.ratio ?? 4;
  const attack = data.attackMs ?? 5;
  const release = data.releaseMs ?? 50;
  const makeup = data.makeUpDb ?? 0;

  return (
    <NodeShell accent={NODE_ACCENTS.compressor} title="Compressor" minWidth="14rem">
      <Row
        label="Threshold"
        value={threshold}
        displayValue={`${threshold} dB`}
        min={-60}
        max={0}
        step={0.5}
        accentClass="accent-rose-400"
        onChange={(v) => updateNode(id, { thresholdDb: v })}
      />
      <Row
        label="Ratio"
        value={ratio}
        displayValue={`${ratio}:1`}
        min={1}
        max={20}
        step={0.1}
        accentClass="accent-rose-400"
        onChange={(v) => updateNode(id, { ratio: v })}
      />
      <Row
        label="Attack"
        value={attack}
        displayValue={`${attack} ms`}
        min={0.1}
        max={100}
        step={0.1}
        accentClass="accent-rose-300"
        onChange={(v) => updateNode(id, { attackMs: v })}
      />
      <Row
        label="Release"
        value={release}
        displayValue={`${release} ms`}
        min={10}
        max={1000}
        step={1}
        accentClass="accent-rose-300"
        onChange={(v) => updateNode(id, { releaseMs: v })}
      />
      <Row
        label="Make-up"
        value={makeup}
        displayValue={`+${makeup} dB`}
        min={0}
        max={24}
        step={0.5}
        accentClass="accent-rose-200"
        onChange={(v) => updateNode(id, { makeUpDb: v })}
      />
      <AudioHandle type="target" position={Position.Left} id="Compressor-target" />
      <AudioHandle type="source" position={Position.Right} id="Compressor-source" />
    </NodeShell>
  );
}

const definition: NodeDefinition<CompressorNodeType> = {
  component: Compressor,
  toAudioNode: (node) => ({
    type: "compressor",
    data: {
      id: node.id,
      thresholdDb: node.data.thresholdDb ?? -12,
      ratio: node.data.ratio ?? 4,
      attackMs: node.data.attackMs ?? 5,
      releaseMs: node.data.releaseMs ?? 50,
      makeUpDb: node.data.makeUpDb ?? 0,
    },
  }),
};

export default definition;
