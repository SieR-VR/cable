import type { Meta, StoryObj } from "@storybook/react-vite";
import { useEffect } from "react";

import { ContextMenu } from "@/components/ContextMenu";
import { useAppStore } from "@/state";
import { seedAppStore } from "../../.storybook/decorators";

interface ContextMenuStoryProps {
  driverConnected: boolean;
  hasTargetNode: boolean;
  x: number;
  y: number;
}

function ContextMenuHarness({
  driverConnected,
  hasTargetNode,
  x,
  y,
}: ContextMenuStoryProps) {
  useEffect(() => {
    seedAppStore({ driverConnected });
    useAppStore.setState({
      contextMenuOpen: true,
      contextMenuPosition: { x, y },
      contextMenuFlowPosition: { x: 0, y: 0 },
      contextMenuTargetNodeId: hasTargetNode ? "node-1" : null,
    });
  }, [driverConnected, hasTargetNode, x, y]);

  return (
    <div
      style={{ position: "relative", width: 720, height: 520 }}
      className="bg-[#0e1116]"
    >
      <ContextMenu />
    </div>
  );
}

const meta: Meta<typeof ContextMenuHarness> = {
  title: "Components/ContextMenu",
  component: ContextMenuHarness,
  parameters: { layout: "centered", backgrounds: { default: "graph" } },
  args: { driverConnected: true, hasTargetNode: false, x: 40, y: 40 },
};
export default meta;

type Story = StoryObj<typeof ContextMenuHarness>;

export const AddNodeMenu: Story = {};

export const DriverDisconnected: Story = {
  args: { driverConnected: false },
};

export const OnExistingNode: Story = {
  args: { hasTargetNode: true },
};
