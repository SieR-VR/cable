import { render, screen } from "@testing-library/react";
import { ReactFlowProvider } from "@xyflow/react";
import { describe, it, expect } from "vitest";

import { ChannelSplit } from "@/nodes/ChannelSplit";

function makeProps(id = "node-1") {
  return {
    id,
    type: "channelSplit" as const,
    data: { edgeType: null },
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

function renderInProvider(id?: string) {
  return render(
    <ReactFlowProvider>
      <ChannelSplit {...makeProps(id)} />
    </ReactFlowProvider>,
  );
}

describe("ChannelSplit", () => {
  it("renders target handle", () => {
    renderInProvider();
    expect(document.querySelector('[data-handleid="ChannelSplit-target"]')).toBeTruthy();
  });

  it("renders per-channel source handles", () => {
    renderInProvider();
    expect(document.querySelector('[data-handleid="ch-0"]')).toBeTruthy();
    expect(document.querySelector('[data-handleid="ch-1"]')).toBeTruthy();
  });

  it("renders header label", () => {
    renderInProvider();
    expect(screen.getByText("Channel Split")).toBeTruthy();
  });

  it("renders channel labels", () => {
    renderInProvider();
    expect(screen.getByText("L / Ch 0")).toBeTruthy();
    expect(screen.getByText("R / Ch 1")).toBeTruthy();
  });
});
