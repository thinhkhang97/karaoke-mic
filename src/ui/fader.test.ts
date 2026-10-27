// @vitest-environment jsdom
import { describe, expect, it, vi } from "vitest";
import { createFader, formatFaderValue } from "./fader";

describe("createFader", () => {
  it("renders a range input with the fader's range and label", () => {
    const fader = createFader({ key: "delayMs", label: "DELAY", value: 230, onChange: () => {} });
    const input = fader.element.querySelector("input")!;
    expect(input.type).toBe("range");
    expect(input.min).toBe("60");
    expect(input.max).toBe("500");
    expect(input.value).toBe("230");
    expect(fader.element.textContent).toContain("DELAY");
    expect(fader.element.textContent).toContain("230ms");
  });

  it("calls onChange with the key and numeric value on input", () => {
    const onChange = vi.fn();
    const fader = createFader({ key: "delayMs", label: "DELAY", value: 230, onChange });
    const input = fader.element.querySelector("input")!;
    input.value = "300";
    input.dispatchEvent(new Event("input"));
    expect(onChange).toHaveBeenCalledWith("delayMs", 300);
  });

  it("setValue moves the knob and the readout without firing onChange", () => {
    const onChange = vi.fn();
    const fader = createFader({ key: "echo", label: "ECHO", value: 0.35, onChange });
    fader.setValue(0.4);
    expect(fader.element.querySelector("input")!.value).toBe("0.4");
    expect(fader.element.textContent).toContain("40%");
    expect(onChange).not.toHaveBeenCalled();
  });
});

describe("formatFaderValue", () => {
  it("shows percentages and milliseconds", () => {
    expect(formatFaderValue("micGain", 1.5)).toBe("150%");
    expect(formatFaderValue("repeat", 0.45)).toBe("45%");
    expect(formatFaderValue("delayMs", 230)).toBe("230ms");
  });
});
