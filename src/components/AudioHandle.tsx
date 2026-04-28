import { Handle, Position, useNodeId } from "@xyflow/react";
import { useState } from "react";

import { STRAND_OFFSETS } from "@/components/AudioEdge";
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

/** Dot offsets along the strand-spread axis. Capped at 4 to match AudioEdge. */
function dotOffsets(channels: number | undefined): number[] {
  if (!channels || channels < 1) return [];
  const c = Math.min(channels, 4);
  return STRAND_OFFSETS[c];
}

const HANDLE_SIZE = 18;
const DOT_SIZE = 4;
const DISABLED_COLOR = "#6e7681";

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
 * Visual encoding (matches AudioEdge):
 *   - Channel count -> N dots (1..4, capped) arranged perpendicular to the
 *     handle direction. Dot centers align with the corresponding edge strand
 *     offsets so the edge starts/ends exactly at a dot.
 *   - Sample rate   -> dot color (hue palette).
 *   - Disabled / no format -> single hollow gray circle.
 *
 * Hover tooltip (only when not connected) shows `Nch . rate . bits`.
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

  const [hovered, setHovered] = useState(false);
  const showTooltip = hovered && !isConnected;

  const offsets = dotOffsets(channels);
  const disabled = !fmt || offsets.length === 0;
  const color = disabled ? DISABLED_COLOR : hueForRate(sampleRate);

  // Strand-spread axis is perpendicular to the handle's outward direction.
  // Left / Right -> dots stack vertically (Y axis offsets).
  // Top  / Bottom -> dots line up horizontally (X axis offsets).
  const isHorizontalHandle =
    props.position === Position.Left || props.position === Position.Right;

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
        width: HANDLE_SIZE,
        height: HANDLE_SIZE,
        borderRadius: HANDLE_SIZE / 2,
        background: "transparent",
        border: "none",
        position: "relative",
        ...props.style,
      }}
    >
      {disabled ? (
        <span
          style={{
            position: "absolute",
            left: "50%",
            top: "50%",
            transform: "translate(-50%, -50%)",
            width: 9,
            height: 9,
            borderRadius: "50%",
            background: "transparent",
            border: `1.5px solid ${DISABLED_COLOR}`,
            pointerEvents: "none",
            boxSizing: "border-box",
          }}
        />
      ) : (
        offsets.map((off, i) => {
          const transform = isHorizontalHandle
            ? `translate(-50%, calc(-50% + ${off}px))`
            : `translate(calc(-50% + ${off}px), -50%)`;
          return (
            <span
              key={i}
              style={{
                position: "absolute",
                left: "50%",
                top: "50%",
                transform,
                width: DOT_SIZE,
                height: DOT_SIZE,
                borderRadius: "50%",
                background: color,
                pointerEvents: "none",
              }}
            />
          );
        })
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
