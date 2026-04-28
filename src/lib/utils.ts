import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export function formatAudioEdgeType(freqency: number, channels: number, bitsPerSample: number) {
  return `audio_${freqency}Hz_${channels}ch_${bitsPerSample}bit`;
}

export interface AudioFormat {
  frequency: number;
  channels: number;
  bitsPerSample: number;
}

/** Parses an `audio_<freq>Hz_<ch>ch_<bits>bit` string back into its components. */
export function parseAudioEdgeType(s: string | null | undefined): AudioFormat | null {
  if (!s) return null;
  const m = /^audio_(\d+)Hz_(\d+)ch_(\d+)bit$/.exec(s);
  if (!m) return null;
  return {
    frequency: Number(m[1]),
    channels: Number(m[2]),
    bitsPerSample: Number(m[3]),
  };
}
