import { ReactFlow, ReactFlowProvider, Background, Node, Edge } from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import type { ReactNode } from "react";
import { useEffect } from "react";

import { AudioEdge } from "@/components/AudioEdge";
import { useAppStore } from "@/state";
import { nodeTypes, NodeType, EdgeType, AudioDevice, VirtualDevice, VstPluginInfo, NodeRenderData } from "@/types";

export const edgeTypes = { audio: AudioEdge } as const;

// ----- Mock fixtures -----------------------------------------------------

export const mockInputDevices: AudioDevice[] = [
  {
    id: "input-default",
    readableName: "Default Microphone",
    descriptions: ["Default", "Microphone (Realtek)"],
    frequency: 48000,
    channels: 2,
    bitsPerSample: 24,
  },
  {
    id: "input-usb",
    readableName: "USB Audio Interface",
    descriptions: ["USB Audio", "Focusrite Scarlett 2i2"],
    frequency: 96000,
    channels: 2,
    bitsPerSample: 32,
  },
];

export const mockOutputDevices: AudioDevice[] = [
  {
    id: "output-speakers",
    readableName: "Speakers",
    descriptions: ["Speakers (Realtek)"],
    frequency: 48000,
    channels: 2,
    bitsPerSample: 24,
  },
  {
    id: "output-headphones",
    readableName: "Headphones",
    descriptions: ["Headphones (USB)"],
    frequency: 44100,
    channels: 2,
    bitsPerSample: 16,
  },
];

export const mockVirtualDevices: VirtualDevice[] = [
  { id: "virt-render-01", name: "Cable Speaker A", deviceType: "render" },
  { id: "virt-render-02", name: "Cable Speaker B", deviceType: "render" },
  { id: "virt-capture-01", name: "Cable Mic A", deviceType: "capture" },
  { id: "virt-capture-02", name: "Cable Mic B", deviceType: "capture" },
];

export const mockVstPlugins: VstPluginInfo[] = [
  {
    name: "Bertom Denoiser Classic",
    path: "C:\\Program Files\\Common Files\\VST3\\Bertom_DenoiserClassic.vst3",
    vendor: "Bertom",
    numInputs: 2,
    numOutputs: 2,
    numParams: 8,
  },
  {
    name: "OrilRiver",
    path: "C:\\Program Files\\Common Files\\VST3\\OrilRiver.vst3",
    vendor: "Denis Tihanov",
    numInputs: 2,
    numOutputs: 2,
    numParams: 16,
  },
];

// ----- Decorators --------------------------------------------------------

export interface SeedOptions {
  nodes?: NodeType[];
  edges?: EdgeType[];
  inputDevices?: AudioDevice[] | null;
  outputDevices?: AudioDevice[] | null;
  virtualDevices?: VirtualDevice[];
  vstPluginList?: VstPluginInfo[];
  driverConnected?: boolean;
  nodeRenderData?: Record<string, NodeRenderData>;
}

export function seedAppStore(opts: SeedOptions = {}): void {
  useAppStore.setState({
    nodes: opts.nodes ?? [],
    edges: opts.edges ?? [],
    availableAudioInputDevices: opts.inputDevices ?? mockInputDevices,
    availableAudioOutputDevices: opts.outputDevices ?? mockOutputDevices,
    virtualDevices: opts.virtualDevices ?? mockVirtualDevices,
    vstPluginList: opts.vstPluginList ?? mockVstPlugins,
    driverConnected: opts.driverConnected ?? true,
    nodeRenderData: opts.nodeRenderData ?? {},
  });
}

interface NodeCanvasProps {
  nodes: Node[];
  edges?: Edge[];
  width?: number | string;
  height?: number | string;
  children?: ReactNode;
  seed?: SeedOptions;
}

/**
 * Renders the given nodes/edges in a real ReactFlow canvas with the project's
 * nodeTypes and edgeTypes. Seeds the zustand store with mock fixtures so
 * components that read from the store work without a live backend.
 */
export function NodeCanvas({
  nodes,
  edges = [],
  width = 720,
  height = 420,
  seed,
}: NodeCanvasProps) {
  useEffect(() => {
    seedAppStore({ ...seed, nodes: nodes as NodeType[], edges: edges as EdgeType[] });
  }, [nodes, edges, seed]);

  return (
    <div style={{ width, height }} className="bg-[#0e1116]">
      <ReactFlowProvider>
        <ReactFlow
          nodes={nodes}
          edges={edges}
          nodeTypes={nodeTypes}
          edgeTypes={edgeTypes}
          fitView
          proOptions={{ hideAttribution: true }}
        >
          <Background color="#30363d" gap={16} />
        </ReactFlow>
      </ReactFlowProvider>
    </div>
  );
}
