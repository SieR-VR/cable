import { invoke } from "@tauri-apps/api/core";
import { render, screen, act } from "@testing-library/react";
import { ReactFlowProvider } from "@xyflow/react";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

import WaveformMonitor from "@/nodes/WaveformMonitor";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockedInvoke = vi.mocked(invoke) as unknown as ReturnType<
  typeof vi.fn<(...args: any[]) => any>
>;

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

describe("WaveformMonitor", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    mockedInvoke.mockResolvedValue([]);
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.clearAllMocks();
  });

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

  it("starts polling on mount", async () => {
    renderInProvider("node-wm");
    await act(async () => {
      vi.advanceTimersByTime(33);
    });
    expect(mockedInvoke).toHaveBeenCalledWith("get_waveform_data", { nodeId: "node-wm" });
  });

  it("polls at ~30fps (33ms interval)", async () => {
    renderInProvider("node-wm");
    await act(async () => {
      vi.advanceTimersByTime(100);
    });
    // 3 ticks in 100ms at 33ms interval
    expect(mockedInvoke.mock.calls.length).toBeGreaterThanOrEqual(3);
  });

  it("clears interval on unmount", async () => {
    const clearIntervalSpy = vi.spyOn(globalThis, "clearInterval");
    const { unmount } = renderInProvider();
    unmount();
    expect(clearIntervalSpy).toHaveBeenCalled();
  });
});
