import {
  EdgeLabelRenderer,
  EdgeProps,
  getBezierPath,
  Position,
} from "@xyflow/react";
import { useState } from "react";

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
 * Returns vertical offsets for parallel strands (centered around 0).
 * Returns null for >=6 channels — render bundled (2 strands + count badge).
 */
function strandOffsets(channels: number): number[] | null {
  if (channels <= 1) return [0];
  if (channels === 2) return [-3, 3];
  if (channels === 3) return [-5, 0, 5];
  if (channels === 4) return [-6, -2, 2, 6];
  if (channels === 5) return [-8, -4, 0, 4, 8];
  return null;
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

  const sourceEdgeType = useAppStore((s) => {
    const node = s.nodes.find((n) => n.id === source);
    return node?.data && "edgeType" in node.data
      ? (node.data.edgeType as string | null | undefined)
      : null;
  });

  const fmt =
    parseAudioEdgeType(sourceEdgeType) ??
    (data?.frequency && data?.channels && data?.bitsPerSample
      ? {
          frequency: data.frequency,
          channels: data.channels,
          bitsPerSample: data.bitsPerSample,
        }
      : FALLBACK_FORMAT);

  const channels = fmt.channels;
  const sampleRate = fmt.frequency;
  const bits = fmt.bitsPerSample;

  const stroke = hueForRate(sampleRate);
  const bs = bitStyle(bits);
  const offsets = strandOffsets(channels);

  const [hovered, setHovered] = useState(false);
  const showChip = hovered || !!selected;

  const [, labelX, labelY] = getBezierPath({
    sourceX,
    sourceY,
    targetX,
    targetY,
    sourcePosition: sourcePosition as Position,
    targetPosition: targetPosition as Position,
  });

  const strandPaths: { d: string; key: string }[] = [];
  if (offsets) {
    for (let i = 0; i < offsets.length; i++) {
      const dy = offsets[i];
      const [d] = getBezierPath({
        sourceX,
        sourceY: sourceY + dy,
        targetX,
        targetY: targetY + dy,
        sourcePosition: sourcePosition as Position,
        targetPosition: targetPosition as Position,
      });
      strandPaths.push({ d, key: `s-${i}` });
    }
  } else {
    for (let i = 0; i < 2; i++) {
      const dy = i === 0 ? -4 : 4;
      const [d] = getBezierPath({
        sourceX,
        sourceY: sourceY + dy,
        targetX,
        targetY: targetY + dy,
        sourcePosition: sourcePosition as Position,
        targetPosition: targetPosition as Position,
      });
      strandPaths.push({ d, key: `b-${i}` });
    }
  }

  const [hitPath] = getBezierPath({
    sourceX,
    sourceY,
    targetX,
    targetY,
    sourcePosition: sourcePosition as Position,
    targetPosition: targetPosition as Position,
  });

  const chipLabel = `${channels}ch · ${rateLabel(sampleRate)} · ${bits >= 32 ? "32f" : bits}`;
  const showInlineCountBadge = !offsets && !showChip;

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
          strokeWidth={offsets ? bs.width : bs.width + 0.6}
          strokeOpacity={bs.opacity}
          strokeDasharray={bs.dash}
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
        style={{ cursor: "pointer" }}
      />

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
      </EdgeLabelRenderer>
    </>
  );
}

export const edgeTypes = {
  audio: AudioEdge,
};
