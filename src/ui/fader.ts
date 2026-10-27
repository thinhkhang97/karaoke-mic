import { FADER_RANGES, type FaderKey } from "../state";

export type FaderOptions = Readonly<{
  key: FaderKey;
  label: string;
  value: number;
  onChange: (key: FaderKey, value: number) => void;
}>;

export type Fader = Readonly<{ element: HTMLElement; setValue: (value: number) => void }>;

const STEPS: Readonly<Record<FaderKey, number>> = {
  micGain: 0.01,
  echo: 0.01,
  delayMs: 5,
  repeat: 0.01,
  reverb: 0.01,
  outGain: 0.01,
};

export function formatFaderValue(key: FaderKey, value: number): string {
  return key === "delayMs" ? `${Math.round(value)}ms` : `${Math.round(value * 100)}%`;
}

/** One vertical mixer fader: readout on top, slider, label underneath. */
export function createFader(options: FaderOptions): Fader {
  const { key, label, value, onChange } = options;
  const { min, max } = FADER_RANGES[key];

  const element = document.createElement("div");
  element.className = "fader";

  const readout = document.createElement("span");
  readout.className = "fader-value";

  const input = document.createElement("input");
  input.type = "range";
  input.min = String(min);
  input.max = String(max);
  input.step = String(STEPS[key]);
  input.dataset.key = key;
  input.setAttribute("aria-label", label);

  const caption = document.createElement("span");
  caption.className = "fader-label";
  caption.textContent = label;

  const setValue = (v: number) => {
    input.value = String(v);
    readout.textContent = formatFaderValue(key, v);
  };

  input.addEventListener("input", () => {
    const v = Number(input.value);
    readout.textContent = formatFaderValue(key, v);
    onChange(key, v);
  });

  setValue(value);
  element.append(readout, input, caption);
  return { element, setValue };
}
