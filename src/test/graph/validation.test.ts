import { describe, expect, it } from "vitest";

import { NONE, audioType } from "@/graph/edge-type";
import { runCascade, runFullValidation } from "@/graph/validation";
import audioInputDeviceDef from "@/nodes/AudioInputDevice";
import audioOutputDeviceDef from "@/nodes/AudioOutputDevice";
import gainDef from "@/nodes/Gain";
import mixerDef from "@/nodes/Mixer";

const defs: Record<string, any> = {
  audioInputDevice: audioInputDeviceDef,
  audioOutputDevice: audioOutputDeviceDef,
  gain: gainDef,
  mixer: mixerDef,
};
const getDef = (t: string | undefined) => (t ? defs[t] : undefined);

const device48k2c16b = {
  id: "dev-1",
  readableName: "Mic",
  descriptions: null,
  frequency: 48000,
  channels: 2,
  bitsPerSample: 16,
};

describe("validation engine", () => {
  it("propagates source produced type through a passthrough chain", () => {
    const nodes: any[] = [
      { id: "in", type: "audioInputDevice", position: { x: 0, y: 0 }, data: { device: device48k2c16b } },
      { id: "g", type: "gain", position: { x: 100, y: 0 }, data: { gain: 1 } },
      { id: "out", type: "audioOutputDevice", position: { x: 200, y: 0 }, data: { device: device48k2c16b } },
    ];
    const edges: any[] = [
      { id: "e1", source: "in", sourceHandle: "AudioInputDevice-source", target: "g", targetHandle: "Gain-target", data: { id: "e1", from: "in", to: "g" } },
      { id: "e2", source: "g", sourceHandle: "Gain-source", target: "out", targetHandle: "AudioOutputDevice-target", data: { id: "e2", from: "g", to: "out" } },
    ];
    const out = runFullValidation({ nodes, edges, validation: {}, getDef });

    expect(out.edges[0]!.data!.edgeType).toEqual(audioType(2, 48000, 16));
    expect(out.edges[1]!.data!.edgeType).toEqual(audioType(2, 48000, 16));
    expect(out.edges[0]!.data!.invalid).toBe(false);
    expect(out.edges[1]!.data!.invalid).toBe(false);
  });

  it("flags edge invalid when sink expects different format", () => {
    const otherDev = { ...device48k2c16b, frequency: 44100 };
    const nodes: any[] = [
      { id: "in", type: "audioInputDevice", position: { x: 0, y: 0 }, data: { device: device48k2c16b } },
      { id: "out", type: "audioOutputDevice", position: { x: 200, y: 0 }, data: { device: otherDev } },
    ];
    const edges: any[] = [
      { id: "e", source: "in", sourceHandle: "AudioInputDevice-source", target: "out", targetHandle: "AudioOutputDevice-target", data: { id: "e", from: "in", to: "out" } },
    ];
    const out = runFullValidation({ nodes, edges, validation: {}, getDef });
    expect(out.edges[0]!.data!.invalid).toBe(true);
    expect((out.nodes[1]!.data as any).invalid).toBe(true);
  });

  it("cascade re-runs downstream when source state changes", () => {
    const nodes: any[] = [
      { id: "in", type: "audioInputDevice", position: { x: 0, y: 0 }, data: { device: device48k2c16b } },
      { id: "out", type: "audioOutputDevice", position: { x: 200, y: 0 }, data: { device: device48k2c16b } },
    ];
    const edges: any[] = [
      { id: "e", source: "in", sourceHandle: "AudioInputDevice-source", target: "out", targetHandle: "AudioOutputDevice-target", data: { id: "e", from: "in", to: "out" } },
    ];
    const first = runFullValidation({ nodes, edges, validation: {}, getDef });
    expect(first.edges[0]!.data!.edgeType).toEqual(audioType(2, 48000, 16));

    first.nodes[0] = { ...first.nodes[0], data: { device: { ...device48k2c16b, frequency: 96000 } } };
    const second = runCascade({ ...first, getDef }, ["in"]);
    expect(second.edges[0]!.data!.edgeType).toEqual(audioType(2, 96000, 16));
    expect(second.edges[0]!.data!.invalid).toBe(true);
  });

  it("disconnected input keeps the sink valid", () => {
    const nodes: any[] = [
      { id: "out", type: "audioOutputDevice", position: { x: 0, y: 0 }, data: { device: device48k2c16b } },
    ];
    const out = runFullValidation({ nodes, edges: [], validation: {}, getDef });
    expect(out.validation.out.expectedInputs["AudioOutputDevice-target"]).toEqual(audioType(2, 48000, 16));
    expect((out.nodes[0]!.data as any).invalid).toBe(false);
  });

  it("Mixer flags invalid when its two inputs disagree", () => {
    const dev44 = { ...device48k2c16b, frequency: 44100 };
    const nodes: any[] = [
      { id: "a", type: "audioInputDevice", position: { x: 0, y: 0 }, data: { device: device48k2c16b } },
      { id: "b", type: "audioInputDevice", position: { x: 0, y: 50 }, data: { device: dev44 } },
      { id: "m", type: "mixer", position: { x: 100, y: 0 }, data: {} },
    ];
    const edges: any[] = [
      { id: "ea", source: "a", sourceHandle: "AudioInputDevice-source", target: "m", targetHandle: "input-a", data: { id: "ea", from: "a", to: "m" } },
      { id: "eb", source: "b", sourceHandle: "AudioInputDevice-source", target: "m", targetHandle: "input-b", data: { id: "eb", from: "b", to: "m" } },
    ];
    const out = runFullValidation({ nodes, edges, validation: {}, getDef });
    expect((out.nodes[2]!.data as any).invalid).toBe(true);
  });

  it("terminates on cycles", () => {
    const nodes: any[] = [
      { id: "a", type: "gain", position: { x: 0, y: 0 }, data: { gain: 1 } },
      { id: "b", type: "gain", position: { x: 100, y: 0 }, data: { gain: 1 } },
    ];
    const edges: any[] = [
      { id: "e1", source: "a", sourceHandle: "Gain-source", target: "b", targetHandle: "Gain-target", data: { id: "e1", from: "a", to: "b" } },
      { id: "e2", source: "b", sourceHandle: "Gain-source", target: "a", targetHandle: "Gain-target", data: { id: "e2", from: "b", to: "a" } },
    ];
    const out = runFullValidation({ nodes, edges, validation: {}, getDef });
    expect(out.edges[0]!.data!.edgeType).toEqual(NONE);
    expect(out.edges[1]!.data!.edgeType).toEqual(NONE);
  });
});
