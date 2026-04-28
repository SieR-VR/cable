import { useEffect, useRef } from "react";
import { Node, NodeProps, Position } from "@xyflow/react";

import { AudioHandle } from "@/components/AudioHandle";
import { NODE_ACCENTS, NodeShell } from "@/components/NodeShell";
import { useAppStore } from "../state";
import { NodeDefinition } from "@/node-definition";

export type SpectrumAnalyzerNodeData = {
  /** FFT window size. Must be a power of two. Default: 1024 */
  fftSize: number;
  edgeType: string | null;
};

export type SpectrumAnalyzerNode = Node<SpectrumAnalyzerNodeData, "spectrumAnalyzer">;

const CANVAS_WIDTH = 240;
const CANVAS_HEIGHT = 80;
const BAR_COLOR = "#a855f7"; // purple-500
const BACKGROUND_COLOR = "#111827"; // gray-900

function drawSpectrum(canvas: HTMLCanvasElement | null, bins: number[]): void {
  if (!canvas || bins.length === 0) return;
  const ctx = canvas.getContext("2d");
  if (!ctx) return;

  ctx.fillStyle = BACKGROUND_COLOR;
  ctx.fillRect(0, 0, CANVAS_WIDTH, CANVAS_HEIGHT);

  if (bins.length === 0) return;

  const barWidth = CANVAS_WIDTH / bins.length;
  const maxBin = Math.max(...bins, 1e-6);

  ctx.fillStyle = BAR_COLOR;
  for (let i = 0; i < bins.length; i++) {
    const normalised = bins[i] / maxBin;
    const barHeight = normalised * CANVAS_HEIGHT;
    ctx.fillRect(i * barWidth, CANVAS_HEIGHT - barHeight, Math.max(barWidth - 1, 1), barHeight);
  }
}

export function SpectrumAnalyzer({ id }: NodeProps<SpectrumAnalyzerNode>) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const renderData = useAppStore((s) => s.nodeRenderData[id]);
  const bins = renderData?.type === "spectrumAnalyzer" ? renderData.data.bins : [];

  useEffect(() => {
    drawSpectrum(canvasRef.current, bins);
  }, [bins]);

  return (
    <NodeShell accent={NODE_ACCENTS.spectrumAnalyzer} title="Spectrum Analyzer" minWidth="16rem">
      <canvas
        ref={canvasRef}
        width={CANVAS_WIDTH}
        height={CANVAS_HEIGHT}
        className="rounded"
        style={{ background: BACKGROUND_COLOR }}
      />
      <AudioHandle type="target" position={Position.Left} id="SpectrumAnalyzer-target" />
      <AudioHandle type="source" position={Position.Right} id="SpectrumAnalyzer-source" />
    </NodeShell>
  );
}

const definition: NodeDefinition<SpectrumAnalyzerNode> = {
  component: SpectrumAnalyzer,
  toAudioNode: (node) => ({
    type: "spectrumAnalyzer",
    data: { id: node.id, fftSize: node.data.fftSize ?? 1024 },
  }),
};

export default definition;
