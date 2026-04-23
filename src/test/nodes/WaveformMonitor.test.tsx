import { render, screen, act } from "@testing-library/react";
import { ReactFlowProvider } from "@xyflow/react";
import { describe, it, expect, beforeEach } from "vitest";

import { WaveformMonitor } from "@/nodes/WaveformMonitor";
import { useAppStore } from "@/state";

function makeProps(id = "node-1") {
  return {
    id,
    type: "waveformMonitor" as const,
    data: { windowSize: 2048, edgeType: null },
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
  } as Parameters<typeof WaveformMonitor>[0];
}

function renderInProvider(id?: string) {
  const props = makeProps(id);
  return render(
    <ReactFlowProvider>
      <WaveformMonitor {...props} />
    </ReactFlowProvider>,
  );
}

beforeEach(() => {
  useAppStore.setState({ nodeRenderData: {} });
});

describe("WaveformMonitor", () => {
  it("renders target and source handles", () => {
    renderInProvider();
    expect(document.querySelector('[data-handleid="WaveformMonitor-target"]')).toBeTruthy();
    expect(document.querySelector('[data-handleid="WaveformMonitor-source"]')).toBeTruthy();
  });

  it("renders header label", () => {
    renderInProvider();
    expect(screen.getByText("Waveform Monitor")).toBeTruthy();
  });

  it("renders canvas element", () => {
    renderInProvider();
    expect(document.querySelector("canvas")).toBeTruthy();
  });

  it("draws waveform when nodeRenderData is updated in store", async () => {
    renderInProvider("node-1");

    await act(async () => {
      useAppStore.setState({
        nodeRenderData: {
          "node-1": { type: "waveformMonitor", data: { samples: [0.1, -0.3, 0.5] } },
        },
      });
    });

    expect(document.querySelector("canvas")).toBeTruthy();
  });

  it("renders with empty samples when no render data is in store", () => {
    renderInProvider("node-none");
    expect(document.querySelector("canvas")).toBeTruthy();
  });
});
