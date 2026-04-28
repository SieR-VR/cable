import type { Meta, StoryObj } from "@storybook/react-vite";

import { NodeCanvas, mockInputDevices } from "../../.storybook/decorators";
import { AudioInputDeviceNode } from "@/nodes/AudioInputDevice";

function makeNode(data: AudioInputDeviceNode["data"]): AudioInputDeviceNode {
  return {
    id: "audio-input-1",
    type: "audioInputDevice",
    position: { x: 80, y: 80 },
    data,
  };
}

const meta: Meta<typeof NodeCanvas> = {
  title: "Nodes/AudioInputDevice",
  component: NodeCanvas,
  parameters: { layout: "centered", backgrounds: { default: "graph" } },
};
export default meta;

type Story = StoryObj<typeof NodeCanvas>;

export const NoSelection: Story = {
  args: {
    nodes: [makeNode({ device: null, edgeType: null })],
  },
};

export const DeviceSelected: Story = {
  args: {
    nodes: [
      makeNode({
        device: mockInputDevices[1],
        edgeType: "48kHz/2ch/24bit",
      }),
    ],
  },
};

export const Loading: Story = {
  args: {
    nodes: [makeNode({ device: null, edgeType: null })],
    seed: { inputDevices: null },
  },
};
