import { Handle, Position, useNodeId } from "@xyflow/react";
import { useState } from "react";

import { parseAudioEdgeType } from "@/lib/utils";
import { useAppStore } from "@/state";

// Keep these in sync with components/AudioEdge.tsx.
const HUE_BY_RATE: Record<number, string> = {
  44100: "#22c1c3",
  48000: "#3fb950",
  88200: "#3fb950",
  96000: "#d29922",
  176400: "#d29922",
  192000: "#f85149",
};

const RATE_LABEL: Record<number, string> = {
  44100: "44.1k",
  48000: "48k",
  88200: "88.2k",
  96000: "96k",
  176400: "176.4k",
  192000: "192k",
};

function hueForRate(rate: number | undefined): string {
  if (rate == null) return "#8b949e";
  return HUE_BY_RATE[rate] ?? "#8b949e";
}

function rateLabel(rate: number | undefined): string {
  if (rate == null) return "?";
  if (RATE_LABEL[rate]) return RATE_LABEL[rate];
  if (rate >= 1000) return `${Math.round(rate / 100) / 10}k`;
  return String(rate);
}

interface AudioHandleProps {
  type: "source" | "target";
  position: Position;
  id?: string;
  /** Optional inline override; otherwise the owning node's `data.edgeType` is used. */
  edgeType?: string | null;
  /** Extra classes (e.g. `!static !transform-none` for in-flow handles). */
  className?: string;
  /** Extra inline styles, merged on top of the computed handle style. */
  style?: React.CSSProperties;
}

/**
 * Audio-aware connection handle.
 *
 * - Renders a row of small colored dots equal to the channel count (1-4).
 *   For >=5 channels the dots collapse into a single ring with a numeric badge.
 * - The dot/ring color is the sample-rate hue (same palette as AudioEdge).
 * - When the handle is not currently connected, hovering reveals a tooltip
 *   with the exact `Nch . rate . bits` triple.
 */
export function AudioHandle(props: AudioHandleProps) {
  const nodeId = useNodeId();

  const edgeTypeFromStore = useAppStore((s) => {
    if (!nodeId) return null;
    const node = s.nodes.find((n) => n.id === nodeId);
    return node?.data && "edgeType" in node.data
      ? ((node.data as { edgeType?: string | null }).edgeType ?? null)
      : null;
  });

  const isConnected = useAppStore((s) => {
    if (!nodeId) return false;
    return s.edges.some((e) =>
      props.type === "source"
        ? e.source === nodeId && (e.sourceHandle ?? null) === (props.id ?? null)
        : e.target === nodeId && (e.targetHandle ?? null) === (props.id ?? null),
    );
  });

  const edgeType = props.edgeType !== undefined ? props.edgeType : edgeTypeFromStore;
  const fmt = parseAudioEdgeType(edgeType);

  const channels = fmt?.channels;
  const sampleRate = fmt?.frequency;
  const bits = fmt?.bitsPerSample;
  const color = hueForRate(sampleRate);

  const [hovered, setHovered] = useState(false);
  const showTooltip = hovered && !isConnected;

  const dotCount = channels ?? 0;
  const showRing = dotCount === 0 || dotCount >= 5;

  // Tooltip placement: opposite the node body so it doesn't overlap node UI.
  const tooltipStyle: React.CSSProperties = (() => {
    switch (props.position) {
      case Position.Left:
        return { right: "calc(100% + 8px)", top: "50%", transform: "translateY(-50%)" };
      case Position.Right:
        return { left: "calc(100% + 8px)", top: "50%", transform: "translateY(-50%)" };
      case Position.Top:
        return { bottom: "calc(100% + 8px)", left: "50%", transform: "translateX(-50%)" };
      case Position.Bottom:
        return { top: "calc(100% + 8px)", left: "50%", transform: "translateX(-50%)" };
      default:
        return { left: "calc(100% + 8px)", top: "50%", transform: "translateY(-50%)" };
    }
  })();

  return (
    <Handle
      type={props.type}
      position={props.position}
      id={props.id}
      className={props.className}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      style={{
        width: 18,
        height: 14,
        borderRadius: 7,
        background: "#0e1116",
        border: `1.5px solid ${color}`,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        gap: 2,
        padding: "0 2px",
        ...props.style,
      }}
    >
      {showRing ? (
        <div
          style={{
            width: 8,
            height: 8,
            borderRadius: "50%",
            border: `1.5px solid ${color}`,
            background: "transparent",
            fontSize: 7,
            color: color,
            lineHeight: 1,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            fontFamily: "ui-monospace, monospace",
            fontWeight: 700,
            pointerEvents: "none",
          }}
        >
          {dotCount >= 5 ? dotCount : ""}
        </div>
      ) : (
        Array.from({ length: dotCount }, (_, i) => (
          <span
            key={i}
            style={{
              width: dotCount <= 2 ? 4 : 3,
              height: dotCount <= 2 ? 4 : 3,
              borderRadius: "50%",
              background: color,
              flex: "0 0 auto",
              pointerEvents: "none",
            }}
          />
        ))
      )}

      {showTooltip && (
        <div
          style={{
            position: "absolute",
            ...tooltipStyle,
            background: "#0e1116",
            border: `1px solid ${color}`,
            color,
            padding: "3px 8px",
            borderRadius: 9999,
            fontSize: 10,
            fontWeight: 600,
            fontFamily: "ui-monospace, monospace",
            lineHeight: 1.2,
            whiteSpace: "nowrap",
            pointerEvents: "none",
            zIndex: 10,
          }}
        >
          {fmt
            ? `${channels}ch · ${rateLabel(sampleRate)} · ${(bits ?? 0) >= 32 ? "32f" : bits}`
            : "no format"}
        </div>
      )}
    </Handle>
  );
}
