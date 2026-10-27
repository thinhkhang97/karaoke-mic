import { describe, expect, it } from "vitest";
import { PRESETS } from "./presets";
import { applyPreset, defaultState, selectInput, selectOutput, setFader, toggleMute, type MixerState } from "./state";

function deepFreeze<T>(o: T): T {
  Object.values(o as object).forEach((v) => typeof v === "object" && v !== null && deepFreeze(v));
  return Object.freeze(o);
}

describe("defaultState", () => {
  it("starts on the karaoke preset, unmuted, MIC 100 % and LOA 80 %", () => {
    const s = defaultState();
    expect(s.preset).toBe("karaoke");
    expect(s.params).toMatchObject({ ...PRESETS.karaoke.values, micGain: 1, outGain: 0.8, muted: false });
  });
});

describe("applyPreset", () => {
  it("applyPreset_keeps_mic_and_loa", () => {
    const start = setFader(setFader(defaultState(), "micGain", 1.7), "outGain", 0.3);
    const s = applyPreset(start, "ballad");
    expect(s.preset).toBe("ballad");
    expect(s.params).toMatchObject({ ...PRESETS.ballad.values, micGain: 1.7, outGain: 0.3 });
  });

  it("off_keeps_delay_and_repeat", () => {
    const start = applyPreset(defaultState(), "ballad");
    const s = applyPreset(start, "off");
    expect(s.params).toMatchObject({ echo: 0, reverb: 0, delayMs: 320, repeat: 0.55 });
  });
});

describe("setFader", () => {
  it("setFader_marks_custom_and_clamps", () => {
    const s = setFader(defaultState(), "delayMs", 1000);
    expect(s.preset).toBe("custom");
    expect(s.params.delayMs).toBe(500);
    expect(setFader(defaultState(), "repeat", 1).params.repeat).toBe(0.85);
    expect(setFader(defaultState(), "micGain", -3).params.micGain).toBe(0);
  });

  it("ignores a non-finite value", () => {
    const s = setFader(defaultState(), "echo", Number.NaN);
    expect(s.params.echo).toBe(defaultState().params.echo);
  });
});

describe("toggles and devices", () => {
  it("toggleMute flips muted", () => {
    expect(toggleMute(defaultState()).params.muted).toBe(true);
    expect(toggleMute(toggleMute(defaultState())).params.muted).toBe(false);
  });

  it("selectInput and selectOutput record the device names", () => {
    const s = selectOutput(selectInput(defaultState(), "ATR2100x-USB Microphone"), "External Headphones");
    expect(s.input).toBe("ATR2100x-USB Microphone");
    expect(s.output).toBe("External Headphones");
  });

  it("state_functions_do_not_mutate", () => {
    const frozen: MixerState = deepFreeze(defaultState());
    const before = JSON.stringify(frozen);
    expect(() => {
      applyPreset(frozen, "fast");
      setFader(frozen, "echo", 0.9);
      toggleMute(frozen);
      selectInput(frozen, "x");
      selectOutput(frozen, "y");
    }).not.toThrow();
    expect(JSON.stringify(frozen)).toBe(before);
  });
});
