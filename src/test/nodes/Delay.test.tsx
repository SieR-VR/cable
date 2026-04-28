import { render, screen } from "@testing-library/react";
import { ReactFlowProvider } from "@xyflow/react";
import { describe, it, expect } from "vitest";

import { Delay } from "@/nodes/Delay";

function makeProps(id = "node-1") {
  return {
    id,
    type: "delay" as const,
    data: { delayMs: 250, edgeType: null },
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
  } as Parameters<typeof Delay>[0];
}

function renderInProvider(id?: string) {
  return render(
    <ReactFlowProvider>
      <Delay {...makeProps(id)} />
    </ReactFlowProvider>,
  );
}

describe("Delay", () => {
  it("renders target and source handles", () => {
    renderInProvider();
    expect(document.querySelector('[data-handleid="Delay-target"]')).toBeTruthy();
    expect(document.querySelector('[data-handleid="Delay-source"]')).toBeTruthy();
  });

  it("renders header label", () => {
    renderInProvider();
    expect(screen.getByText("Delay")).toBeTruthy();
  });

  it("renders delay slider with default value", () => {
    renderInProvider();
    const slider = document.querySelector('input[type="range"]') as HTMLInputElement;
    expect(slider).toBeTruthy();
    expect(parseInt(slider.value, 10)).toBe(250);
  });
});
