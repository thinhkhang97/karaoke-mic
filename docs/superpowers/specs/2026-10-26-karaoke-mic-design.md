# Karaoke Mic: Design

Date: 2026-10-26
Status: approved by owner 2026-10-26

## Intent

The owner sings karaoke at home with a wired USB microphone (Audio-Technica
ATR2100x-USB), a MacBook Pro (Apple Silicon, macOS 26) and a JBL Flip 3 SE speaker
connected by a 3.5mm AUX cable. Music comes from YouTube in a browser.

Today the only way to hear their voice through the speaker with echo is
GarageBand. It works, but it is heavy to open and needs several steps each time.
They want a small app that does just one thing: **open it, plug in the mic, sing.**
The mic is played through the chosen speaker immediately with a karaoke-bar echo,
and every control is adjustable.

Echo is the point of the app. In the owner's words, "karaoke without echo is
useless". The default sound should be like a Vietnamese karaoke bar: a clear
repeating echo ("anh… anh… anh…") that fades out over 3–5 repeats, plus a little
reverb to smooth the voice.

## Goals

1. Open the app and the voice is already playing through the speaker with the
   "Quán karaoke" preset. No setup and no start button.
2. Pick the input device (mic) and the output device (speaker/headphones) from
   two dropdowns.
3. Six vertical faders, styled like a karaoke-bar mixer: MIC, ECHO, DELAY, LẶP,
   REVERB, LOA (meanings below).
4. One-tap presets: Quán karaoke, Ballad, Nhanh, Tắt.
5. A large MUTE button for when the speaker starts howling (feedback).
6. The app remembers the last devices, fader positions and preset between runs.
7. Plugging or unplugging the mic or speaker while the app is open is handled
   without restarting the app.
8. Light and fast: a small download that opens almost instantly.

## Non-goals (this version)

- Recording, playing music, or showing lyrics inside the app. YouTube stays in
  the browser.
- Pitch or key shifting, auto-tune, EQ beyond what the echo needs.
- More than one microphone at a time.
- A public release, code signing, notarisation, auto-update. It is for the
  owner's own Mac first.
- Shipping a Windows build now. The code must stay cross-platform (see
  Constraints) so a Windows build can be added later.
- Fixing Bluetooth latency. That is a property of Bluetooth and cannot be fixed
  in software.

## Anti-goals (would make it a failure even if it technically works)

- **Audible delay added by the app.** Any delay the app adds on top of the
  device itself makes singing feel off-beat. Target: at most about 20 ms
  round-trip added by the app on a wired output.
- Clicks, crackles or dropouts in the audio while singing.
- Needing any setup or button press before the voice comes out.
- Harsh digital clipping when someone sings loudly.
- Feeling as heavy as GarageBand to open.

## Screen (chosen: layout C, "karaoke mixer")

One small fixed-size dark window, no menus beyond the OS defaults.

- Top: title "KARAOKE MIXER" and two dropdowns side by side: **Mic** and **Loa**.
- Middle: six vertical faders with labels underneath, left to right:

| Fader  | Meaning                                         | Range (builder may tune) |
|--------|-------------------------------------------------|--------------------------|
| MIC    | Input gain of the voice                         | 0–200 %                  |
| ECHO   | Level of the echo relative to the dry voice     | 0–100 %                  |
| DELAY  | Time between echo repeats                       | 60–500 ms                |
| LẶP    | Echo feedback: how many repeats before it fades | 0–85 %                   |
| REVERB | Level of the room reverb                        | 0–100 %                  |
| LOA    | Final output volume                             | 0–100 %                  |

- Below the faders: preset buttons **Quán karaoke**, **Ballad**, **Nhanh**, **Tắt**.
  The active one is highlighted. Moving any fader by hand switches the
  highlight to a "Tùy chỉnh" (custom) state.
- Bottom: a wide **MUTE** button. When muted it is clearly lit.
- A small input level meter next to the MIC fader so the owner can see the mic
  is alive.
- A one-line hint appears under the dropdowns when the selected output looks
  like a Bluetooth device: "Loa Bluetooth sẽ bị trễ, nên cắm dây AUX".

The UI language was Vietnamese in the first build. On 2026-10-27 the owner switched it to English when open-sourcing the project: MIC, ECHO, DELAY, REPEAT, REVERB, OUTPUT; presets Karaoke Bar, Ballad, Upbeat, Off, Custom; device labels Mic and Speaker.

## Behaviour

**Startup**
- Load saved settings. If none, use the "Quán karaoke" preset.
- Input: the saved mic if present; else the first USB microphone found; else
  the system default input.
- Output: the saved output if present; else the system default output.
- Start passing audio through immediately (not muted). **This is the owner's
  decision:** it applies even when the output is the MacBook's built-in speakers.
  A feedback howl in that case is accepted, and MUTE is the remedy. Do not
  auto-mute and do not refuse built-in speakers.
- Device matching is by device name. The "first USB microphone" heuristic is
  the builder's call, because cpal does not report the transport type. Use
  CoreAudio on macOS if needed, or skip the heuristic and use the system default
  input.
- Closing the window quits the app. There is no tray or background mode.
- On macOS the first run triggers the system microphone permission prompt. If
  permission is denied, show a clear Vietnamese message telling the owner where
  to allow it (System Settings → Privacy & Security → Microphone).

**Devices changing while running**
- The device lists refresh on their own (polling every 1–2 s is fine).
- If the selected device disappears, fall back the way startup does and show
  which device is now used. If the saved device comes back, switch back to it.
- Changing a dropdown takes effect immediately without restarting the app.

**Presets**
Starting values, which the builder should tune by ear:

| Preset       | ECHO | DELAY  | LẶP | REVERB |
|--------------|------|--------|-----|--------|
| Quán karaoke | 35 % | 230 ms | 45 %| 25 %   |
| Ballad       | 40 % | 320 ms | 55 %| 35 %   |
| Nhanh        | 25 % | 150 ms | 30 %| 15 %   |
| Tắt          | 0 %  | (keep) | (keep)| 0 %  |

Presets do not change MIC or LOA. Tapping a preset moves the faders to its
values.

**Mute** silences the output instantly. It does not stop the device streams, so
unmuting is instant too. The mute state is not remembered; the app always opens
unmuted.

**Persistence**: devices (by name), the six fader values and the active preset,
stored in the app's per-user config folder and written whenever they change.

## How it gets made

Decided:
- **Tauri 2** desktop app. UI in **TypeScript** (the owner's strongest
  language). The audio engine is in **Rust**, using **`cpal`** for device access
  (CoreAudio on macOS, WASAPI on Windows).
- For now it is built and run on the owner's Mac only. No GitHub repo yet.
- Project folder: `/Users/nguyenthinhkhang/karaoke-mic`. "Karaoke Mic" is the
  working name.

Claude's calls (the owner may override):
- Audio pipeline: mono mic input → MIC gain → dry voice plus an **echo**
  (feedback delay line with a gentle low-pass filter inside the feedback loop,
  so each repeat sounds a bit darker, like a bar mixer's echo) plus a
  **reverb** (Freeverb-style) → LOA gain → soft limiter → stereo output.
- Fader values reach the audio thread lock-free (atomics). There is no
  allocation or locking in the audio callback.
- Input and output are separate devices, so they may run at different sample
  rates and clocks. The engine must handle that, by resampling or by bridging
  with a small ring buffer that has drift correction. It must not glitch or
  build up delay over a long session.
- The UI framework inside Tauri is the builder's choice. Keep it light.
- Bluetooth detection on macOS uses the CoreAudio transport type. On Windows
  the hint can be skipped in this version.

## Constraints

- macOS 26 on Apple Silicon is the only platform that must work now.
- Do not use any macOS-only API in a way that breaks a Windows build. Put any
  platform-specific bits (Bluetooth hint, permission prompt) behind
  `cfg(target_os)`.
- Rust is not installed on the owner's Mac yet. Setup will install it through
  rustup. Node 22, npm, pnpm and bun are available.
- The app needs `NSMicrophoneUsageDescription` and the audio-input entitlement
  so macOS allows microphone access.
- The owner's standards: TDD, 80% test coverage on testable logic, files under
  500 lines (frontend components under 800), immutable data patterns in TS.

## Testing

- Rust unit tests for the DSP: delay timing, echo repeats decaying with
  feedback, LẶP = 0 giving a single echo, "Tắt" giving dry output, the limiter
  never exceeding full scale, and preset values.
- Rust tests for device fallback logic, using a fake device list.
- TS tests for UI state: switching presets, the "Tùy chỉnh" state on manual
  moves, save and load of settings.
- Manual end-to-end check on the owner's Mac with the ATR2100x-USB and the JBL
  over AUX: open the app, sing, check the echo sounds right, check there is no
  noticeable delay, then unplug and replug the mic.

## Left to the builder

- Exact fader curves (linear or logarithmic), knob tuning and preset values,
  judged by ear.
- Exact window size and visual polish within the "dark karaoke mixer" style
  from mockup C.
- The choice of resampler or ring-buffer crate.
- Buffer sizes: as small as works reliably without crackles.
