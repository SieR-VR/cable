import type { Meta, StoryObj } from "@storybook/react-vite";

import { NodeCanvas } from "../../.storybook/decorators";
import { MixerNodeType } from "@/nodes/Mixer";

const meta: Meta<typeof NodeCanvas> = {
  title: "Nodes/Mixer",
  component: NodeCanvas,
  parameters: { layout: "centered", backgrounds: { default: "graph" } },
};
export default meta;

type Story = StoryObj<typeof NodeCanvas>;

const node: MixerNodeType = {
  id: "mixer-1",
  type: "mixer",
  position: { x: 80, y: 80 },
  data: { edgeType: null },
};

export const Default: Story = {
  args: { nodes: [node] },
};

export const ConfiguredEdgeType: Story = {
  args: {
    nodes: [{ ...node, data: { edgeType: "48kHz/2ch/24bit" } }],
  },
};
