import { render, screen } from "@testing-library/react";
import { ReactFlowProvider } from "@xyflow/react";
import { describe, it, expect, beforeEach } from "vitest";

import AppAudioCapture from "@/nodes/AppAudioCapture";
import { useAppStore } from "@/state";
import { AudioDevice } from "@/types";

const mockDevice: AudioDevice = {
  id: "device-1",
  readableName: "Speakers (Test Device)",
  frequency: 48000,
  channels: 2,
  bitsPerSample: 32,
  descriptions: ["Speakers (Test Device)"],
};

function makeProps(id = "node-1") {
  return {
    id,
    type: "appAudioCapture" as const,
    data: { device: null, edgeType: null },
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

function renderInProvider(id?: string) {
  const props = makeProps(id);
  return render(
    <ReactFlowProvider>
      <AppAudioCapture {...props} />
    </ReactFlowProvider>,
  );
}

beforeEach(() => {
  useAppStore.setState({ availableAudioOutputDevices: null });
});

describe("AppAudioCapture", () => {
  it("renders source handle only (source node)", () => {
    renderInProvider();
    expect(document.querySelector('[data-handleid="AppAudioCapture-source"]')).toBeTruthy();
    expect(document.querySelector('[data-handleid="AppAudioCapture-target"]')).toBeFalsy();
  });

  it("renders header label", () => {
    renderInProvider();
    expect(screen.getByText("App Audio Capture")).toBeTruthy();
  });

  it("shows loading state when devices are not yet loaded", () => {
    useAppStore.setState({ availableAudioOutputDevices: null });
    renderInProvider();
    expect(screen.getByText("Loading devices...")).toBeTruthy();
  });

  it("shows dropdown when devices are available", () => {
    useAppStore.setState({ availableAudioOutputDevices: [mockDevice] });
    renderInProvider();
    const options = document.querySelectorAll("option");
    expect(options.length).toBeGreaterThan(0);
    expect(screen.getByText("Speakers (Test Device)")).toBeTruthy();
  });

  it("shows format badges when a device is selected", () => {
    const props = makeProps("node-1");
    props.data = { device: mockDevice, edgeType: "48000Hz/2ch/32bit" };
    render(
      <ReactFlowProvider>
        <AppAudioCapture {...props} />
      </ReactFlowProvider>,
    );
    expect(screen.getByText("48000Hz")).toBeTruthy();
    expect(screen.getByText("2ch")).toBeTruthy();
    expect(screen.getByText("32bit")).toBeTruthy();
  });
});
