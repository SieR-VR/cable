import { render, screen } from "@testing-library/react";
import { ReactFlowProvider } from "@xyflow/react";
import { describe, it, expect } from "vitest";

import { Echo } from "@/nodes/Echo";

function makeProps(id = "node-1") {
  return {
    id,
    type: "echo" as const,
    data: { delayMs: 375, feedback: 0.4, wet: 0.5, edgeType: null },
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
  } as Parameters<typeof Echo>[0];
}

function renderInProvider(id?: string) {
  return render(
    <ReactFlowProvider>
      <Echo {...makeProps(id)} />
    </ReactFlowProvider>,
  );
}

describe("Echo", () => {
  it("renders target and source handles", () => {
    renderInProvider();
    expect(document.querySelector('[data-handleid="Echo-target"]')).toBeTruthy();
    expect(document.querySelector('[data-handleid="Echo-source"]')).toBeTruthy();
  });

  it("renders header label", () => {
    renderInProvider();
    expect(screen.getByText("Echo")).toBeTruthy();
  });

  it("renders all parameter sliders", () => {
    renderInProvider();
    const sliders = document.querySelectorAll('input[type="range"]');
    expect(sliders.length).toBe(3);
  });

  it("renders parameter labels", () => {
    renderInProvider();
    expect(screen.getByText("Time")).toBeTruthy();
    expect(screen.getByText("Feedback")).toBeTruthy();
    expect(screen.getByText("Wet")).toBeTruthy();
  });
});
