import { EdgeLabelRenderer, EdgeProps, getBezierPath, Position } from "@xyflow/react";
import { useState, useCallback, useEffect } from "react";

import { formatEdgeType } from "@/graph/edge-type";
import { parseAudioEdgeType } from "@/lib/utils";
import { useAppStore } from "@/state";
import { EdgeType } from "@/types";

// ---------------------------------------------------------------------------
// Visual encoding
// ---------------------------------------------------------------------------

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

function hueForRate(rate: number): string {
  return HUE_BY_RATE[rate] ?? "#8b949e";
}

function rateLabel(rate: number): string {
  if (RATE_LABEL[rate]) return RATE_LABEL[rate];
  if (rate >= 1000) return `${Math.round(rate / 100) / 10}k`;
  return String(rate);
}

interface BitStyle {
  dash: string | undefined;
  opacity: number;
  width: number;
  halo: boolean;
}

function bitStyle(bits: number): BitStyle {
  if (bits <= 16) return { dash: "3 2", opacity: 0.55, width: 1.8, halo: false };
  if (bits <= 24) return { dash: undefined, opacity: 1.0, width: 2.2, halo: false };
  return { dash: undefined, opacity: 1.0, width: 2.4, halo: true };
}

/**
 * Returns perpendicular offsets for parallel strands (centered around 0).
 * Capped at 4 strands — the same offsets are used by AudioHandle so that
 * edge endpoints align with the dot centers on the connected handles.
 */
export const STRAND_OFFSETS: Record<number, number[]> = {
  1: [0],
  2: [-3, 3],
  3: [-5, 0, 5],
  4: [-6, -2, 2, 6],
};

function strandOffsets(channels: number): number[] {
  const c = Math.min(Math.max(channels, 1), 4);
  return STRAND_OFFSETS[c];
}

/**
 * Returns the perpendicular offset (dx, dy) to apply to an endpoint at the
 * given handle position so that parallel strands stay evenly spaced regardless
 * of overall edge direction.
 *
 * The strand "spread" axis is perpendicular to the handle's outward direction:
 *   - Left / Right handles  -> spread vertically (offset y)
 *   - Top / Bottom handles  -> spread horizontally (offset x)
 *
 * Without this, vertically-arranged nodes (Top/Bottom handles) would have
 * strands offset along the same axis as the edge curve, causing them to
 * collapse onto each other near the midpoint.
 */
function perpOffset(pos: Position, d: number): { dx: number; dy: number } {
  switch (pos) {
    case Position.Left:
    case Position.Right:
      return { dx: 0, dy: d };
    case Position.Top:
    case Position.Bottom:
      return { dx: d, dy: 0 };
    default:
      return { dx: 0, dy: d };
  }
}

/** Parse "M sx,sy C cx1,cy1 cx2,cy2 tx,ty" into the four control points. */
function parseBezier(path: string): {
  sx: number;
  sy: number;
  cx1: number;
  cy1: number;
  cx2: number;
  cy2: number;
  tx: number;
  ty: number;
} | null {
  const m =
    /M\s*([-\d.]+)[, ]([-\d.]+)\s*C\s*([-\d.]+)[, ]([-\d.]+)\s+([-\d.]+)[, ]([-\d.]+)\s+([-\d.]+)[, ]([-\d.]+)/.exec(
      path,
    );
  if (!m) return null;
  const [, sx, sy, cx1, cy1, cx2, cy2, tx, ty] = m;
  return {
    sx: +sx,
    sy: +sy,
    cx1: +cx1,
    cy1: +cy1,
    cx2: +cx2,
    cy2: +cy2,
    tx: +tx,
    ty: +ty,
  };
}

/**
 * Build a polyline path that follows the cubic bezier defined by
 * (sx, sy) - (cx1, cy1) - (cx2, cy2) - (tx, ty), but displaced by `offset`
 * pixels along the curve's normal at each sample point. This produces strands
 * that stay evenly spaced even when the edge curves sharply (e.g. when
 * vertically-arranged nodes connect via Left/Right handles).
 */
function offsetBezierPolyline(
  ctrl: {
    sx: number;
    sy: number;
    cx1: number;
    cy1: number;
    cx2: number;
    cy2: number;
    tx: number;
    ty: number;
  },
  offset: number,
  samples = 48,
): string {
  const { sx, sy, cx1, cy1, cx2, cy2, tx, ty } = ctrl;
  let d = "";
  for (let i = 0; i <= samples; i++) {
    const t = i / samples;
    const u = 1 - t;
    const x = u * u * u * sx + 3 * u * u * t * cx1 + 3 * u * t * t * cx2 + t * t * t * tx;
    const y = u * u * u * sy + 3 * u * u * t * cy1 + 3 * u * t * t * cy2 + t * t * t * ty;
    const dx = 3 * u * u * (cx1 - sx) + 6 * u * t * (cx2 - cx1) + 3 * t * t * (tx - cx2);
    const dy = 3 * u * u * (cy1 - sy) + 6 * u * t * (cy2 - cy1) + 3 * t * t * (ty - cy2);
    const len = Math.hypot(dx, dy) || 1;
    const nx = -dy / len;
    const ny = dx / len;
    const px = x + nx * offset;
    const py = y + ny * offset;
    d += i === 0 ? `M ${px},${py}` : ` L ${px},${py}`;
  }
  return d;
}

/**
 * Inset (in graph coords) by which we pull each endpoint inward toward the
 * node so the strand visually starts/ends at the dot center inside the
 * AudioHandle, rather than at the handle's outer edge that ReactFlow uses
 * as the raw connection point. Half of AudioHandle.HANDLE_SIZE.
 */
const HANDLE_HALF = 9;

/** Offset in the direction *into* the node, given the handle position. */
function inwardOffset(pos: Position): { dx: number; dy: number } {
  switch (pos) {
    case Position.Left:
      return { dx: HANDLE_HALF, dy: 0 };
    case Position.Right:
      return { dx: -HANDLE_HALF, dy: 0 };
    case Position.Top:
      return { dx: 0, dy: HANDLE_HALF };
    case Position.Bottom:
      return { dx: 0, dy: -HANDLE_HALF };
    default:
      return { dx: 0, dy: 0 };
  }
}

function bezierAtOffset(
  sourceX: number,
  sourceY: number,
  targetX: number,
  targetY: number,
  sourcePosition: Position,
  targetPosition: Position,
  d: number,
): string {
  const sIn = inwardOffset(sourcePosition);
  const tIn = inwardOffset(targetPosition);
  const sx = sourceX + sIn.dx;
  const sy = sourceY + sIn.dy;
  const tx = targetX + tIn.dx;
  const ty = targetY + tIn.dy;

  const [path] = getBezierPath({
    sourceX: sx,
    sourceY: sy,
    targetX: tx,
    targetY: ty,
    sourcePosition,
    targetPosition,
  });
  if (d === 0) return path;
  const ctrl = parseBezier(path);
  if (!ctrl) {
    // Fallback to endpoint translation if parsing fails.
    const so = perpOffset(sourcePosition, d);
    const to = perpOffset(targetPosition, d);
    const [translated] = getBezierPath({
      sourceX: sx + so.dx,
      sourceY: sy + so.dy,
      targetX: tx + to.dx,
      targetY: ty + to.dy,
      sourcePosition,
      targetPosition,
    });
    return translated;
  }
  return offsetBezierPolyline(ctrl, d);
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

const FALLBACK_FORMAT = { frequency: 48000, channels: 2, bitsPerSample: 24 };

export function AudioEdge(props: EdgeProps<EdgeType>) {
  const {
    id,
    sourceX,
    sourceY,
    targetX,
    targetY,
    sourcePosition,
    targetPosition,
    source,
    selected,
    data,
  } = props;

  const onEdgesChange = useAppStore((s) => s.onEdgesChange);

  const sourceEdgeType = useAppStore((s) => {
    const node = s.nodes.find((n) => n.id === source);
    return node?.data && "edgeType" in node.data
      ? (node.data.edgeType as string | null | undefined)
      : null;
  });

  // Prefer the structured EdgeType carried by the edge itself (set by the
  // validator); fall back to the legacy source-node string field, then to a
  // hard-coded default.
  const structuredType = data?.edgeType;
  const fmt =
    structuredType && structuredType.kind === "audio"
      ? {
          frequency: structuredType.frequency,
          channels: structuredType.channels,
          bitsPerSample: structuredType.bitsPerSample,
        }
      : structuredType && structuredType.kind === "frequency"
        ? {
            frequency: structuredType.frequency,
            channels: structuredType.channels,
            bitsPerSample: 24,
          }
        : (parseAudioEdgeType(sourceEdgeType) ??
          (data?.frequency && data?.channels && data?.bitsPerSample
            ? {
                frequency: data.frequency,
                channels: data.channels,
                bitsPerSample: data.bitsPerSample,
              }
            : FALLBACK_FORMAT));

  const isFrequencyKind = structuredType?.kind === "frequency";
  const isInvalid = !!data?.invalid;
  const isNone = structuredType?.kind === "none";

  const channels = fmt.channels;
  const sampleRate = fmt.frequency;
  const bits = fmt.bitsPerSample;

  const stroke = isInvalid ? "#f85149" : isNone ? "#8b949e" : hueForRate(sampleRate);
  const bs = bitStyle(bits);
  const offsets = strandOffsets(channels);

  const [hovered, setHovered] = useState(false);
  const showChip = hovered || !!selected;

  // Edge context menu (right-click to delete).
  const [edgeMenu, setEdgeMenu] = useState<{ x: number; y: number } | null>(null);
  const onContextMenu = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setEdgeMenu({ x: e.clientX, y: e.clientY });
  }, []);
  // Close menu on any click/scroll outside.
  useEffect(() => {
    if (!edgeMenu) return;
    const handler = () => setEdgeMenu(null);
    window.addEventListener("click", handler, { once: true });
    window.addEventListener("scroll", handler, { once: true, capture: true });
    return () => {
      window.removeEventListener("click", handler);
    };
  }, [edgeMenu]);

  const deleteEdge = useCallback(() => {
    onEdgesChange([{ id, type: "remove" }]);
    setEdgeMenu(null);
  }, [id, onEdgesChange]);

  const sp = sourcePosition as Position;
  const tp = targetPosition as Position;

  const [, labelX, labelY] = getBezierPath({
    sourceX,
    sourceY,
    targetX,
    targetY,
    sourcePosition: sp,
    targetPosition: tp,
  });

  const strandPaths: { d: string; key: string }[] = [];
  for (let i = 0; i < offsets.length; i++) {
    const d = bezierAtOffset(sourceX, sourceY, targetX, targetY, sp, tp, offsets[i]);
    strandPaths.push({ d, key: `s-${i}` });
  }

  const hitPath = bezierAtOffset(sourceX, sourceY, targetX, targetY, sp, tp, 0);

  const chipLabel = structuredType
    ? formatEdgeType(structuredType)
    : `${channels}ch · ${rateLabel(sampleRate)} · ${bits}b`;
  const showInlineCountBadge = channels > 4 && !showChip;

  return (
    <>
      {bs.halo &&
        strandPaths.map((p) => (
          <path
            key={`halo-${p.key}`}
            d={p.d}
            fill="none"
            stroke={stroke}
            strokeWidth={bs.width + 2}
            strokeOpacity={0.18}
            pointerEvents="none"
          />
        ))}

      {strandPaths.map((p) => (
        <path
          key={p.key}
          d={p.d}
          fill="none"
          stroke={stroke}
          strokeWidth={bs.width}
          strokeOpacity={isInvalid ? 1 : bs.opacity}
          strokeDasharray={isInvalid ? "6 4" : isFrequencyKind ? "2 3" : bs.dash}
          pointerEvents="none"
        />
      ))}

      <path
        id={id}
        d={hitPath}
        fill="none"
        stroke="transparent"
        strokeWidth={20}
        onMouseEnter={() => setHovered(true)}
        onMouseLeave={() => setHovered(false)}
        onContextMenu={onContextMenu}
        style={{ cursor: "pointer" }}
      >
        {isInvalid && <title>Type mismatch: {chipLabel}</title>}
      </path>

      <EdgeLabelRenderer>
        {showInlineCountBadge && (
          <div
            style={{
              position: "absolute",
              transform: `translate(-50%, -50%) translate(${labelX}px, ${labelY}px)`,
              pointerEvents: "none",
            }}
          >
            <div
              style={{
                background: "#0e1116",
                border: `1px solid ${stroke}`,
                color: stroke,
                padding: "2px 8px",
                borderRadius: 9999,
                fontSize: 11,
                fontWeight: 600,
                fontFamily: "ui-monospace, monospace",
                lineHeight: 1.2,
              }}
            >
              {channels}ch
            </div>
          </div>
        )}

        {showChip && (
          <div
            style={{
              position: "absolute",
              transform: `translate(-50%, -50%) translate(${labelX}px, ${labelY - 18}px)`,
              pointerEvents: "none",
            }}
            onMouseEnter={() => setHovered(true)}
            onMouseLeave={() => setHovered(false)}
          >
            <div
              style={{
                background: "#0e1116",
                border: `1px solid ${stroke}`,
                color: stroke,
                padding: "3px 10px",
                borderRadius: 9999,
                fontSize: 11,
                fontWeight: 600,
                fontFamily: "ui-monospace, monospace",
                lineHeight: 1.2,
                whiteSpace: "nowrap",
              }}
            >
              {chipLabel}
            </div>
          </div>
        )}

        {edgeMenu && (
          <div
            style={{
              position: "fixed",
              top: edgeMenu.y,
              left: edgeMenu.x,
              zIndex: 1000,
            }}
            onClick={(e) => e.stopPropagation()}
          >
            <div className="min-w-36 bg-white border border-gray-200 shadow-lg rounded-md py-1">
              <button
                className="w-full text-left px-3 py-2 text-sm text-red-600 hover:bg-gray-100 cursor-pointer"
                onClick={deleteEdge}
              >
                Delete Edge
              </button>
            </div>
          </div>
        )}
      </EdgeLabelRenderer>
    </>
  );
}

export const edgeTypes = {
  audio: AudioEdge,
};
