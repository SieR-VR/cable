import { render, screen } from "@testing-library/react";
import { ReactFlowProvider } from "@xyflow/react";
import { describe, it, expect } from "vitest";

import { Gain } from "@/nodes/Gain";

function makeProps(id = "node-1") {
  return {
    id,
    type: "gain" as const,
    data: { gain: 1.0, edgeType: null },
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
  } as Parameters<typeof Gain>[0];
}

function renderInProvider(id?: string) {
  return render(
    <ReactFlowProvider>
      <Gain {...makeProps(id)} />
    </ReactFlowProvider>,
  );
}

describe("Gain", () => {
  it("renders target and source handles", () => {
    renderInProvider();
    expect(document.querySelector('[data-handleid="Gain-target"]')).toBeTruthy();
    expect(document.querySelector('[data-handleid="Gain-source"]')).toBeTruthy();
  });

  it("renders header label", () => {
    renderInProvider();
    // "Gain" appears in both the NodeShell title and the slider label.
    const matches = screen.getAllByText("Gain");
    expect(matches.length).toBeGreaterThanOrEqual(1);
  });

  it("renders gain slider", () => {
    renderInProvider();
    const slider = document.querySelector('input[type="range"]') as HTMLInputElement;
    expect(slider).toBeTruthy();
    expect(parseFloat(slider.value)).toBe(1.0);
  });
});
