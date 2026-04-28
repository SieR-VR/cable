import { useEffect, useRef } from "react";
import { Node, NodeProps, Position } from "@xyflow/react";

import { AudioHandle } from "@/components/AudioHandle";
import { NODE_ACCENTS, NodeShell } from "@/components/NodeShell";
import { useAppStore } from "../state";
import { NodeDefinition } from "@/node-definition";

export type WaveformMonitorNodeData = {
  /** Number of samples in the rolling display window. Default: 2048 */
  windowSize: number;
  edgeType: string | null;
};

export type WaveformMonitorNode = Node<WaveformMonitorNodeData, "waveformMonitor">;

const CANVAS_WIDTH = 240;
const CANVAS_HEIGHT = 80;
const WAVE_COLOR = "#34d399"; // emerald-400
const ZERO_COLOR = "#374151"; // gray-700
const BACKGROUND_COLOR = "#111827"; // gray-900

function drawWaveform(canvas: HTMLCanvasElement | null, samples: number[]): void {
  if (!canvas || samples.length === 0) return;
  const ctx = canvas.getContext("2d");
  if (!ctx) return;

  ctx.fillStyle = BACKGROUND_COLOR;
  ctx.fillRect(0, 0, CANVAS_WIDTH, CANVAS_HEIGHT);

  // Zero line
  const midY = CANVAS_HEIGHT / 2;
  ctx.strokeStyle = ZERO_COLOR;
  ctx.lineWidth = 1;
  ctx.beginPath();
  ctx.moveTo(0, midY);
  ctx.lineTo(CANVAS_WIDTH, midY);
  ctx.stroke();

  // Waveform
  ctx.strokeStyle = WAVE_COLOR;
  ctx.lineWidth = 1.5;
  ctx.beginPath();

  const step = CANVAS_WIDTH / samples.length;
  for (let i = 0; i < samples.length; i++) {
    // Clamp amplitude to [-1, 1] and map to canvas Y
    const amplitude = Math.max(-1, Math.min(1, samples[i]));
    const y = midY - amplitude * midY;
    if (i === 0) {
      ctx.moveTo(i * step, y);
    } else {
      ctx.lineTo(i * step, y);
    }
  }
  ctx.stroke();
}

export function WaveformMonitor({ id }: NodeProps<WaveformMonitorNode>) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const renderData = useAppStore((s) => s.nodeRenderData[id]);
  const samples = renderData?.type === "waveformMonitor" ? renderData.data.samples : [];

  useEffect(() => {
    drawWaveform(canvasRef.current, samples);
  }, [samples]);

  return (
    <NodeShell accent={NODE_ACCENTS.waveformMonitor} title="Waveform Monitor" minWidth="16rem">
      <canvas
        ref={canvasRef}
        width={CANVAS_WIDTH}
        height={CANVAS_HEIGHT}
        className="rounded"
        style={{ background: BACKGROUND_COLOR }}
      />
      <AudioHandle type="target" position={Position.Left} id="WaveformMonitor-target" />
      <AudioHandle type="source" position={Position.Right} id="WaveformMonitor-source" />
    </NodeShell>
  );
}

const definition: NodeDefinition<WaveformMonitorNode> = {
  component: WaveformMonitor,
  toAudioNode: (node) => ({
    type: "waveformMonitor",
    data: { id: node.id, windowSize: node.data.windowSize ?? 2048 },
  }),
};

export default definition;
