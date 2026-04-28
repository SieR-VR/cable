import type { Meta, StoryObj } from "@storybook/react-vite";
import {
  Background,
  Edge,
  Node,
  NodeProps,
  Position,
  ReactFlow,
  ReactFlowProvider,
} from "@xyflow/react";

import { AudioHandle } from "@/components/AudioHandle";
import { edgeTypes } from "@/components/AudioEdge";

type DemoNodeData = { label: string; edgeType: string | null };

function SourceNode({ data }: NodeProps<Node<DemoNodeData, "src">>) {
  return (
    <div className="bg-gray-700 rounded-lg flex flex-col items-center text-white min-w-32">
      <div className="w-full h-6 bg-emerald-500 rounded-t-lg flex items-center text-xs font-bold p-2">
        {data.label}
      </div>
      <div className="p-3 text-xs text-gray-300">{data.edgeType}</div>
      <AudioHandle type="source" position={Position.Right} id="out" edgeType={data.edgeType} />
    </div>
  );
}

function SinkNode({ data }: NodeProps<Node<DemoNodeData, "snk">>) {
  return (
    <div className="bg-gray-700 rounded-lg flex flex-col items-center text-white min-w-32">
      <div className="w-full h-6 bg-rose-500 rounded-t-lg flex items-center text-xs font-bold p-2">
        {data.label}
      </div>
      <div className="p-3 text-xs text-gray-300">{data.edgeType}</div>
      <AudioHandle type="target" position={Position.Left} id="in" edgeType={data.edgeType} />
    </div>
  );
}

const nodeTypes = { src: SourceNode, snk: SinkNode };

interface CanvasArgs {
  edgeType: string;
  vertical: boolean;
}

function EdgeCanvas({ edgeType, vertical }: CanvasArgs) {
  const m = /^audio_(\d+)Hz_(\d+)ch_(\d+)bit$/.exec(edgeType);
  const edgeData = m
    ? {
        frequency: parseInt(m[1], 10),
        channels: parseInt(m[2], 10),
        bitsPerSample: parseInt(m[3], 10),
      }
    : undefined;

  const nodes: Node[] = [
    {
      id: "a",
      type: "src",
      position: { x: 40, y: 80 },
      data: { label: "Source", edgeType },
    },
    {
      id: "b",
      type: "snk",
      position: vertical ? { x: 40, y: 320 } : { x: 360, y: 80 },
      data: { label: "Sink", edgeType },
    },
  ];
  const edges: Edge[] = [
    {
      id: "a-b",
      source: "a",
      target: "b",
      sourceHandle: "out",
      targetHandle: "in",
      type: "audio",
      data: edgeData,
    },
  ];

  return (
    <div style={{ width: 700, height: 480, background: "#0e1116" }}>
      <ReactFlowProvider>
        <ReactFlow
          nodes={nodes}
          edges={edges}
          nodeTypes={nodeTypes}
          edgeTypes={edgeTypes}
          fitView
          proOptions={{ hideAttribution: true }}
        >
          <Background color="#21262d" gap={16} />
        </ReactFlow>
      </ReactFlowProvider>
    </div>
  );
}

const meta: Meta<typeof EdgeCanvas> = {
  title: "Audio/AudioEdge",
  component: EdgeCanvas,
  argTypes: {
    edgeType: {
      control: "select",
      options: [
        "audio_44100Hz_1ch_16bit",
        "audio_48000Hz_2ch_24bit",
        "audio_48000Hz_3ch_24bit",
        "audio_96000Hz_4ch_24bit",
        "audio_96000Hz_6ch_32bit",
        "audio_192000Hz_8ch_32bit",
      ],
    },
    vertical: { control: "boolean" },
  },
  args: {
    edgeType: "audio_48000Hz_2ch_24bit",
    vertical: false,
  },
};
export default meta;

type Story = StoryObj<typeof EdgeCanvas>;

export const Stereo48k24: Story = {};
export const Mono44k16: Story = { args: { edgeType: "audio_44100Hz_1ch_16bit" } };
export const Quad96k24: Story = { args: { edgeType: "audio_96000Hz_4ch_24bit" } };
export const SixCh96k32: Story = { args: { edgeType: "audio_96000Hz_6ch_32bit" } };
export const EightCh192k32: Story = { args: { edgeType: "audio_192000Hz_8ch_32bit" } };

export const Vertical: Story = {
  args: { vertical: true, edgeType: "audio_96000Hz_4ch_24bit" },
};
