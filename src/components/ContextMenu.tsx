import { ChevronRight } from "lucide-react";
import { useState } from "react";

import { useAppStore } from "@/state";
import { NodeType } from "@/types";

type NodeMenuEntry = {
  type: NodeType["type"];
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
    ],
  },
  {
    label: "Sources",
    items: [{ type: "appAudioCapture", label: "App Audio Capture" }],
  },
  {
    label: "Visualizers",
    items: [
      { type: "spectrumAnalyzer", label: "Spectrum Analyzer" },
      { type: "waveformMonitor", label: "Waveform Monitor" },
    ],
  },
  {
    label: "Processing",
    items: [{ type: "mixer", label: "Mixer" }],
  },
  {
    label: "Virtual Devices",
    requiresDriver: true,
    items: [
      { type: "virtualAudioInput", label: "Virtual Mic (Capture)" },
      { type: "virtualAudioOutput", label: "Virtual Speaker (Render)" },
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

  const [hoveredCategory, setHoveredCategory] = useState<number | null>(null);

  if (!contextMenuOpen) {
    return null;
  }

  return (
    <div
      className="fixed min-w-48 bg-white border border-gray-200 shadow-lg rounded-md py-1"
      style={{ top: contextMenuPosition.y, left: contextMenuPosition.x }}
    >
      <div className="px-3 py-1 text-xs font-semibold text-gray-400 tracking-wide">Add Node</div>

      {NODE_CATEGORIES.map((category, i) => {
        const disabled = category.requiresDriver && !driverConnected;
        const isHovered = hoveredCategory === i;

        return (
          <div
            key={category.label}
            className="relative"
            onMouseEnter={() => setHoveredCategory(i)}
            onMouseLeave={() => setHoveredCategory(null)}
          >
            <div
              className={[
                "flex items-center justify-between px-3 py-2 text-sm rounded mx-1 select-none",
                disabled ? "text-gray-400 cursor-not-allowed" : "cursor-pointer hover:bg-gray-100",
                isHovered && !disabled ? "bg-gray-100" : "",
              ].join(" ")}
            >
              <span>{category.label}</span>
              <div className="flex items-center gap-1">
                {disabled && <span className="text-xs text-yellow-500">offline</span>}
                <ChevronRight className="w-3.5 h-3.5 text-gray-400" />
              </div>
            </div>

            {isHovered && !disabled && (
              <div className="absolute left-full top-0 min-w-44 bg-white border border-gray-200 shadow-lg rounded-md py-1 -mt-px ml-px z-10">
                {category.items.map((item) => (
                  <button
                    key={item.type}
                    className="w-full text-left px-3 py-2 text-sm hover:bg-gray-100 cursor-pointer rounded mx-0"
                    onClick={() => {
                      addNodeAtContextMenu(item.type);
                      setContextMenuOpen(false);
                    }}
                  >
                    {item.label}
                  </button>
                ))}
              </div>
            )}
          </div>
        );
      })}

      {contextMenuTargetNodeId && (
        <>
          <div className="my-1 h-px bg-gray-200" />
          <button
            className="w-full text-left px-3 py-2 text-sm hover:bg-gray-100 cursor-pointer rounded mx-0"
            onClick={() => {
              removeNodeAtContextMenu();
              setContextMenuOpen(false);
            }}
          >
            Remove Node
          </button>
        </>
      )}
    </div>
  );
}
