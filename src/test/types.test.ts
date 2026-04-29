import { describe, expect, it } from "vitest";

import { audioType, frequencyType, NONE } from "@/graph/edge-type";
import { EdgeType, serializeEdge } from "@/types";

function makeEdge(partial?: Partial<EdgeType["data"]>): EdgeType {
  return {
    id: "e1",
    source: "n1",
    target: "n2",
    sourceHandle: "out",
    targetHandle: "in",
    type: "audio",
    data: partial,
  } as EdgeType;
}

describe("serializeEdge", () => {
  it("derives flat audio fields from a structured audio EdgeType", () => {
    const out = serializeEdge(makeEdge({ edgeType: audioType(2, 48000, 24) }));
    expect(out.frequency).toBe(48000);
    expect(out.channels).toBe(2);
    expect(out.bitsPerSample).toBe(24);
    expect(out.edgeType).toEqual(audioType(2, 48000, 24));
  });

  it("leaves flat fields undefined for none / frequency types", () => {
    const noneOut = serializeEdge(makeEdge({ edgeType: NONE }));
    expect(noneOut.frequency).toBeUndefined();
    expect(noneOut.channels).toBeUndefined();
    expect(noneOut.bitsPerSample).toBeUndefined();

    const freqOut = serializeEdge(makeEdge({ edgeType: frequencyType(2, 48000, 1024) }));
    expect(freqOut.frequency).toBeUndefined();
    expect(freqOut.channels).toBeUndefined();
    expect(freqOut.bitsPerSample).toBeUndefined();
  });

  it("propagates the invalid flag", () => {
    const out = serializeEdge(makeEdge({ edgeType: audioType(2, 48000, 16), invalid: true }));
    expect(out.invalid).toBe(true);
  });

  it("falls back to legacy flat fields when no edgeType is present", () => {
    const out = serializeEdge(
      makeEdge({ frequency: 44100, channels: 1, bitsPerSample: 16 }),
    );
    expect(out.frequency).toBe(44100);
    expect(out.channels).toBe(1);
    expect(out.bitsPerSample).toBe(16);
    expect(out.edgeType).toBeUndefined();
  });
});
