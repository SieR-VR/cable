import type { Meta, StoryObj } from "@storybook/react-vite";

import { NodeCanvas, mockVstPlugins } from "../../.storybook/decorators";
import { VstNodeType } from "@/nodes/VstNode";

function makeNode(data: VstNodeType["data"]): VstNodeType {
  return {
    id: "vst-1",
    type: "vst",
    position: { x: 80, y: 80 },
    data,
  };
}

const meta: Meta<typeof NodeCanvas> = {
  title: "Nodes/VstNode",
  component: NodeCanvas,
  parameters: { layout: "centered", backgrounds: { default: "graph" } },
};
export default meta;

type Story = StoryObj<typeof NodeCanvas>;

export const NoPluginSelected: Story = {
  args: {
    nodes: [
      makeNode({ pluginPath: "", numInputs: 0, numOutputs: 0, channels: 2, params: [] }),
    ],
  },
};

export const PluginSelected: Story = {
  args: {
    nodes: [
      makeNode({
        pluginPath: mockVstPlugins[0].path,
        numInputs: mockVstPlugins[0].numInputs,
        numOutputs: mockVstPlugins[0].numOutputs,
        channels: 2,
        params: Array(mockVstPlugins[0].numParams).fill(0.5),
      }),
    ],
  },
};

export const NoScannedPlugins: Story = {
  args: {
    nodes: [
      makeNode({ pluginPath: "", numInputs: 0, numOutputs: 0, channels: 2, params: [] }),
    ],
    seed: { vstPluginList: [] },
  },
};
