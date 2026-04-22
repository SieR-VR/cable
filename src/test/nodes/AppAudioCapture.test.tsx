import { render, screen } from "@testing-library/react";
import { ReactFlowProvider } from "@xyflow/react";
import { describe, it, expect, beforeEach, vi } from "vitest";

import AppAudioCapture from "@/nodes/AppAudioCapture";
import { WindowInfo } from "@/types";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";

const mockWindows: WindowInfo[] = [
  { processId: 1234, title: "Visual Studio Code" },
  { processId: 5678, title: "Spotify" },
];

function makeProps(id = "node-1", data: object = {}) {
  return {
    id,
    type: "appAudioCapture" as const,
    data: { processId: null, windowTitle: null, edgeType: null, ...data },
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
  } as Parameters<typeof AppAudioCapture>[0];
}

function renderInProvider(id?: string, data?: object) {
  const props = makeProps(id, data);
  return render(
    <ReactFlowProvider>
      <AppAudioCapture {...props} />
    </ReactFlowProvider>,
  );
}

beforeEach(() => {
  vi.mocked(invoke).mockResolvedValue([]);
});

describe("AppAudioCapture", () => {
  it("renders source handle only (source node)", async () => {
    vi.mocked(invoke).mockResolvedValue(mockWindows);
    renderInProvider();
    expect(document.querySelector('[data-handleid="AppAudioCapture-source"]')).toBeTruthy();
    expect(document.querySelector('[data-handleid="AppAudioCapture-target"]')).toBeFalsy();
  });

  it("renders header label", () => {
    renderInProvider();
    expect(screen.getByText("App Audio Capture")).toBeTruthy();
  });

  it("shows loading state initially", () => {
    vi.mocked(invoke).mockReturnValue(new Promise(() => {}));
    renderInProvider();
    expect(screen.getByText("Loading windows...")).toBeTruthy();
  });

  it("shows window dropdown after invoke resolves", async () => {
    vi.mocked(invoke).mockResolvedValue(mockWindows);
    const { findByText } = renderInProvider();
    expect(await findByText("Visual Studio Code")).toBeTruthy();
    expect(await findByText("Spotify")).toBeTruthy();
  });

  it("shows PID when a window is selected", () => {
    vi.mocked(invoke).mockResolvedValue(mockWindows);
    renderInProvider("node-1", { processId: 1234, windowTitle: "Visual Studio Code" });
    expect(screen.getByText("PID: 1234")).toBeTruthy();
  });
});
