import { nodeTypes } from "@/types";
import { useAppStore } from "@/state";

type NodeMenuEntry = {
  type: keyof typeof nodeTypes;
  label: string;
};

type NodeCategory = {
  label: string;
  requiresDriver?: boolean;
  items: NodeMenuEntry[];
};

const NODE_CATEGORIES: NodeCategory[] = [
  {
    label: "Devices",
    items: [
      { type: "audioInputDevice", label: "Audio Input Device" },
      { type: "audioOutputDevice", label: "Audio Output Device" },
      { type: "appAudioCapture", label: "App Audio Capture" },
    ],
  },
  {
    label: "Virtual Devices",
    requiresDriver: true,
    items: [
      { type: "virtualAudioInput", label: "Virtual Mic (Capture)" },
      { type: "virtualAudioOutput", label: "Virtual Speaker (Render)" },
    ],
  },
  {
    label: "Visualizers",
    items: [
      { type: "spectrumAnalyzer", label: "Spectrum Analyzer" },
      { type: "waveformMonitor", label: "Waveform Monitor" },
    ],
  },
];

export function ContextMenu() {
  const {
    contextMenuOpen,
    contextMenuPosition,
    contextMenuTargetNodeId,
    addNodeAtContextMenu,
    removeNodeAtContextMenu,
    setContextMenuOpen,
    driverConnected,
  } = useAppStore();

  if (!contextMenuOpen) {
    return null;
  }

  return (
    <div
      className="fixed min-w-56 bg-white border border-gray-300 shadow-lg rounded-md p-2"
      style={{ top: contextMenuPosition.y, left: contextMenuPosition.x }}
    >
      <div className="px-2 pb-1 text-xs font-semibold text-gray-500">Add Node</div>
      {NODE_CATEGORIES.map((category, i) => {
        const disabled = category.requiresDriver && !driverConnected;
        return (
          <div key={category.label}>
            {i > 0 && <div className="my-2 h-px bg-gray-200" />}
            <div className="px-2 pb-1 text-xs font-semibold text-gray-500">
              {category.label}
              {disabled && <span className="text-yellow-500"> (driver offline)</span>}
            </div>
            {category.items.map((item) => (
              <button
                key={item.type}
                className="w-full text-left px-4 py-2 hover:bg-gray-100 cursor-pointer rounded disabled:opacity-40 disabled:cursor-not-allowed"
                disabled={!!disabled}
                onClick={() => {
                  addNodeAtContextMenu(item.type);
                  setContextMenuOpen(false);
                }}
              >
                {item.label}
              </button>
            ))}
          </div>
        );
      })}
      <div className="my-2 h-px bg-gray-200" />
      <button
        className="w-full text-left px-4 py-2 hover:bg-gray-100 cursor-pointer rounded disabled:opacity-40 disabled:cursor-not-allowed"
        disabled={!contextMenuTargetNodeId}
        onClick={() => {
          removeNodeAtContextMenu();
          setContextMenuOpen(false);
        }}
      >
        Remove Node
      </button>
    </div>
  );
}
