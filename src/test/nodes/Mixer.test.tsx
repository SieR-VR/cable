import { render, screen } from "@testing-library/react";
import { ReactFlowProvider } from "@xyflow/react";
import { describe, it, expect } from "vitest";

import Mixer from "@/nodes/Mixer";

function makeProps(id = "node-1") {
  return {
    id,
    type: "mixer" as const,
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
  } as Parameters<typeof Mixer>[0];
}

function renderInProvider(id?: string) {
  return render(
    <ReactFlowProvider>
      <Mixer {...makeProps(id)} />
    </ReactFlowProvider>,
  );
}

describe("Mixer", () => {
  it("renders target and source handles", () => {
    renderInProvider();
    expect(document.querySelector('[data-handleid="Mixer-target"]')).toBeTruthy();
    expect(document.querySelector('[data-handleid="Mixer-source"]')).toBeTruthy();
  });

  it("renders header label", () => {
    renderInProvider();
    expect(screen.getByText("Mixer")).toBeTruthy();
  });

  it("renders sum+clamp badge", () => {
    renderInProvider();
    expect(screen.getByText("sum + clamp")).toBeTruthy();
  });

  it("renders passthrough badge", () => {
    renderInProvider();
    expect(screen.getByText("passthrough")).toBeTruthy();
  });
});
