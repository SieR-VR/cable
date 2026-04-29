import { Handle, Position, useNodeId } from "@xyflow/react";
import { useState } from "react";

import { STRAND_OFFSETS } from "@/components/AudioEdge";
import { EdgeType, isCompatible } from "@/graph/edge-type";
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
 *     handle direction. Dot centers are placed on the handle's *outer* edge
 *     (the actual connection point ReactFlow uses for sourceX/targetX) so the
 *     edge starts/ends exactly at a dot.
 *   - Sample rate   -> dot color (hue palette).
 *   - Disabled / no format -> single hollow gray circle.
 *
 * Hover tooltip (only when not connected) shows `Nch . rate . bits`.
 */
/**
 * Pick the EdgeType this handle should visualize:
 *  - For source handles: the validator's `producedOutputs[handleId]`.
 *  - For target handles: the connected edge's actual carried type if any,
 *    otherwise the validator's `expectedInputs[handleId]` (so an
 *    unconnected sink still hints at what it wants to receive).
 *
 * Returns NONE / null when no validator entry exists yet.
 */
function audioFmtFromEdgeType(t: EdgeType | undefined | null) {
  if (!t || t.kind !== "audio") return null;
  return { channels: t.channels, frequency: t.frequency, bitsPerSample: t.bitsPerSample };
}

export function AudioHandle(props: AudioHandleProps) {
  const nodeId = useNodeId();

  // Structured type from the validation engine (preferred).
  const structured = useAppStore((s) => {
    if (!nodeId) return null;
    const v = s.validation[nodeId];
    if (!v) return null;
    if (props.type === "source") {
      return v.producedOutputs[props.id ?? ""] ?? null;
    }
    // For target handles, prefer the actual edge-carried type so visuals
    // reflect what's really flowing in (which may differ from `expected`
    // when the connection is mid-mismatch).
    const e = s.edges.find(
      (edge) => edge.target === nodeId && (edge.targetHandle ?? null) === (props.id ?? null),
    );
    return e?.data?.edgeType ?? v.expectedInputs[props.id ?? ""] ?? null;
  });

  // Mismatch flag for target handles only: actual incoming != expected.
  const mismatched = useAppStore((s) => {
    if (!nodeId || props.type !== "target") return false;
    const v = s.validation[nodeId];
    const expected = v?.expectedInputs[props.id ?? ""];
    const e = s.edges.find(
      (edge) => edge.target === nodeId && (edge.targetHandle ?? null) === (props.id ?? null),
    );
    const actual = e?.data?.edgeType;
    if (!expected || !actual) return false;
    return !isCompatible(actual, expected);
  });

  // Legacy fallback so nodes that still expose data.edgeType keep working
  // until they're migrated.
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

  // Resolution order: explicit prop override > structured validation type >
  // legacy node.data.edgeType string.
  const fmt =
    props.edgeType !== undefined
      ? parseAudioEdgeType(props.edgeType)
      : (audioFmtFromEdgeType(structured) ?? parseAudioEdgeType(edgeTypeFromStore));

  const channels = fmt?.channels;
  const sampleRate = fmt?.frequency;
  const bits = fmt?.bitsPerSample;

  const [hovered, setHovered] = useState(false);
  const showTooltip = hovered && !isConnected;

  const offsets = dotOffsets(channels);
  const disabled = !fmt || offsets.length === 0;
  const color = mismatched ? "#f85149" : disabled ? DISABLED_COLOR : hueForRate(sampleRate);

  // Dot center is at the handle DOM box center, which sits *inside* the node
  // visually. ReactFlow's connection point (sourceX/targetX) is at the handle
  // outer edge, so AudioEdge shifts those points inward by HANDLE_SIZE/2 to
  // make the strand actually start/end at the dot.

  // Strand-spread axis is perpendicular to the handle's outward direction.
  // Left / Right -> dots stack vertically (Y axis offsets).
  // Top  / Bottom -> dots line up horizontally (X axis offsets).
  const isHorizontalHandle = props.position === Position.Left || props.position === Position.Right;

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
        ...props.style,
      }}
    >
      {/*
        Inner positioning context. Required because some call sites apply
        `!static` to the outer Handle (e.g. Mixer's in-flow handles). Without
        this wrapper, absolutely-positioned dots would resolve against an
        ancestor instead of the handle box.
      */}
      <div
        style={{
          position: "relative",
          width: HANDLE_SIZE,
          height: HANDLE_SIZE,
          pointerEvents: "none",
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
              boxSizing: "border-box",
            }}
          />
        ) : (
          <>
            {offsets.length >= 2 &&
              (() => {
                // Gray capsule border tying all dots together so users can
                // recognize a multi-channel handle as one group when several
                // handles sit on the same node.
                const maxOff = Math.max(...offsets.map(Math.abs));
                const PAD = 2.5;
                const along = (maxOff + DOT_SIZE / 2 + PAD) * 2;
                const across = DOT_SIZE + PAD * 2;
                const w = isHorizontalHandle ? across : along;
                const h = isHorizontalHandle ? along : across;
                return (
                  <span
                    style={{
                      position: "absolute",
                      left: "50%",
                      top: "50%",
                      transform: "translate(-50%, -50%)",
                      width: w,
                      height: h,
                      borderRadius: 9999,
                      border: `2px solid ${DISABLED_COLOR}`,
                      boxSizing: "border-box",
                      opacity: 0.6,
                    }}
                  />
                );
              })()}
            {offsets.map((off, i) => {
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
                  }}
                />
              );
            })}
          </>
        )}
      </div>

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
          {fmt ? `${channels}ch · ${rateLabel(sampleRate)} · ${bits}b` : "no format"}
        </div>
      )}
    </Handle>
  );
}
