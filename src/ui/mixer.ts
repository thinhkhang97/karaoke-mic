import type { AudioApi, AudioStatus } from "../api";
import { saveSettings, type KeyValueStore } from "../persist";
import { CUSTOM_LABEL, PRESET_ORDER, PRESETS, type PresetId } from "../presets";
import { applyPreset, selectInput, selectOutput, setFader, toggleMute, type FaderKey, type MixerState } from "../state";
import { createFader, type Fader } from "./fader";

export type MixerDeps = Readonly<{
  root: HTMLElement;
  initial: MixerState;
  api: AudioApi;
  store: KeyValueStore;
  saveDelayMs?: number;
}>;

export type Mixer = Readonly<{ state: () => MixerState; dispose: () => void }>;

const FADERS: ReadonlyArray<readonly [FaderKey, string]> = [
  ["micGain", "MIC"],
  ["echo", "ECHO"],
  ["delayMs", "DELAY"],
  ["repeat", "REPEAT"],
  ["reverb", "REVERB"],
  ["outGain", "OUTPUT"],
];
const BLUETOOTH_HINT = "Bluetooth speakers add delay — use an AUX cable";
const DEFAULT_SAVE_DELAY_MS = 300;
const METER_INTERVAL_MS = 50;

function el<K extends keyof HTMLElementTagNameMap>(tag: K, className = "", text = ""): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text) node.textContent = text;
  return node;
}

function deviceSelect(name: "input" | "output", label: string): { wrap: HTMLElement; select: HTMLSelectElement } {
  const wrap = el("label", "device", label);
  const select = el("select");
  select.name = name;
  wrap.append(select);
  return { wrap, select };
}

function fillOptions(select: HTMLSelectElement, names: readonly string[], active: string | null): void {
  select.replaceChildren(
    ...names.map((n) => {
      const option = el("option", "", n);
      option.value = n;
      return option;
    }),
  );
  if (active !== null) select.value = active;
}

/** Builds the karaoke mixer UI in `root`, starts audio and keeps both in sync. */
export async function mountMixer(deps: MixerDeps): Promise<Mixer> {
  const { root, api, store } = deps;
  const saveDelay = deps.saveDelayMs ?? DEFAULT_SAVE_DELAY_MS;
  let state = deps.initial;
  let saveTimer: ReturnType<typeof setTimeout> | undefined;

  const input = deviceSelect("input", "Mic");
  const output = deviceSelect("output", "Speaker");
  const hint = el("p", "hint", BLUETOOTH_HINT);
  const error = el("p", "error");
  const meterFill = el("div", "meter-fill");
  const meter = el("div", "meter");
  meter.append(meterFill);

  const faders = new Map<FaderKey, Fader>(
    FADERS.map(([key, label]) => [key, createFader({ key, label, value: state.params[key], onChange: onFader })]),
  );
  const faderRow = el("div", "faders");
  faderRow.append(meter, ...[...faders.values()].map((f) => f.element));

  const presetButtons = new Map<PresetId, HTMLButtonElement>(
    PRESET_ORDER.map((id) => {
      const button = el("button", "preset", PRESETS[id].label);
      button.dataset.preset = id;
      button.addEventListener("click", () => update(applyPreset(state, id)));
      return [id, button];
    }),
  );
  const custom = el("span", "preset preset-custom", CUSTOM_LABEL);
  const presetRow = el("div", "presets");
  presetRow.append(...presetButtons.values(), custom);

  const mute = el("button", "mute");
  mute.addEventListener("click", () => update(toggleMute(state)));

  const devices = el("div", "devices");
  devices.append(input.wrap, output.wrap);
  root.replaceChildren(el("h1", "title", "KARAOKE MIXER"), devices, hint, error, faderRow, presetRow, mute);

  input.select.addEventListener("change", () => changeDevices(selectInput(state, input.select.value)));
  output.select.addEventListener("change", () => changeDevices(selectOutput(state, output.select.value)));

  function onFader(key: FaderKey, value: number): void {
    update(setFader(state, key, value));
  }

  function render(): void {
    FADERS.forEach(([key]) => faders.get(key)?.setValue(state.params[key]));
    presetButtons.forEach((button, id) => button.classList.toggle("active", state.preset === id));
    custom.classList.toggle("active", state.preset === "custom");
    mute.classList.toggle("on", state.params.muted);
    mute.textContent = state.params.muted ? "🔇 MUTED — tap to unmute" : "🔇 MUTE";
  }

  function scheduleSave(): void {
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      saveSettings(store, state).catch((e) => console.error("Could not save settings", e));
    }, saveDelay);
  }

  function update(next: MixerState): void {
    state = next;
    render();
    api.setParams(state.params).catch((e) => showError(String(e)));
    scheduleSave();
  }

  function changeDevices(next: MixerState): void {
    state = next;
    scheduleSave();
    api.selectDevices(state.input, state.output).then(applyStatus, (e) => showError(String(e)));
  }

  function showError(message: string | null): void {
    error.textContent = message ?? "";
    error.hidden = !message;
  }

  function applyStatus(status: AudioStatus): void {
    fillOptions(input.select, status.inputs, status.input);
    fillOptions(output.select, status.outputs, status.output);
    hint.hidden = !status.outputIsBluetooth;
    showError(status.error);
  }

  render();
  hint.hidden = true;
  showError(null);
  const unlisten = await api.onStatus(applyStatus);
  try {
    applyStatus(await api.startAudio(state.input, state.output, state.params));
  } catch (e) {
    showError(`Could not start audio: ${String(e)}`);
  }

  const meterTimer = setInterval(() => {
    api
      .getLevel()
      .then((level) => (meterFill.style.height = `${Math.round(Math.min(1, Math.sqrt(level)) * 100)}%`))
      .catch(() => {});
  }, METER_INTERVAL_MS);

  return {
    state: () => state,
    dispose: () => {
      clearInterval(meterTimer);
      clearTimeout(saveTimer);
      unlisten();
    },
  };
}
