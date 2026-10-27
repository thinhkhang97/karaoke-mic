import { load } from "@tauri-apps/plugin-store";
import { tauriAudioApi } from "./api";
import { loadSettings } from "./persist";
import { defaultState } from "./state";
import { mountMixer } from "./ui/mixer";

async function main(): Promise<void> {
  const root = document.querySelector<HTMLElement>("#app");
  if (!root) return;
  const store = await load("settings.json", { autoSave: false, defaults: {} });
  const initial = await loadSettings(store).catch(() => defaultState());
  await mountMixer({ root, initial, api: tauriAudioApi, store });
}

main().catch((e) => {
  document.body.textContent = `Failed to start: ${String(e)}`;
});
