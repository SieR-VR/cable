import { render, screen } from "@testing-library/react";
import { ReactFlowProvider } from "@xyflow/react";
import { describe, it, expect } from "vitest";

import { Compressor } from "@/nodes/Compressor";

function makeProps(id = "node-1") {
  return {
    id,
    type: "compressor" as const,
    data: { thresholdDb: -12, ratio: 4, attackMs: 5, releaseMs: 50, makeUpDb: 0, edgeType: null },
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
  } as Parameters<typeof Compressor>[0];
}

function renderInProvider(id?: string) {
  return render(
    <ReactFlowProvider>
      <Compressor {...makeProps(id)} />
    </ReactFlowProvider>,
  );
}

describe("Compressor", () => {
  it("renders target and source handles", () => {
    renderInProvider();
    expect(document.querySelector('[data-handleid="Compressor-target"]')).toBeTruthy();
    expect(document.querySelector('[data-handleid="Compressor-source"]')).toBeTruthy();
  });

  it("renders header label", () => {
    renderInProvider();
    expect(screen.getByText("Compressor")).toBeTruthy();
  });

  it("renders all parameter sliders", () => {
    renderInProvider();
    const sliders = document.querySelectorAll('input[type="range"]');
    expect(sliders.length).toBe(5);
  });

  it("renders parameter labels", () => {
    renderInProvider();
    expect(screen.getByText("Threshold")).toBeTruthy();
    expect(screen.getByText("Ratio")).toBeTruthy();
    expect(screen.getByText("Attack")).toBeTruthy();
    expect(screen.getByText("Release")).toBeTruthy();
    expect(screen.getByText("Make-up")).toBeTruthy();
  });
});
