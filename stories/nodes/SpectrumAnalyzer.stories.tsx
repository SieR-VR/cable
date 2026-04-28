import type { Meta, StoryObj } from "@storybook/react-vite";

import { NodeCanvas } from "../../.storybook/decorators";
import { SpectrumAnalyzerNode } from "@/nodes/SpectrumAnalyzer";

const NODE_ID = "spectrum-1";

function makeNode(data: SpectrumAnalyzerNode["data"]): SpectrumAnalyzerNode {
  return {
    id: NODE_ID,
    type: "spectrumAnalyzer",
    position: { x: 80, y: 80 },
    data,
  };
}

// Simulated FFT bins resembling pink noise.
const fakeBins = Array.from({ length: 64 }, (_, i) => 1 / (i + 1) + Math.random() * 0.05);

const meta: Meta<typeof NodeCanvas> = {
  title: "Nodes/SpectrumAnalyzer",
  component: NodeCanvas,
  parameters: { layout: "centered", backgrounds: { default: "graph" } },
};
export default meta;

type Story = StoryObj<typeof NodeCanvas>;

export const NoData: Story = {
  args: {
    nodes: [makeNode({ fftSize: 1024, edgeType: null })],
  },
};

export const WithSignal: Story = {
  args: {
    nodes: [makeNode({ fftSize: 1024, edgeType: "48kHz/2ch/24bit" })],
    seed: {
      nodeRenderData: {
        [NODE_ID]: { type: "spectrumAnalyzer", data: { bins: fakeBins } },
      },
    },
  },
};
