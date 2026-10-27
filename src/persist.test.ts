import { describe, expect, it } from "vitest";
import { loadSettings, saveSettings, SETTINGS_KEY, type KeyValueStore } from "./persist";
import { defaultState, setFader, toggleMute, selectInput } from "./state";

function memoryStore(initial?: unknown): KeyValueStore & { data: Map<string, unknown>; saves: number } {
  const data = new Map<string, unknown>(initial === undefined ? [] : [[SETTINGS_KEY, initial]]);
  return {
    data,
    saves: 0,
    async get<T>(key: string) {
      return data.get(key) as T | undefined;
    },
    async set(key: string, value: unknown) {
      data.set(key, value);
    },
    async save() {
      this.saves += 1;
    },
  };
}

describe("loadSettings", () => {
  it("loadSettings_missing_gives_default", async () => {
    expect(await loadSettings(memoryStore())).toEqual(defaultState());
  });

  it("loadSettings_corrupt_gives_default", async () => {
    expect(await loadSettings(memoryStore("garbage"))).toEqual(defaultState());
    expect(await loadSettings(memoryStore({ params: { echo: "x" } }))).toEqual(defaultState());
    expect(await loadSettings(memoryStore({ params: null, preset: 42, input: 7 }))).toEqual(defaultState());
  });

  it("loadSettings_partial_merges_valid_fields", async () => {
    const s = await loadSettings(memoryStore({ params: { echo: 0.9 }, preset: "custom" }));
    expect(s.params).toEqual({ ...defaultState().params, echo: 0.9 });
    expect(s.preset).toBe("custom");
  });

  it("clamps out-of-range stored values", async () => {
    const s = await loadSettings(memoryStore({ params: { delayMs: 9000 } }));
    expect(s.params.delayMs).toBe(500);
  });

  it("loadSettings_always_unmuted", async () => {
    const s = await loadSettings(memoryStore({ params: { muted: true } }));
    expect(s.params.muted).toBe(false);
  });

  it("restores device names", async () => {
    const s = await loadSettings(memoryStore({ input: "ATR2100x-USB Microphone", output: "External Headphones" }));
    expect(s.input).toBe("ATR2100x-USB Microphone");
    expect(s.output).toBe("External Headphones");
  });
});

describe("saveSettings", () => {
  it("saveSettings_omits_muted and round-trips", async () => {
    const store = memoryStore();
    const state = toggleMute(selectInput(setFader(defaultState(), "echo", 0.6), "Mic"));
    await saveSettings(store, state);
    const saved = store.data.get(SETTINGS_KEY) as { params: Record<string, unknown> };
    expect(saved.params).not.toHaveProperty("muted");
    expect(store.saves).toBe(1);
    const loaded = await loadSettings(store);
    expect(loaded).toEqual({ ...state, params: { ...state.params, muted: false } });
  });
});
