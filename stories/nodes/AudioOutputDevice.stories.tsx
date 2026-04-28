import type { Meta, StoryObj } from "@storybook/react-vite";

import { NodeCanvas, mockOutputDevices } from "../../.storybook/decorators";
import { AudioOutputDeviceNode } from "@/nodes/AudioOutputDevice";

function makeNode(data: AudioOutputDeviceNode["data"]): AudioOutputDeviceNode {
  return {
    id: "audio-output-1",
    type: "audioOutputDevice",
    position: { x: 80, y: 80 },
    data,
  };
}

const meta: Meta<typeof NodeCanvas> = {
  title: "Nodes/AudioOutputDevice",
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
        device: mockOutputDevices[0],
        edgeType: "48kHz/2ch/24bit",
      }),
    ],
  },
};

export const Loading: Story = {
  args: {
    nodes: [makeNode({ device: null, edgeType: null })],
    seed: { outputDevices: null },
  },
};
