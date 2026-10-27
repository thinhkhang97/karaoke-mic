# Karaoke Mic Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A small Tauri desktop app that plays the mic through the chosen speaker with a karaoke-bar echo, controlled by six mixer faders and four presets.

**Architecture:** Rust owns real-time audio: cpal input stream → lock-free ring buffer with a drift-correcting resampler → DSP chain (gain, feedback echo, Freeverb reverb, soft limiter) → cpal output stream. Fader values cross to the audio thread as atomics. A watcher thread polls devices and emits status events. The TypeScript UI owns view state, presets and persistence (tauri-plugin-store), and pushes a full `Params` snapshot to Rust on every change.

**Tech Stack:** Tauri 2, Rust (cpal, ringbuf, serde, coreaudio-sys and objc2-av-foundation on macOS only), TypeScript (vanilla, Vite), Vitest, @tauri-apps/plugin-store.

**Spec:** `docs/superpowers/specs/2026-10-26-karaoke-mic-design.md`

## Global Constraints

- Only macOS 26 on Apple Silicon must work now. The code must still compile for Windows, so macOS-only code goes behind `#[cfg(target_os = "macos")]` with a non-mac stub.
- UI copy is Vietnamese, using exactly these strings: fader labels `MIC`, `ECHO`, `DELAY`, `LẶP`, `REVERB`, `LOA`; presets `Quán karaoke`, `Ballad`, `Nhanh`, `Tắt`, `Tùy chỉnh`; title `KARAOKE MIXER`; Bluetooth hint `Loa Bluetooth sẽ bị trễ, nên cắm dây AUX`.
- Fader ranges: MIC 0–200 %, ECHO 0–100 %, DELAY 60–500 ms, LẶP 0–85 %, REVERB 0–100 %, LOA 0–100 %.
- App-added latency must be at most about 20 ms round-trip on a wired output.
- The audio callbacks must not allocate, lock or log.
- The app opens unmuted and starts audio with no user action, including when the output is the built-in speakers.
- Presets never change MIC or LOA. `Tắt` sets ECHO and REVERB to 0 and keeps DELAY and LẶP.
- Mute is not persisted.
- Closing the window quits the app.
- Owner's standards: TDD, at least 80 % coverage on testable logic, source files under 500 lines, immutable state updates in TS.
- **Commits:** do not commit per task. `git init` happens in Task 1. Make one commit only after the owner has verified the app end to end in Task 8 (owner preference).

## Review Focus

1. **Mic and output running at different sample rates** (USB mic at 48 kHz, built-in output at 44.1 kHz). Expect no pitch change and no delay build-up over a long song. Pinned in Task 3 (`resampler_tracks_rate_mismatch_without_drift`).
2. **Moving DELAY while singing.** Expect no loud click or zipper noise. Pinned in Task 2 (`delay_change_is_smoothed`).
3. **Every fader at maximum** (MIC 200 %, ECHO 100 %, LẶP 85 %, REVERB 100 %, LOA 100 %). Expect the output to stay within full scale and the echo to still die away. Pinned in Task 2 (`chain_bounded_and_decays_at_max_settings`).
4. **Mic unplugged mid-song.** Expect no crash; the app falls back to another input and resumes when the mic returns. Pinned in Task 4 (`fallback_*` and `returns_to_preferred_when_back`). Task 5 wires the stream error into the watcher.
5. **Missing, corrupt or partial settings file.** Expect defaults to be used for anything invalid, without a crash. Pinned in Task 6 (`loadSettings_*`).

---

### Task 1: Toolchain and scaffold

**Files:**
- Create: the Tauri vanilla-ts project in `/Users/nguyenthinhkhang/karaoke-mic` (keep the existing `docs/` and `.superpowers/`)
- Create: `.gitignore` (add `.superpowers/`, `node_modules/`, `dist/`, `src-tauri/target/`)
- Create: `src-tauri/Info.plist` with `NSMicrophoneUsageDescription` = `Karaoke Mic cần dùng micro để phát giọng hát ra loa.`
- Modify: `src-tauri/tauri.conf.json` (window title `Karaoke Mic`, size 420×560, `resizable: false`, product name `Karaoke Mic`)
- Modify: `package.json` (add `vitest`, `@vitest/coverage-v8`, a `test` script)

- [ ] **Step 1:** Install Rust with `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y`, then source `~/.cargo/env`. Expected: `cargo --version` prints a version.
- [ ] **Step 2:** Scaffold with `pnpm create tauri-app` using the vanilla TypeScript template and the pnpm manager. Scaffold into a temporary folder and move the files in if the target folder is not empty. Then run `git init`.
- [ ] **Step 3:** Add `src-tauri/src/lib.rs` modules as empty files: `dsp/mod.rs`, `bridge.rs`, `params.rs`, `devices.rs`, `platform.rs`, `engine.rs`, `watcher.rs`, `commands.rs`. Add a trivial `src/smoke.test.ts`.
- [ ] **Step 4:** Verify. Run `cd src-tauri && cargo test` and `pnpm test`; both must pass. Run `pnpm tauri dev`; a 420×560 window titled "Karaoke Mic" must open.

### Task 2: DSP chain (echo, reverb, limiter)

**Files:**
- Create: `src-tauri/src/dsp/mod.rs`, `dsp/echo.rs`, `dsp/reverb.rs`, `dsp/limiter.rs`, `dsp/chain.rs`
- Tests: in-file `#[cfg(test)]` modules

**Interfaces:**
- Produces:
  - `pub struct Params { pub mic_gain: f32, pub echo: f32, pub delay_ms: f32, pub repeat: f32, pub reverb: f32, pub out_gain: f32, pub muted: bool }`. Derives `Clone, Copy, Debug, PartialEq, serde::{Serialize, Deserialize}` with `#[serde(rename_all = "camelCase")]`. Lives in `dsp/mod.rs`. `Params::clamped(self) -> Params` enforces the Global Constraints ranges (0–2, 0–1, 60–500, 0–0.85, 0–1, 0–1).
  - `Echo::new(sample_rate: f32) -> Echo` (buffer sized for 500 ms plus margin). `Echo::process(&mut self, x: f32, delay_ms: f32, feedback: f32) -> f32` returns the wet echo only.
  - `Reverb::new(sample_rate: f32) -> Reverb`. `Reverb::process(&mut self, x: f32) -> f32` returns wet only.
  - `soft_clip(x: f32) -> f32`, with `|soft_clip(x)| <= 1.0` for all finite x.
  - `VoiceChain::new(sample_rate: f32) -> VoiceChain`. `VoiceChain::process(&mut self, x: f32, p: &Params) -> f32`.

- [ ] **Step 1: Write the failing tests.**
  - `echo_first_repeat_at_delay`: at 48 kHz, an impulse with delay 230 ms and feedback 0 gives the wet output's peak at sample index 11040 ±2, and nothing above 1e-3 after index 11100.
  - `echo_repeats_decay_with_feedback`: with feedback 0.45, the peaks at k·delay for k = 1..4 are strictly decreasing, and each is ≤ 0.45 × the previous one.
  - `delay_change_is_smoothed`: feed a 440 Hz sine and jump `delay_ms` from 230 to 60. The largest sample-to-sample difference of the wet output within 50 ms after the jump is < 0.2.
  - `reverb_tail_decays`: the energy of the reverb output after an impulse in [1.5 s, 2 s) is < 1 % of its energy in [0, 0.5 s).
  - `soft_clip_bounded_and_transparent`: `soft_clip(0.3)` is within 0.02 of 0.3, and `|soft_clip(x)| <= 1` for x in {-100, -2, 2, 100}.
  - `chain_off_is_dry`: with echo = 0, reverb = 0, mic_gain = 1, out_gain = 1, the output equals `soft_clip(x)` for a test signal.
  - `chain_muted_is_silent`: with `muted = true`, the output is 0 after a ramp of at most 5 ms.
  - `chain_bounded_and_decays_at_max_settings`: with all faders at max, feed 1 s of a full-scale square wave and then silence. Every output sample has |y| ≤ 1, and the RMS over the window from 4 to 5 s after the input stops is < 0.01.
- [ ] **Step 2:** Run `cargo test dsp`. Expected: compile errors or FAIL.
- [ ] **Step 3: Implement.**
  - **Echo:**
    ```
    d += (target_samples - d) * 0.0015                         // per-sample delay smoothing
    tap = interp(buf, write - d)                               // linear interpolation
    lp += LP_COEF * (tap - lp)                                 // one-pole low-pass, cutoff about 3.5 kHz
    buf[write] = x + lp * feedback; write = (write + 1) % len
    return tap
    ```
  - **Reverb:** mono Freeverb. Comb tunings 1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617 and allpass tunings 556, 441, 341, 225, scaled by `sample_rate / 44100`. Room feedback 0.84, damping 0.2, allpass gain 0.5, input gain 0.015.
  - **Limiter:** `soft_clip` uses tanh above a knee of 0.5 and is linear below it.
  - **Chain:**
    ```
    v = x * mic_gain
    y = (v + echo * echo.process(v, delay_ms, repeat) + reverb * reverb.process(v)) * out_gain * mute_ramp
    return soft_clip(y)
    ```
    Here `mute_ramp` moves linearly toward 0 or 1 over 5 ms.
- [ ] **Step 4:** Run `cargo test dsp`. Expected: all PASS.

### Task 3: Shared params and the input→output bridge

**Files:**
- Create: `src-tauri/src/params.rs`, `src-tauri/src/bridge.rs`

**Interfaces:**
- Consumes: `Params` (Task 2).
- Produces:
  - `SharedParams::new(p: Params) -> SharedParams`. Methods: `store(&self, p: Params)` and `load(&self) -> Params`. Uses one `AtomicU32` per f32 (as bits) plus an `AtomicBool` for muted. Shared as `Arc<SharedParams>`.
  - `LevelMeter`: `push_block(&self, samples: &[f32])` stores the block peak; `take(&self) -> f32` returns and resets it (atomic).
  - `trait SampleSource { fn available(&self) -> usize; fn pop(&mut self) -> Option<f32>; }`, implemented for `ringbuf::HeapCons<f32>` and for `VecDeque<f32>` (in tests).
  - `Resampler::new(in_rate: f32, out_rate: f32, target_fill: usize) -> Resampler`. `Resampler::next(&mut self, src: &mut impl SampleSource) -> f32`.
  - `downmix(frame: &[f32]) -> f32` returns the average of the channels.
- [ ] **Step 1: Write the failing tests.**
  - `shared_params_roundtrip`: storing a `Params` and loading it returns an equal value.
  - `downmix_averages`: `downmix(&[0.2, 0.4]) == 0.3`.
  - `resampler_same_rate_passthrough`: at 48 k → 48 k with a pre-filled source, the output equals the input sequence (allowing the start-up offset).
  - `resampler_underrun_outputs_silence`: with an empty source, `next` returns 0.0 and does not panic.
  - `resampler_tracks_rate_mismatch_without_drift`: simulate 10 minutes. A producer pushes 48000 samples/s in 480-sample blocks, and a consumer pulls at 44100/s in 441-sample blocks. After the first second, `available()` stays within `target_fill ± 2×480`, and a 1 kHz input comes out at 1 kHz ±0.5 % (measured by zero crossings).
- [ ] **Step 2:** Run `cargo test params bridge`. Expected: FAIL.
- [ ] **Step 3: Implement.**
  - The resampler uses linear interpolation with fractional position `pos`.
  - `ratio = in_rate / out_rate * (1 + clamp(K * (available - target_fill) / target_fill, -0.01, 0.01))`, with K ≈ 0.002 per block. Tune K until the drift test passes.
  - If `available < 2`, return 0.0 and keep `pos`.
  - `target_fill` = 10 ms of input samples.
- [ ] **Step 4:** Run `cargo test params bridge`. Expected: PASS.

### Task 4: Device choice, Bluetooth and mic-permission helpers

**Files:**
- Create: `src-tauri/src/devices.rs` (pure logic), `src-tauri/src/platform.rs` (macOS FFI plus stubs)

**Interfaces:**
- Produces:
  - `pub struct DeviceInfo { pub name: String, pub is_default: bool }`.
  - `choose_input(available: &[DeviceInfo], preferred: Option<&str>) -> Option<String>`. Order: preferred if present; else the first whose name contains `USB` (case-insensitive); else the default; else the first.
  - `choose_output(available: &[DeviceInfo], preferred: Option<&str>) -> Option<String>`. Order: preferred if present; else the default; else the first.
  - `platform::is_bluetooth_output(name: &str) -> bool`. On macOS, uses the CoreAudio `kAudioDevicePropertyTransportType` ∈ {Bluetooth, BluetoothLE}, matched by device name. Elsewhere it returns `false`.
  - `platform::mic_permission() -> MicPermission`, where `enum MicPermission { Granted, Denied, Undetermined }` is serialized lowercase. On macOS it uses `AVCaptureDevice::authorizationStatusForMediaType(AVMediaTypeAudio)`, and `request_mic_permission()` triggers the prompt. Elsewhere it returns `Granted`.
- [ ] **Step 1: Write the failing tests** (devices.rs only):
  - `input_prefers_saved`
  - `input_falls_back_to_usb_then_default`: with `[MacBook Pro Microphone (default), ATR2100x-USB Microphone]` and no preference, the result is ATR2100x.
  - `fallback_when_preferred_missing`: the preferred device is gone, so the default is chosen.
  - `returns_to_preferred_when_back`: once the preferred device is in the list again, it is chosen.
  - `output_prefers_saved_then_default`
  - `empty_list_gives_none`
- [ ] **Step 2:** Run `cargo test devices`. Expected: FAIL.
- [ ] **Step 3:** Implement `devices.rs`, then `platform.rs`.
- [ ] **Step 4:** Run `cargo test devices`, then `cargo check`. Expected: PASS and a clean check. Then add a temporary `#[test] #[ignore]` that prints `is_bluetooth_output("JBL Flip 3 SE")` and run it with the speaker paired. Expected: `true`. Delete the test afterwards.

### Task 5: Audio engine, watcher and Tauri commands

**Files:**
- Create: `src-tauri/src/engine.rs`, `src-tauri/src/watcher.rs`, `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs` (managed state, command registration, store plugin, start watcher)
- Modify: `src-tauri/Cargo.toml` (`cpal`, `ringbuf`, `serde`, `tauri-plugin-store`; for macOS targets only, `coreaudio-sys` and `objc2-av-foundation`)

**Interfaces:**
- Consumes: Tasks 2–4.
- Produces:
  - `Engine::start(input: &str, output: &str, params: Arc<SharedParams>, meter: Arc<LevelMeter>, failed: Arc<AtomicBool>) -> Result<Engine, String>`. Dropping the `Engine` stops both streams. Ask both devices for 48 kHz when supported, otherwise use their default configs, and request the smallest fixed buffer that works (start at 128 frames). The stream error callbacks set `failed`.
  - `AudioStatus { inputs: Vec<String>, outputs: Vec<String>, input: Option<String>, output: Option<String>, output_is_bluetooth: bool, mic_permission: MicPermission, error: Option<String> }`, serde camelCase.
  - Tauri commands, all camelCase over IPC:
    - `start_audio(input: Option<String>, output: Option<String>, params: Params) -> Result<AudioStatus, String>`
    - `select_devices(input: Option<String>, output: Option<String>) -> Result<AudioStatus, String>`
    - `set_params(params: Params)`
    - `get_status() -> AudioStatus`
    - `get_level() -> f32`
  - Event `audio-status` with an `AudioStatus` payload, emitted by the watcher whenever the device lists, the active devices or the error change.
- The watcher thread polls every 1.5 s. It restarts the engine through `choose_input` and `choose_output` with the preferred names when:
  - the active device disappears;
  - `failed` is set;
  - the preferred device reappears.
- If the mic permission is not `Granted`, `start_audio` requests it and returns a status with `error` = `Chưa cho phép dùng micro. Vào System Settings → Privacy & Security → Microphone để bật cho Karaoke Mic.`
- [ ] **Step 1:** Write `status_reflects_choice` in `commands.rs`. It builds an `AudioStatus` from a fake device list through a pure `build_status(inputs, outputs, active_in, active_out, ...)` helper and checks that the fields are correct. Run it. Expected: FAIL.
- [ ] **Step 2:** Implement `build_status`, the engine, the watcher, the commands and the `lib.rs` wiring.
- [ ] **Step 3:** Run `cargo test` (all PASS) and `cargo clippy -- -D warnings` (clean).
- [ ] **Step 4:** Manual smoke test. Temporarily call `start_audio(None, None, karaoke defaults)` from `src/main.ts` and run `pnpm tauri dev` with the ATR2100x plugged in. Expected: you hear your voice with echo in the AUX speaker, there is no audible extra delay, and unplugging and replugging the mic recovers within about 2 s.

### Task 6: UI state, presets and persistence (TypeScript)

**Files:**
- Create: `src/presets.ts`, `src/state.ts`, `src/persist.ts`
- Tests: `src/presets.test.ts`, `src/state.test.ts`, `src/persist.test.ts`

**Interfaces:**
- Produces:
  - `type Params = { micGain: number; echo: number; delayMs: number; repeat: number; reverb: number; outGain: number; muted: boolean }`. This mirrors the Rust `Params`.
  - `type PresetId = 'karaoke' | 'ballad' | 'fast' | 'off'`.
  - `PRESETS: Readonly<Record<PresetId, { label: string; values: Partial<Params> }>>`. Values from the spec:
    - karaoke: echo .35, delayMs 230, repeat .45, reverb .25
    - ballad: .40, 320, .55, .35
    - fast: .25, 150, .30, .15
    - off: echo 0, reverb 0
  - `type FaderKey = 'micGain' | 'echo' | 'delayMs' | 'repeat' | 'reverb' | 'outGain'`.
  - `type MixerState = Readonly<{ params: Params; preset: PresetId | 'custom'; input?: string; output?: string }>`.
  - Functions in `state.ts`, each returning a new object:
    - `defaultState(): MixerState` (karaoke values, micGain 1, outGain .8, muted false)
    - `applyPreset(s, id): MixerState`
    - `setFader(s, key: FaderKey, value: number): MixerState` (clamps to the Global Constraints ranges and sets preset to `'custom'`)
    - `toggleMute(s): MixerState`
    - `selectInput(s, name): MixerState`
    - `selectOutput(s, name): MixerState`
  - `interface KeyValueStore { get<T>(k: string): Promise<T | undefined>; set(k: string, v: unknown): Promise<void>; save(): Promise<void> }`.
  - `loadSettings(store: KeyValueStore): Promise<MixerState>` and `saveSettings(store: KeyValueStore, s: MixerState): Promise<void>`. Settings are stored under key `settings`, with `muted` excluded.
- [ ] **Step 1: Write the failing tests.**
  - `applyPreset_keeps_mic_and_loa`
  - `off_keeps_delay_and_repeat`
  - `setFader_marks_custom_and_clamps`: delayMs 1000 → 500, and repeat 1 → 0.85.
  - `state_functions_do_not_mutate`: `Object.freeze` the input; there is no throw and the input is unchanged.
  - `loadSettings_missing_gives_default`
  - `loadSettings_corrupt_gives_default`: the stored value is `"garbage"` or `{params: {echo: "x"}}`.
  - `loadSettings_partial_merges_valid_fields`: a stored `{params: {echo: .9}}` gives echo .9 and default values for everything else.
  - `loadSettings_always_unmuted`
  - `saveSettings_omits_muted`
- [ ] **Step 2:** Run `pnpm test`. Expected: FAIL.
- [ ] **Step 3:** Implement. Validate by hand (check that each number is finite and then clamp it); add no schema library.
- [ ] **Step 4:** Run `pnpm test --coverage`. Expected: PASS, with at least 80 % coverage on these three files.

### Task 7: Mixer UI (layout C)

**Files:**
- Create: `src/api.ts` (typed wrappers over `invoke` and `listen`), `src/ui/mixer.ts` (render and bind), `src/ui/fader.ts`, `src/styles.css`
- Modify: `src/main.ts` (bootstrap), `index.html`
- Remove: the scaffold's demo markup and the Task 5 smoke call

**Interfaces:**
- Consumes: the Task 5 commands and `audio-status` event; the Task 6 state functions.
- Behaviour:
  - Bootstrap order: `loadSettings`, then `start_audio(state.input, state.output, state.params)`, then render.
  - Every state change calls `set_params` (or `select_devices` for the dropdowns), then calls `saveSettings`, debounced by 300 ms.
  - The level meter polls `get_level` every 50 ms.
  - On an `audio-status` event: refresh both dropdowns and show the device actually in use. Show the Bluetooth hint when `outputIsBluetooth`, and show `error` in a red line.
- Layout and look follow mockup C: `.superpowers/brainstorm/12103-1793012134/content/layout.html`, card C.
  - Dark background.
  - Two dropdowns on top.
  - Six vertical `<input type="range">` faders (use `writing-mode: vertical-lr; direction: rtl`) with labels below. Each fader shows its value as text (`%` or `ms`).
  - Preset pills, with the active one highlighted and `Tùy chỉnh` shown when the preset is custom.
  - A full-width MUTE button that turns red when muted.
- [ ] **Step 1:** Write `src/ui/fader.test.ts` using jsdom. `createFader({key: 'delayMs', label: 'DELAY', ...})` renders a range input with min 60, max 500 and the label `DELAY`. An input event calls `onChange('delayMs', value)`. Run it. Expected: FAIL.
- [ ] **Step 2:** Implement the fader, the mixer and the bootstrap. Keep each file under 300 lines.
- [ ] **Step 3:** Run `pnpm test` and `pnpm tsc --noEmit`. Expected: PASS and clean.
- [ ] **Step 4:** Run `pnpm tauri dev` and take a screenshot. Check it against mockup C, then try the presets, the faders and mute by hand.

### Task 8: End-to-end check, tuning and build

- [ ] **Step 1:** Run `pnpm tauri build`. Expected: `src-tauri/target/release/bundle/macos/Karaoke Mic.app` exists.
- [ ] **Step 2:** Code review. Dispatch a code reviewer over the whole tree. Fix every CRITICAL and HIGH finding.
- [ ] **Step 3: Owner check on the real setup** (ATR2100x-USB, plus the JBL over AUX, plus YouTube in Chrome):
  - Open the app; the voice and echo play with no clicks.
  - Every preset sounds distinct, and "Quán karaoke" sounds like a karaoke bar.
  - There is no noticeable delay.
  - Unplugging the mic falls back to another input, and replugging it switches back.
  - Quitting and reopening the app keeps the settings.

  Tune the preset values and the echo low-pass by ear with the owner, and update `PRESETS` and the spec table to match.
- [ ] **Step 4:** After the owner confirms, make one commit:

```bash
git add -A
git commit -m "feat: karaoke mic app with echo mixer"
```
