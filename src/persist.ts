import { PRESETS, type PresetId } from "./presets";
import { clampFader, defaultState, FADER_RANGES, type FaderKey, type MixerState, type Params } from "./state";

/** The subset of tauri-plugin-store's `Store` this module needs (lets tests use a fake). */
export interface KeyValueStore {
  get<T>(key: string): Promise<T | undefined>;
  set(key: string, value: unknown): Promise<void>;
  save(): Promise<void>;
}

export const SETTINGS_KEY = "settings";

const FADER_KEYS = Object.keys(FADER_RANGES) as FaderKey[];

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

function readPreset(v: unknown): MixerState["preset"] | undefined {
  if (v === "custom") return "custom";
  return typeof v === "string" && v in PRESETS ? (v as PresetId) : undefined;
}

function readParams(v: unknown, fallback: Params): Params {
  if (!isRecord(v)) return fallback;
  const valid = FADER_KEYS.flatMap((key) => {
    const value = v[key];
    return typeof value === "number" && Number.isFinite(value) ? [[key, clampFader(key, value)] as const] : [];
  });
  // Mute is never restored: the app always opens unmuted.
  return { ...fallback, ...Object.fromEntries(valid), muted: false };
}

/** Loads saved settings, keeping every valid field and using defaults for the rest. */
export async function loadSettings(store: KeyValueStore): Promise<MixerState> {
  const fallback = defaultState();
  const raw = await store.get<unknown>(SETTINGS_KEY);
  if (!isRecord(raw)) return fallback;
  return {
    params: readParams(raw.params, fallback.params),
    preset: readPreset(raw.preset) ?? fallback.preset,
    ...(typeof raw.input === "string" ? { input: raw.input } : {}),
    ...(typeof raw.output === "string" ? { output: raw.output } : {}),
  };
}

export async function saveSettings(store: KeyValueStore, s: MixerState): Promise<void> {
  const { muted: _muted, ...params } = s.params;
  await store.set(SETTINGS_KEY, { ...s, params });
  await store.save();
}
