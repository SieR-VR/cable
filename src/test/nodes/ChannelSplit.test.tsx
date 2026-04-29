import { render, screen, fireEvent } from "@testing-library/react";
import { ReactFlowProvider } from "@xyflow/react";
import { describe, it, expect, vi } from "vitest";

import { ChannelSplit } from "@/nodes/ChannelSplit";

const mockUpdateNode = vi.fn();
vi.mock("@/state", () => ({
  useAppStore: (sel: (s: any) => any) => sel({ updateNode: mockUpdateNode }),
}));

function makeProps(id = "node-1", outputCount: 2 | 4 | 6 | 8 = 2) {
  return {
    id,
    type: "channelSplit" as const,
    data: { outputCount },
    selected: false,
    isConnectable: true,
    zIndex: 0,
    xPos: 0,
    yPos: 0,
    positionAbsoluteX: 0,
    positionAbsoluteY: 0,
    dragging: false,
    draggable: true,
    selectable: true,
    deletable: true,
  } as Parameters<typeof ChannelSplit>[0];
}

function renderInProvider(id?: string, outputCount?: 2 | 4 | 6 | 8) {
  return render(
    <ReactFlowProvider>
      <ChannelSplit {...makeProps(id, outputCount)} />
    </ReactFlowProvider>,
  );
}

describe("ChannelSplit", () => {
  it("renders target handle", () => {
    renderInProvider();
    expect(document.querySelector('[data-handleid="ChannelSplit-target"]')).toBeTruthy();
  });

  it("renders per-channel source handles for default 2ch", () => {
    renderInProvider();
    expect(document.querySelector('[data-handleid="ch-0"]')).toBeTruthy();
    expect(document.querySelector('[data-handleid="ch-1"]')).toBeTruthy();
    expect(document.querySelector('[data-handleid="ch-2"]')).toBeFalsy();
  });

  it("renders header label", () => {
    renderInProvider();
    expect(screen.getByText("Channel Split")).toBeTruthy();
  });

  it("renders stereo channel labels for 2ch", () => {
    renderInProvider();
    expect(screen.getByText("L / Ch 0")).toBeTruthy();
    expect(screen.getByText("R / Ch 1")).toBeTruthy();
  });

  it("renders 4 source handles when outputCount is 4", () => {
    renderInProvider("node-1", 4);
    expect(document.querySelector('[data-handleid="ch-0"]')).toBeTruthy();
    expect(document.querySelector('[data-handleid="ch-3"]')).toBeTruthy();
    expect(document.querySelector('[data-handleid="ch-4"]')).toBeFalsy();
  });

  it("renders segment buttons and calls updateNode on click", () => {
    renderInProvider();
    const btn4 = screen.getByText("4ch");
    fireEvent.click(btn4);
    expect(mockUpdateNode).toHaveBeenCalledWith("node-1", { outputCount: 4 });
  });
});
