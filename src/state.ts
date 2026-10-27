import { PRESETS, type PresetId } from "./presets";

/** Mirrors the Rust `Params` (serde camelCase). */
export type Params = Readonly<{
  micGain: number;
  echo: number;
  delayMs: number;
  repeat: number;
  reverb: number;
  outGain: number;
  muted: boolean;
}>;

export type FaderKey = "micGain" | "echo" | "delayMs" | "repeat" | "reverb" | "outGain";

export type MixerState = Readonly<{
  params: Params;
  preset: PresetId | "custom";
  input?: string;
  output?: string;
}>;

export const FADER_RANGES: Readonly<Record<FaderKey, Readonly<{ min: number; max: number }>>> = {
  micGain: { min: 0, max: 2 },
  echo: { min: 0, max: 1 },
  delayMs: { min: 60, max: 500 },
  repeat: { min: 0, max: 0.85 },
  reverb: { min: 0, max: 1 },
  outGain: { min: 0, max: 1 },
};

export function clampFader(key: FaderKey, value: number): number {
  const { min, max } = FADER_RANGES[key];
  return Math.min(max, Math.max(min, value));
}

export function defaultState(): MixerState {
  return {
    params: { micGain: 1, outGain: 0.8, muted: false, delayMs: 230, repeat: 0.45, echo: 0.35, reverb: 0.25, ...PRESETS.karaoke.values },
    preset: "karaoke",
  };
}

export function applyPreset(s: MixerState, id: PresetId): MixerState {
  return { ...s, preset: id, params: { ...s.params, ...PRESETS[id].values } };
}

export function setFader(s: MixerState, key: FaderKey, value: number): MixerState {
  if (!Number.isFinite(value)) return s;
  return { ...s, preset: "custom", params: { ...s.params, [key]: clampFader(key, value) } };
}

export function toggleMute(s: MixerState): MixerState {
  return { ...s, params: { ...s.params, muted: !s.params.muted } };
}

export function selectInput(s: MixerState, name: string): MixerState {
  return { ...s, input: name };
}

export function selectOutput(s: MixerState, name: string): MixerState {
  return { ...s, output: name };
}
