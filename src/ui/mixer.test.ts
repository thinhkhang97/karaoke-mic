// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AudioApi, AudioStatus } from "../api";
import type { KeyValueStore } from "../persist";
import { PRESETS } from "../presets";
import { defaultState, setFader } from "../state";
import { mountMixer } from "./mixer";

const status = (over: Partial<AudioStatus> = {}): AudioStatus => ({
  inputs: ["MacBook Pro Microphone", "ATR2100x-USB Microphone"],
  outputs: ["MacBook Pro Speakers", "External Headphones"],
  input: "ATR2100x-USB Microphone",
  output: "External Headphones",
  outputIsBluetooth: false,
  micPermission: "granted",
  error: null,
  ...over,
});

function fakeApi() {
  let listener: ((s: AudioStatus) => void) | undefined;
  const api = {
    startAudio: vi.fn(async () => status()),
    selectDevices: vi.fn(async () => status({ input: "MacBook Pro Microphone" })),
    setParams: vi.fn(async () => {}),
    getLevel: vi.fn(async () => 0.5),
    onStatus: vi.fn(async (cb: (s: AudioStatus) => void) => {
      listener = cb;
      return () => {};
    }),
  } satisfies AudioApi;
  return { api, emit: (s: AudioStatus) => listener?.(s) };
}

const store = (): KeyValueStore => ({ get: async () => undefined, set: vi.fn(async () => {}), save: vi.fn(async () => {}) });

describe("mountMixer", () => {
  let root: HTMLElement;
  beforeEach(() => {
    document.body.innerHTML = "";
    root = document.createElement("div");
    document.body.append(root);
  });

  it("starts audio with the saved devices and params, then shows six faders", async () => {
    const { api } = fakeApi();
    const initial = { ...defaultState(), input: "ATR2100x-USB Microphone" };
    await mountMixer({ root, initial, api, store: store() });
    expect(api.startAudio).toHaveBeenCalledWith("ATR2100x-USB Microphone", undefined, initial.params);
    const labels = [...root.querySelectorAll(".fader-label")].map((e) => e.textContent);
    expect(labels).toEqual(["MIC", "ECHO", "DELAY", "REPEAT", "REVERB", "OUTPUT"]);
    expect(root.textContent).toContain("KARAOKE MIXER");
    expect([...root.querySelectorAll(".device")].map((e) => e.firstChild?.textContent)).toEqual(["Mic", "Speaker"]);
    expect([...root.querySelectorAll("[data-preset]")].map((e) => e.textContent)).toEqual(["Karaoke Bar", "Ballad", "Upbeat", "Off"]);
    expect((root.querySelector("select[name=input]") as HTMLSelectElement).value).toBe("ATR2100x-USB Microphone");
  });

  it("tapping a preset pushes its values and highlights it", async () => {
    const { api } = fakeApi();
    const mixer = await mountMixer({ root, initial: defaultState(), api, store: store() });
    (root.querySelector("[data-preset=ballad]") as HTMLButtonElement).click();
    expect(mixer.state().preset).toBe("ballad");
    expect(api.setParams).toHaveBeenLastCalledWith(expect.objectContaining(PRESETS.ballad.values));
    expect(root.querySelector("[data-preset=ballad]")!.classList.contains("active")).toBe(true);
  });

  it("moving a fader switches to Custom", async () => {
    const { api } = fakeApi();
    const mixer = await mountMixer({ root, initial: defaultState(), api, store: store() });
    const echo = root.querySelector("input[data-key=echo]") as HTMLInputElement;
    echo.value = "0.8";
    echo.dispatchEvent(new Event("input"));
    expect(mixer.state()).toEqual(setFader(defaultState(), "echo", 0.8));
    expect(root.querySelector(".preset-custom")!.classList.contains("active")).toBe(true);
    expect(root.querySelector(".preset-custom")!.textContent).toBe("Custom");
  });

  it("mute toggles and is pushed to the engine", async () => {
    const { api } = fakeApi();
    await mountMixer({ root, initial: defaultState(), api, store: store() });
    const mute = root.querySelector(".mute") as HTMLButtonElement;
    mute.click();
    expect(api.setParams).toHaveBeenLastCalledWith(expect.objectContaining({ muted: true }));
    expect(mute.classList.contains("on")).toBe(true);
    expect(mute.textContent).toBe("🔇 MUTED — tap to unmute");
  });

  it("choosing a mic asks the engine to switch devices", async () => {
    const { api } = fakeApi();
    const mixer = await mountMixer({ root, initial: defaultState(), api, store: store() });
    const select = root.querySelector("select[name=input]") as HTMLSelectElement;
    select.value = "MacBook Pro Microphone";
    select.dispatchEvent(new Event("change"));
    await vi.waitFor(() => expect(api.selectDevices).toHaveBeenCalledWith("MacBook Pro Microphone", undefined));
    expect(mixer.state().input).toBe("MacBook Pro Microphone");
  });

  it("shows the Bluetooth hint and errors from status events", async () => {
    const { api, emit } = fakeApi();
    await mountMixer({ root, initial: defaultState(), api, store: store() });
    expect(root.querySelector(".hint")!.hasAttribute("hidden")).toBe(true);
    emit(status({ outputIsBluetooth: true, error: "No microphone or speaker found" }));
    expect(root.querySelector(".hint")!.hasAttribute("hidden")).toBe(false);
    expect(root.textContent).toContain("Bluetooth speakers add delay — use an AUX cable");
    expect(root.querySelector(".error")!.textContent).toBe("No microphone or speaker found");
  });

  it("saves settings after changes, debounced", async () => {
    const { api } = fakeApi();
    const s = store();
    await mountMixer({ root, initial: defaultState(), api, store: s, saveDelayMs: 0 });
    (root.querySelector("[data-preset=fast]") as HTMLButtonElement).click();
    await vi.waitFor(() => expect(s.save).toHaveBeenCalled());
  });
});
