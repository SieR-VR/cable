import { describe, it, expect, beforeEach, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { useAppStore } from "../state";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockedInvoke = vi.mocked(invoke) as unknown as ReturnType<
  typeof vi.fn<(...args: any[]) => any>
>;

function resetStore() {
  // Reset the store to its initial state between tests.
  useAppStore.setState(useAppStore.getInitialState());
}

beforeEach(() => {
  resetStore();
  vi.clearAllMocks();
});

describe("useAppStore — synchronous actions", () => {
  it("has two initial nodes and no edges", () => {
    const { nodes, edges } = useAppStore.getState();
    expect(nodes).toHaveLength(2);
    expect(edges).toHaveLength(0);
    expect(nodes[0].type).toBe("audioInputDevice");
    expect(nodes[1].type).toBe("audioOutputDevice");
  });

  it("setMenuOpen toggles menuOpen flag", () => {
    useAppStore.getState().setMenuOpen(true);
    expect(useAppStore.getState().menuOpen).toBe(true);
    useAppStore.getState().setMenuOpen(false);
    expect(useAppStore.getState().menuOpen).toBe(false);
  });

  it("setContextMenuOpen updates position and target node", () => {
    useAppStore.getState().setContextMenuOpen(true, { x: 100, y: 200 }, { x: 50, y: 75 }, "node-1");
    const state = useAppStore.getState();
    expect(state.contextMenuOpen).toBe(true);
    expect(state.contextMenuPosition).toEqual({ x: 100, y: 200 });
    expect(state.contextMenuFlowPosition).toEqual({ x: 50, y: 75 });
    expect(state.contextMenuTargetNodeId).toBe("node-1");
  });

  it("addNodeAtContextMenu adds an audioInputDevice node", () => {
    useAppStore.setState({ contextMenuFlowPosition: { x: 300, y: 400 } });
    useAppStore.getState().addNodeAtContextMenu("audioInputDevice");
    const { nodes } = useAppStore.getState();
    expect(nodes).toHaveLength(3);
    const newNode = nodes[2];
    expect(newNode.type).toBe("audioInputDevice");
    expect(newNode.position).toEqual({ x: 300, y: 400 });
  });

  it("addNodeAtContextMenu adds a virtualAudioInput node with correct data shape", () => {
    useAppStore.setState({ contextMenuFlowPosition: { x: 0, y: 0 } });
    useAppStore.getState().addNodeAtContextMenu("virtualAudioInput");
    const { nodes } = useAppStore.getState();
    const newNode = nodes[2];
    expect(newNode.type).toBe("virtualAudioInput");
    expect(newNode.data).toEqual({ deviceId: "", name: "", edgeType: null });
  });

  it("addNodeAtContextMenu generates unique IDs when gaps exist", () => {
    // Remove node-2 so there's a gap, then add a new node
    useAppStore.setState({
      nodes: useAppStore.getState().nodes.filter((n) => n.id !== "node-2"),
    });
    useAppStore.getState().addNodeAtContextMenu("audioOutputDevice");
    const { nodes } = useAppStore.getState();
    expect(nodes).toHaveLength(2);
    expect(nodes[1].id).toBe("node-2"); // re-uses the gap
  });

  it("removeNodeAtContextMenu removes the target node and connected edges", () => {
    // Set up: add an edge between node-1 and node-2
    useAppStore.setState({
      edges: [
        {
          id: "edge-1",
          source: "node-1",
          target: "node-2",
        },
      ],
      contextMenuTargetNodeId: "node-1",
    });
    useAppStore.getState().removeNodeAtContextMenu();
    const state = useAppStore.getState();
    expect(state.nodes).toHaveLength(1);
    expect(state.nodes[0].id).toBe("node-2");
    expect(state.edges).toHaveLength(0);
  });

  it("removeNodeAtContextMenu does nothing when target is null", () => {
    useAppStore.setState({ contextMenuTargetNodeId: null });
    useAppStore.getState().removeNodeAtContextMenu();
    expect(useAppStore.getState().nodes).toHaveLength(2);
  });

  it("setSelectedAudioHost updates the host", () => {
    useAppStore.getState().setSelectedAudioHost("WASAPI");
    expect(useAppStore.getState().selectedAudioHost).toBe("WASAPI");
  });

  it("setDriverConnected updates the flag", () => {
    useAppStore.getState().setDriverConnected(true);
    expect(useAppStore.getState().driverConnected).toBe(true);
  });

  it("updateNode merges data for matching node", () => {
    useAppStore.getState().updateNode("node-1", { device: { id: "hw:0" } });
    const node = useAppStore.getState().nodes.find((n) => n.id === "node-1")!;
    expect((node.data as any).device).toEqual({ id: "hw:0" });
  });
});

describe("useAppStore — async virtual device actions", () => {
  it("addVirtualDevice calls invoke and appends to list", async () => {
    const fakeDevice = { id: "abcd", name: "Test", deviceType: "render" };
    mockedInvoke.mockResolvedValueOnce(fakeDevice);

    await useAppStore.getState().addVirtualDevice("Test", "render");

    expect(mockedInvoke).toHaveBeenCalledWith("create_virtual_device", {
      name: "Test",
      deviceType: "render",
    });
    expect(useAppStore.getState().virtualDevices).toEqual([fakeDevice]);
  });

  it("removeVirtualDevice calls invoke and removes from list", async () => {
    useAppStore.setState({
      virtualDevices: [
        { id: "abc", name: "A", deviceType: "render" },
        { id: "def", name: "B", deviceType: "capture" },
      ],
    });
    mockedInvoke.mockResolvedValueOnce(undefined);

    await useAppStore.getState().removeVirtualDevice("abc");

    expect(mockedInvoke).toHaveBeenCalledWith("remove_virtual_device", {
      deviceId: "abc",
    });
    expect(useAppStore.getState().virtualDevices).toEqual([
      { id: "def", name: "B", deviceType: "capture" },
    ]);
  });

  it("renameVirtualDevice calls invoke and updates name in list", async () => {
    useAppStore.setState({
      virtualDevices: [{ id: "abc", name: "Old", deviceType: "render" }],
    });
    mockedInvoke.mockResolvedValueOnce(undefined);

    await useAppStore.getState().renameVirtualDevice("abc", "New");

    expect(mockedInvoke).toHaveBeenCalledWith("rename_virtual_device", {
      deviceId: "abc",
      newName: "New",
    });
    expect(useAppStore.getState().virtualDevices[0].name).toBe("New");
  });

  it("addVirtualDevice propagates errors", async () => {
    mockedInvoke.mockRejectedValueOnce(new Error("Driver failed"));
    await expect(useAppStore.getState().addVirtualDevice("Fail", "render")).rejects.toThrow(
      "Driver failed",
    );
  });
});

describe("useAppStore — onConnect edge type guard", () => {
  it("blocks connections between nodes with mismatched edge types", () => {
    useAppStore.setState({
      nodes: [
        {
          id: "n1",
          type: "audioInputDevice",
          dragHandle: ".drag-handle__custom",
          position: { x: 0, y: 0 },
          data: { device: null, edgeType: "pcm-16-48000" },
        } as any,
        {
          id: "n2",
          type: "audioOutputDevice",
          dragHandle: ".drag-handle__custom",
          position: { x: 100, y: 0 },
          data: { device: null, edgeType: "pcm-24-44100" },
        } as any,
      ],
      edges: [],
    });

    useAppStore.getState().onConnect({
      source: "n1",
      target: "n2",
      sourceHandle: null,
      targetHandle: null,
    });

    // Edge should NOT have been added
    expect(useAppStore.getState().edges).toHaveLength(0);
  });

  it("allows connections when edge types match", () => {
    useAppStore.setState({
      nodes: [
        {
          id: "n1",
          type: "audioInputDevice",
          dragHandle: ".drag-handle__custom",
          position: { x: 0, y: 0 },
          data: { device: null, edgeType: "pcm-16-48000" },
        } as any,
        {
          id: "n2",
          type: "audioOutputDevice",
          dragHandle: ".drag-handle__custom",
          position: { x: 100, y: 0 },
          data: { device: null, edgeType: "pcm-16-48000" },
        } as any,
      ],
      edges: [],
    });

    useAppStore.getState().onConnect({
      source: "n1",
      target: "n2",
      sourceHandle: null,
      targetHandle: null,
    });

    expect(useAppStore.getState().edges).toHaveLength(1);
  });

  it("allows connections when edge types are null", () => {
    // Default nodes have edgeType: null, connections should work
    useAppStore.getState().onConnect({
      source: "node-1",
      target: "node-2",
      sourceHandle: null,
      targetHandle: null,
    });

    expect(useAppStore.getState().edges).toHaveLength(1);
  });
});
