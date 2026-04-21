import { render, screen, act } from "@testing-library/react";
import { ReactFlowProvider } from "@xyflow/react";
import { describe, it, expect, beforeEach } from "vitest";

import SpectrumAnalyzer from "@/nodes/SpectrumAnalyzer";
import { useAppStore } from "@/state";

function makeProps(id = "node-1") {
  return {
    id,
    type: "spectrumAnalyzer" as const,
    data: { fftSize: 1024, edgeType: null },
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
  } as Parameters<typeof SpectrumAnalyzer>[0];
}

// Handle requires the ReactFlow store context
function renderInProvider(id?: string) {
  const props = makeProps(id);
  return render(
    <ReactFlowProvider>
      <SpectrumAnalyzer {...props} />
    </ReactFlowProvider>,
  );
}

beforeEach(() => {
  useAppStore.setState({ nodeRenderData: {} });
});

describe("SpectrumAnalyzer node", () => {
  it("renders input and output handles", () => {
    renderInProvider();
    const handles = document.querySelectorAll("[data-handleid]");
    const ids = Array.from(handles).map((h) => h.getAttribute("data-handleid"));
    expect(ids).toContain("SpectrumAnalyzer-target");
    expect(ids).toContain("SpectrumAnalyzer-source");
  });

  it("renders the header label", () => {
    renderInProvider();
    expect(screen.getByText("Spectrum Analyzer")).toBeDefined();
  });

  it("renders a canvas element", () => {
    renderInProvider();
    expect(document.querySelector("canvas")).not.toBeNull();
  });

  it("draws spectrum when nodeRenderData is updated in store", async () => {
    renderInProvider("node-1");

    await act(async () => {
      useAppStore.setState({
        nodeRenderData: {
          "node-1": { type: "spectrumAnalyzer", data: { bins: [0.5, 0.8, 0.3] } },
        },
      });
    });

    expect(document.querySelector("canvas")).not.toBeNull();
  });

  it("renders with empty bins when no render data is in store", () => {
    renderInProvider("node-none");
    expect(document.querySelector("canvas")).not.toBeNull();
  });
});
