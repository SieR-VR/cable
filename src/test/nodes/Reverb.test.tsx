import { render, screen } from "@testing-library/react";
import { ReactFlowProvider } from "@xyflow/react";
import { describe, it, expect } from "vitest";

import { Reverb } from "@/nodes/Reverb";

function makeProps(id = "node-1") {
  return {
    id,
    type: "reverb" as const,
    data: { roomSize: 0.5, wet: 0.33, damp: 0.5, edgeType: null },
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
  } as Parameters<typeof Reverb>[0];
}

function renderInProvider(id?: string) {
  return render(
    <ReactFlowProvider>
      <Reverb {...makeProps(id)} />
    </ReactFlowProvider>,
  );
}

describe("Reverb", () => {
  it("renders target and source handles", () => {
    renderInProvider();
    expect(document.querySelector('[data-handleid="Reverb-target"]')).toBeTruthy();
    expect(document.querySelector('[data-handleid="Reverb-source"]')).toBeTruthy();
  });

  it("renders header label", () => {
    renderInProvider();
    expect(screen.getByText("Reverb")).toBeTruthy();
  });

  it("renders all parameter sliders", () => {
    renderInProvider();
    const sliders = document.querySelectorAll('input[type="range"]');
    expect(sliders.length).toBe(3);
  });

  it("renders parameter labels", () => {
    renderInProvider();
    expect(screen.getByText("Room")).toBeTruthy();
    expect(screen.getByText("Wet")).toBeTruthy();
    expect(screen.getByText("Damp")).toBeTruthy();
  });
});
