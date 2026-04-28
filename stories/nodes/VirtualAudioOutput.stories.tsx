import type { Meta, StoryObj } from "@storybook/react-vite";

import { NodeCanvas } from "../../.storybook/decorators";
import { VirtualAudioOutputNode } from "@/nodes/VirtualAudioOutput";

function makeNode(data: VirtualAudioOutputNode["data"]): VirtualAudioOutputNode {
  return {
    id: "virtual-output-1",
    type: "virtualAudioOutput",
    position: { x: 80, y: 80 },
    data,
  };
}

const meta: Meta<typeof NodeCanvas> = {
  title: "Nodes/VirtualAudioOutput",
  component: NodeCanvas,
  parameters: { layout: "centered", backgrounds: { default: "graph" } },
};
export default meta;

type Story = StoryObj<typeof NodeCanvas>;

export const NoSelection: Story = {
  args: {
    nodes: [makeNode({ deviceId: "", name: "", edgeType: null })],
  },
};

export const DeviceSelected: Story = {
  args: {
    nodes: [
      makeNode({
        deviceId: "virt-render-01",
        name: "Cable Speaker A",
        edgeType: "48kHz/2ch/24bit",
      }),
    ],
  },
};

export const DriverDisconnected: Story = {
  args: {
    nodes: [makeNode({ deviceId: "", name: "", edgeType: null })],
    seed: { driverConnected: false, virtualDevices: [] },
  },
};
