import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Params } from "./state";

/** Mirrors the Rust `AudioStatus` (serde camelCase). */
export type AudioStatus = Readonly<{
  inputs: readonly string[];
  outputs: readonly string[];
  input: string | null;
  output: string | null;
  outputIsBluetooth: boolean;
  micPermission: "granted" | "denied" | "undetermined";
  error: string | null;
}>;

export interface AudioApi {
  startAudio(input: string | undefined, output: string | undefined, params: Params): Promise<AudioStatus>;
  selectDevices(input: string | undefined, output: string | undefined): Promise<AudioStatus>;
  setParams(params: Params): Promise<void>;
  getLevel(): Promise<number>;
  onStatus(callback: (status: AudioStatus) => void): Promise<() => void>;
}

export const tauriAudioApi: AudioApi = {
  startAudio: (input, output, params) =>
    invoke<AudioStatus>("start_audio", { input: input ?? null, output: output ?? null, params }),
  selectDevices: (input, output) =>
    invoke<AudioStatus>("select_devices", { input: input ?? null, output: output ?? null }),
  setParams: (params) => invoke<void>("set_params", { params }),
  getLevel: () => invoke<number>("get_level"),
  onStatus: (callback) => listen<AudioStatus>("audio-status", (event) => callback(event.payload)),
};
