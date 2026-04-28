import type { Meta, StoryObj } from "@storybook/react-vite";

import { NodeCanvas } from "../../.storybook/decorators";
import { VirtualAudioInputNode } from "@/nodes/VirtualAudioInput";

function makeNode(data: VirtualAudioInputNode["data"]): VirtualAudioInputNode {
  return {
    id: "virtual-input-1",
    type: "virtualAudioInput",
    position: { x: 80, y: 80 },
    data,
  };
}

const meta: Meta<typeof NodeCanvas> = {
  title: "Nodes/VirtualAudioInput",
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
        deviceId: "virt-capture-01",
        name: "Cable Mic A",
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

export const NoVirtualDevices: Story = {
  args: {
    nodes: [makeNode({ deviceId: "", name: "", edgeType: null })],
    seed: { virtualDevices: [] },
  },
};
