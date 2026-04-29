import type { CSSProperties, ReactNode } from "react";

/**
 * Shared chrome for every node card.
 *
 * Visual: subtle gradient header in the kind's accent color + glowing status
 * dot + title. Dark card body with thin border. Format/behavior pills are
 * intentionally not rendered here — handles + edges already encode that.
 *
 * Note: the outer wrapper has *no* `position: relative`, so absolutely
 * positioned children (in particular `<AudioHandle/>` from `@xyflow/react`)
 * resolve against ReactFlow's NodeWrapper exactly as before. Don't add
 * `relative` here.
 */

interface NodeShellProps {
  /** Hex color identifying the node kind. Used for the header gradient + dot. */
  accent: string;
  title: string;
  /** Min width of the card. Defaults to a sensible value. */
  minWidth?: number | string;
  /** Custom outer className additions (rare). */
  className?: string;
  /** Extra inline styles on the outer wrapper. */
  style?: CSSProperties;
  /** Render with a red border + warning glow when the node failed validation. */
  invalid?: boolean;
  children?: ReactNode;
}

export function NodeShell({
  accent,
  title,
  minWidth = "11rem",
  className = "",
  style,
  invalid = false,
  children,
}: NodeShellProps) {
  const headerBg = `linear-gradient(135deg, ${accent}33, transparent)`;
  const borderColor = invalid ? "#f85149" : undefined;
  const boxShadow = invalid ? "0 0 0 1px #f85149aa, 0 0 12px #f8514955" : undefined;
  return (
    <div
      className={`rounded-lg flex flex-col text-white shadow-md border ${invalid ? "" : "border-gray-700"} bg-gray-800 ${className}`}
      style={{ minWidth, ...(borderColor ? { borderColor, boxShadow } : {}), ...style }}
      title={invalid ? "Type validation failed for this node" : undefined}
    >
      <div
        className="flex items-center gap-2 px-2 py-1.5 border-b border-gray-700 rounded-t-lg drag-handle__custom"
        style={{ background: headerBg }}
      >
        <span
          className="w-2 h-2 rounded-full flex-shrink-0"
          style={{ background: accent, boxShadow: `0 0 6px ${accent}` }}
        />
        <span className="text-xs font-semibold text-gray-100 truncate">{title}</span>
      </div>
      <div className="flex flex-col gap-2 p-2">{children}</div>
    </div>
  );
}

/** Accent palette per node kind. Hex versions of the previous Tailwind colors. */
export const NODE_ACCENTS = {
  audioInputDevice: "#f87171", // red-400
  audioOutputDevice: "#f87171", // red-400
  mixer: "#f97316", // orange-500
  appAudioCapture: "#f97316", // orange-500
  virtualAudioInput: "#a855f7", // purple-500
  spectrumAnalyzer: "#a855f7", // purple-500
  virtualAudioOutput: "#14b8a6", // teal-500
  waveformMonitor: "#10b981", // emerald-500
  vst: "#8b5cf6", // violet-500
  gain: "#f59e0b", // amber-400
  channelSplit: "#06b6d4", // cyan-500
  channelMerge: "#0891b2", // cyan-600
  delay: "#38bdf8", // sky-400
  compressor: "#fb7185", // rose-400
  reverb: "#22d3ee", // cyan-400
  echo: "#a78bfa", // violet-400
} as const;
