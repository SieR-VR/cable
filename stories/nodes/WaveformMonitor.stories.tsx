import type { Meta, StoryObj } from "@storybook/react-vite";

import { NodeCanvas } from "../../.storybook/decorators";
import { WaveformMonitorNode } from "@/nodes/WaveformMonitor";

const NODE_ID = "waveform-1";

function makeNode(data: WaveformMonitorNode["data"]): WaveformMonitorNode {
  return {
    id: NODE_ID,
    type: "waveformMonitor",
    position: { x: 80, y: 80 },
    data,
  };
}

const fakeSamples = Array.from({ length: 512 }, (_, i) =>
  Math.sin((i / 512) * Math.PI * 8) * 0.7,
);

const meta: Meta<typeof NodeCanvas> = {
  title: "Nodes/WaveformMonitor",
  component: NodeCanvas,
  parameters: { layout: "centered", backgrounds: { default: "graph" } },
};
export default meta;

type Story = StoryObj<typeof NodeCanvas>;

export const NoData: Story = {
  args: {
    nodes: [makeNode({ windowSize: 2048, edgeType: null })],
  },
};

export const WithSignal: Story = {
  args: {
    nodes: [makeNode({ windowSize: 2048, edgeType: "48kHz/2ch/24bit" })],
    seed: {
      nodeRenderData: {
        [NODE_ID]: { type: "waveformMonitor", data: { samples: fakeSamples } },
      },
    },
  },
};
