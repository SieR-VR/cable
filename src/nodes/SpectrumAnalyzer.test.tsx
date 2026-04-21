import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { ReactFlowProvider } from "@xyflow/react";
import { invoke } from "@tauri-apps/api/core";
import SpectrumAnalyzer from "./SpectrumAnalyzer";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockedInvoke = vi.mocked(invoke) as unknown as ReturnType<typeof vi.fn<(...args: any[]) => any>>;

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
    dragging: false,
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
  vi.useFakeTimers();
  mockedInvoke.mockResolvedValue([]);
});

afterEach(() => {
  vi.useRealTimers();
  vi.clearAllMocks();
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

  it("starts polling get_spectrum_data on mount", async () => {
    renderInProvider("node-poll");

    await act(async () => {
      vi.advanceTimersByTime(33);
    });

    expect(mockedInvoke).toHaveBeenCalledWith("get_spectrum_data", { nodeId: "node-poll" });
  });

  it("polls at ~30fps (multiple intervals)", async () => {
    renderInProvider("node-multi");

    await act(async () => {
      vi.advanceTimersByTime(100);
    });

    expect(mockedInvoke.mock.calls.length).toBeGreaterThanOrEqual(3);
  });

  it("clears the polling interval on unmount", async () => {
    const { unmount } = renderInProvider("node-unmount");

    await act(async () => {
      vi.advanceTimersByTime(33);
    });
    const callsBefore = mockedInvoke.mock.calls.length;

    unmount();

    await act(async () => {
      vi.advanceTimersByTime(200);
    });

    expect(mockedInvoke.mock.calls.length).toBe(callsBefore);
  });
});
