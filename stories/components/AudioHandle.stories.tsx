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

// ---------------------------------------------------------------------------
// Story node: shows one AudioHandle on each side, driven by `data.edgeType`.
// ---------------------------------------------------------------------------

type DemoNodeData = {
  label: string;
  edgeType: string | null;
  showSource?: boolean;
  showTarget?: boolean;
  position?: Position;
};

function DemoNode({ data }: NodeProps<Node<DemoNodeData, "demo">>) {
  const sourcePos = data.position ?? Position.Right;
  const targetPos =
    sourcePos === Position.Right
      ? Position.Left
      : sourcePos === Position.Left
        ? Position.Right
        : sourcePos === Position.Top
          ? Position.Bottom
          : Position.Top;
  return (
    <div className="bg-gray-700 rounded-lg flex flex-col items-center text-white min-w-32">
      <div className="w-full h-6 bg-sky-500 rounded-t-lg flex items-center text-xs font-bold p-2">
        {data.label}
      </div>
      <div className="p-3 text-xs text-gray-300">
        {data.edgeType ?? "(no format)"}
      </div>
      {data.showTarget !== false && (
        <AudioHandle type="target" position={targetPos} id="in" edgeType={data.edgeType} />
      )}
      {data.showSource !== false && (
        <AudioHandle type="source" position={sourcePos} id="out" edgeType={data.edgeType} />
      )}
    </div>
  );
}

const nodeTypes = { demo: DemoNode };

// ---------------------------------------------------------------------------

interface CanvasArgs {
  edgeType: string | null;
  position: Position;
  connected: boolean;
}

function HandleCanvas({ edgeType, position, connected }: CanvasArgs) {
  const nodes: Node<DemoNodeData, "demo">[] = [
    {
      id: "a",
      type: "demo",
      position: { x: 40, y: 80 },
      data: { label: "Source", edgeType, showTarget: false, position },
    },
  ];
  const edges: Edge[] = [];
  if (connected) {
    nodes.push({
      id: "b",
      type: "demo",
      position:
        position === Position.Right
          ? { x: 280, y: 80 }
          : position === Position.Left
            ? { x: -200, y: 80 }
            : position === Position.Bottom
              ? { x: 40, y: 280 }
              : { x: 40, y: -120 },
      data: { label: "Sink", edgeType, showSource: false, position },
    });
    edges.push({
      id: "a-b",
      source: "a",
      target: "b",
      sourceHandle: "out",
      targetHandle: "in",
      type: "audio",
    });
  }

  return (
    <div style={{ width: 600, height: 360, background: "#0e1116" }}>
      <ReactFlowProvider>
        <ReactFlow
          nodes={nodes}
          edges={edges}
          nodeTypes={nodeTypes}
          fitView
          proOptions={{ hideAttribution: true }}
        >
          <Background color="#21262d" gap={16} />
        </ReactFlow>
      </ReactFlowProvider>
    </div>
  );
}

const meta: Meta<typeof HandleCanvas> = {
  title: "Audio/AudioHandle",
  component: HandleCanvas,
  argTypes: {
    edgeType: {
      control: "select",
      options: [
        null,
        "audio_44100Hz_1ch_16bit",
        "audio_48000Hz_2ch_24bit",
        "audio_48000Hz_3ch_24bit",
        "audio_96000Hz_4ch_24bit",
        "audio_96000Hz_6ch_32bit",
        "audio_192000Hz_8ch_32bit",
      ],
    },
    position: {
      control: "select",
      options: [Position.Left, Position.Right, Position.Top, Position.Bottom],
    },
    connected: { control: "boolean" },
  },
  args: {
    edgeType: "audio_48000Hz_2ch_24bit",
    position: Position.Right,
    connected: false,
  },
};
export default meta;

type Story = StoryObj<typeof HandleCanvas>;

export const Stereo48k24: Story = {};

export const Mono44k16: Story = {
  args: { edgeType: "audio_44100Hz_1ch_16bit" },
};

export const Quad96k24: Story = {
  args: { edgeType: "audio_96000Hz_4ch_24bit" },
};

export const SixCh96k32: Story = {
  args: { edgeType: "audio_96000Hz_6ch_32bit" },
};

export const EightCh192k32: Story = {
  args: { edgeType: "audio_192000Hz_8ch_32bit" },
};

export const Disabled: Story = {
  args: { edgeType: null },
};

export const Connected: Story = {
  args: { connected: true },
};

export const TopBottom: Story = {
  args: { position: Position.Bottom, connected: true },
};
