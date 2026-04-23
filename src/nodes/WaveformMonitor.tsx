import { useEffect, useRef } from "react";
import { Handle, Node, NodeProps, Position } from "@xyflow/react";

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
    <div className="bg-gray-700 rounded-lg flex flex-col text-white min-w-64">
      {/* Header */}
      <div className="w-full h-6 bg-emerald-500 rounded-t-lg flex items-center text-sm font-bold p-2 drag-handle__custom">
        Waveform Monitor
      </div>
      <div className="flex flex-col gap-2 p-2 relative">
        <canvas
          ref={canvasRef}
          width={CANVAS_WIDTH}
          height={CANVAS_HEIGHT}
          className="rounded"
          style={{ background: BACKGROUND_COLOR }}
        />
        <div className="flex flex-row gap-1 items-center">
          <span className="rounded-md text-xs bg-emerald-300 text-emerald-900 p-1">time-domain</span>
          <span className="rounded-md text-xs bg-gray-500 p-1">passthrough</span>
        </div>
        <Handle
          type="target"
          position={Position.Left}
          id="WaveformMonitor-target"
          className="w-4 h-4 bg-emerald-400 rounded-full"
        />
        <Handle
          type="source"
          position={Position.Right}
          id="WaveformMonitor-source"
          className="w-4 h-4 bg-emerald-400 rounded-full"
        />
      </div>
    </div>
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
