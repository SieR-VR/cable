import { render, screen } from "@testing-library/react";
import { ReactFlowProvider } from "@xyflow/react";
import { describe, it, expect, vi } from "vitest";

import { ChannelMerge } from "@/nodes/ChannelMerge";

vi.mock("@/state", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/state")>();
  return {
    ...actual,
    useAppStore: (selector: any) =>
      selector({ ...actual.useAppStore.getState(), updateNode: vi.fn() }),
  };
});

function makeProps(id = "node-1", inputCount: 2 | 4 | 6 | 8 = 2) {
  return {
    id,
    type: "channelMerge" as const,
    data: { inputCount },
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
  } as Parameters<typeof ChannelMerge>[0];
}

function renderInProvider(id?: string, inputCount: 2 | 4 | 6 | 8 = 2) {
  return render(
    <ReactFlowProvider>
      <ChannelMerge {...makeProps(id, inputCount)} />
    </ReactFlowProvider>,
  );
}

describe("ChannelMerge", () => {
  it("renders header label", () => {
    renderInProvider();
    expect(screen.getByText("Channel Merge")).toBeTruthy();
  });

  it("renders source handle", () => {
    renderInProvider();
    expect(document.querySelector('[data-handleid="ChannelMerge-source"]')).toBeTruthy();
  });

  it("renders 2 input handles by default", () => {
    renderInProvider();
    expect(document.querySelector('[data-handleid="ch-0"]')).toBeTruthy();
    expect(document.querySelector('[data-handleid="ch-1"]')).toBeTruthy();
    expect(document.querySelector('[data-handleid="ch-2"]')).toBeNull();
  });

  it("renders 4 input handles when inputCount=4", () => {
    renderInProvider("node-1", 4);
    expect(document.querySelector('[data-handleid="ch-3"]')).toBeTruthy();
    expect(document.querySelector('[data-handleid="ch-4"]')).toBeNull();
  });

  it("renders L/R labels for 2-channel mode", () => {
    renderInProvider();
    expect(screen.getByText("L")).toBeTruthy();
    expect(screen.getByText("R")).toBeTruthy();
  });

  it("renders segmented control buttons", () => {
    renderInProvider();
    expect(screen.getByText("2ch")).toBeTruthy();
    expect(screen.getByText("4ch")).toBeTruthy();
    expect(screen.getByText("6ch")).toBeTruthy();
    expect(screen.getByText("8ch")).toBeTruthy();
  });
});
