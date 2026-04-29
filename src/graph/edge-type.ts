/**
 * Cable graph type system: edge types and node-validation contract.
 *
 * Edge type is the data carried by a connection between two node handles.
 * It is determined by the *source* node's `producedOutputs[sourceHandle]` —
 * sink nodes only declare `expectedInputs` for type-checking; they cannot
 * change an edge type by themselves.
 *
 * See plan in session-state for the full propagation rules.
 */

export type AudioEdgeType = {
  kind: "audio";
  channels: number;
  frequency: number;
  bitsPerSample: number;
};

export type FrequencyEdgeType = {
  kind: "frequency";
  channels: number;
  frequency: number;
  bins: number;
};

export type NoneEdgeType = {
  kind: "none";
};

export type EdgeType = NoneEdgeType | AudioEdgeType | FrequencyEdgeType;

export const NONE: NoneEdgeType = { kind: "none" };

export function audioType(
  channels: number,
  frequency: number,
  bitsPerSample: number,
): AudioEdgeType {
  return { kind: "audio", channels, frequency, bitsPerSample };
}

export function frequencyType(
  channels: number,
  frequency: number,
  bins: number,
): FrequencyEdgeType {
  return { kind: "frequency", channels, frequency, bins };
}

/** Structural equality. Sub-param differences make AudioData types distinct. */
export function equalEdgeType(a: EdgeType, b: EdgeType): boolean {
  if (a.kind !== b.kind) return false;
  if (a.kind === "none") return true;
  if (a.kind === "audio" && b.kind === "audio") {
    return (
      a.channels === b.channels &&
      a.frequency === b.frequency &&
      a.bitsPerSample === b.bitsPerSample
    );
  }
  if (a.kind === "frequency" && b.kind === "frequency") {
    return a.channels === b.channels && a.frequency === b.frequency && a.bins === b.bins;
  }
  return false;
}

/**
 * Compatibility check used by validators when deciding `ok`. Treats `none` as a
 * wildcard: a sink that doesn't know its expected format yet (or a source that
 * hasn't claimed one) shouldn't trigger an invalid flag. Use `equalEdgeType`
 * directly when you need strict equality (e.g. inside Mixer to compare two
 * concrete audio inputs).
 */
export function isCompatible(actual: EdgeType, expected: EdgeType): boolean {
  if (actual.kind === "none" || expected.kind === "none") return true;
  return equalEdgeType(actual, expected);
}

/** Compact human-readable label, e.g. "audio 2ch 48k 24b" or "freq 1ch 48k 1024bins". */
export function formatEdgeType(t: EdgeType): string {
  switch (t.kind) {
    case "none":
      return "none";
    case "audio":
      return `${t.channels}ch · ${formatRate(t.frequency)} · ${t.bitsPerSample}b`;
    case "frequency":
      return `freq ${t.channels}ch · ${formatRate(t.frequency)} · ${t.bins}bins`;
  }
}

function formatRate(rate: number): string {
  if (rate >= 1000) return `${Math.round(rate / 100) / 10}k`;
  return String(rate);
}

/** Map of handle id -> EdgeType. Used for both expected inputs and produced outputs. */
export type NodeTypeRecord = Record<string, EdgeType>;

export interface ValidationResult {
  /** What this node *expects* on each input handle. */
  expectedInputs: NodeTypeRecord;
  /** What this node *produces* on each output handle. */
  producedOutputs: NodeTypeRecord;
  /** Did the validator's own state + input checks pass? */
  ok: boolean;
}

/** Shallow record equality (same keys + equalEdgeType per key). */
export function equalTypeRecord(a: NodeTypeRecord, b: NodeTypeRecord): boolean {
  const ak = Object.keys(a);
  const bk = Object.keys(b);
  if (ak.length !== bk.length) return false;
  for (const k of ak) {
    if (!(k in b)) return false;
    if (!equalEdgeType(a[k], b[k])) return false;
  }
  return true;
}

/**
 * Default fallback validator for nodes that haven't declared their own.
 *
 * Treats all incoming edge types as passthrough: each input is whatever it
 * already is, and each output (if any keys are present in `outputHandles`)
 * mirrors the first non-`none` input — or `none` if there are no inputs.
 *
 * Phase 1 only: real per-node validators land in Phase 2.
 */
export function defaultPassthroughValidator(
  inputs: NodeTypeRecord,
  outputHandles: string[],
): ValidationResult {
  const firstNonNone = Object.values(inputs).find((t) => t.kind !== "none") ?? NONE;
  const producedOutputs: NodeTypeRecord = {};
  for (const h of outputHandles) producedOutputs[h] = firstNonNone;
  return {
    expectedInputs: inputs,
    producedOutputs,
    ok: true,
  };
}

/**
 * Build a 1-in / 1-out passthrough validator for effect nodes (Gain, Delay,
 * Compressor, Reverb, Echo, WaveformMonitor): produced = input, no constraint
 * on the input format.
 */
export function passthroughValidator(
  inputHandle: string,
  outputHandle: string,
): (state: unknown, inputs: NodeTypeRecord) => ValidationResult {
  return (_state, inputs) => {
    const t = inputs[inputHandle] ?? NONE;
    return {
      expectedInputs: { [inputHandle]: t },
      producedOutputs: { [outputHandle]: t },
      ok: true,
    };
  };
}
