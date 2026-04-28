import type { Meta, StoryObj } from "@storybook/react-vite";

import { NodeCanvas } from "../../.storybook/decorators";
import { AppAudioCaptureNode } from "@/nodes/AppAudioCapture";

function makeNode(data: AppAudioCaptureNode["data"]): AppAudioCaptureNode {
  return {
    id: "app-capture-1",
    type: "appAudioCapture",
    position: { x: 80, y: 80 },
    data,
  };
}

const meta: Meta<typeof NodeCanvas> = {
  title: "Nodes/AppAudioCapture",
  component: NodeCanvas,
  parameters: { layout: "centered", backgrounds: { default: "graph" } },
};
export default meta;

type Story = StoryObj<typeof NodeCanvas>;

export const NoSelection: Story = {
  args: {
    nodes: [makeNode({ processId: null, windowTitle: null, edgeType: null })],
  },
};

export const WindowSelected: Story = {
  args: {
    nodes: [
      makeNode({
        processId: 5678,
        windowTitle: "Chrome — Storybook",
        edgeType: "48kHz/2ch/24bit",
      }),
    ],
  },
};
