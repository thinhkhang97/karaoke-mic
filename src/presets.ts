import type { Params } from "./state";

export type PresetId = "karaoke" | "ballad" | "fast" | "off";

export type Preset = Readonly<{ label: string; values: Readonly<Partial<Params>> }>;

/** Presets only touch the echo and reverb faders; MIC and LOA stay where they are. */
export const PRESETS: Readonly<Record<PresetId, Preset>> = Object.freeze({
  karaoke: { label: "Karaoke Bar", values: { echo: 0.35, delayMs: 230, repeat: 0.45, reverb: 0.25 } },
  ballad: { label: "Ballad", values: { echo: 0.4, delayMs: 320, repeat: 0.55, reverb: 0.35 } },
  fast: { label: "Upbeat", values: { echo: 0.25, delayMs: 150, repeat: 0.3, reverb: 0.15 } },
  off: { label: "Off", values: { echo: 0, reverb: 0 } },
});

export const PRESET_ORDER: readonly PresetId[] = ["karaoke", "ballad", "fast", "off"];

export const CUSTOM_LABEL = "Custom";
