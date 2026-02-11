import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export function formatAudioEdgeId(
  freqency: number,
  channels: number,
  bitsPerSample: number,
) {
  return `audio_${freqency}Hz_${channels}ch_${bitsPerSample}bit`;
}
