//! Tauri commands the UI calls, and the status payload it receives.

use serde::Serialize;
use tauri::State;

use crate::devices::DeviceInfo;
use crate::dsp::Params;
use crate::platform::MicPermission;
use crate::watcher::AudioHandle;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioStatus {
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub input: Option<String>,
    pub output: Option<String>,
    pub output_is_bluetooth: bool,
    pub mic_permission: MicPermission,
    pub error: Option<String>,
}

pub struct StatusParts<'a> {
    pub inputs: &'a [DeviceInfo],
    pub outputs: &'a [DeviceInfo],
    pub active_input: Option<&'a str>,
    pub active_output: Option<&'a str>,
    pub output_is_bluetooth: bool,
    pub mic_permission: MicPermission,
    pub error: Option<String>,
}

pub fn build_status(parts: StatusParts) -> AudioStatus {
    let names = |list: &[DeviceInfo]| list.iter().map(|d| d.name.clone()).collect();
    AudioStatus {
        inputs: names(parts.inputs),
        outputs: names(parts.outputs),
        input: parts.active_input.map(str::to_string),
        output: parts.active_output.map(str::to_string),
        output_is_bluetooth: parts.output_is_bluetooth,
        mic_permission: parts.mic_permission,
        error: parts.error,
    }
}

/// Runs a request against the audio manager thread off the main thread, so
/// opening devices never freezes the window.
async fn off_main_thread(
    audio: &AudioHandle,
    request: impl FnOnce(AudioHandle) -> Result<AudioStatus, String> + Send + 'static,
) -> Result<AudioStatus, String> {
    let handle = audio.clone();
    tauri::async_runtime::spawn_blocking(move || request(handle))
        .await
        .map_err(|e| format!("Audio engine error: {e}"))?
}

/// Stores the fader values and starts audio with the preferred devices.
#[tauri::command]
pub async fn start_audio(
    audio: State<'_, AudioHandle>,
    input: Option<String>,
    output: Option<String>,
    params: Params,
) -> Result<AudioStatus, String> {
    audio.params.store(params.clamped());
    off_main_thread(&audio, move |a| a.select(input, output)).await
}

#[tauri::command]
pub async fn select_devices(
    audio: State<'_, AudioHandle>,
    input: Option<String>,
    output: Option<String>,
) -> Result<AudioStatus, String> {
    off_main_thread(&audio, move |a| a.select(input, output)).await
}

#[tauri::command]
pub fn set_params(audio: State<'_, AudioHandle>, params: Params) {
    audio.params.store(params.clamped());
}

#[tauri::command]
pub async fn get_status(audio: State<'_, AudioHandle>) -> Result<AudioStatus, String> {
    off_main_thread(&audio, |a| a.status()).await
}

#[tauri::command]
pub fn get_level(audio: State<'_, AudioHandle>) -> f32 {
    audio.meter.take()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_reflects_choice() {
        let ins = vec![DeviceInfo { name: "ATR2100x-USB Microphone".into(), is_default: false }];
        let outs = vec![
            DeviceInfo { name: "MacBook Pro Speakers".into(), is_default: true },
            DeviceInfo { name: "External Headphones".into(), is_default: false },
        ];
        let s = build_status(StatusParts {
            inputs: &ins,
            outputs: &outs,
            active_input: Some("ATR2100x-USB Microphone"),
            active_output: Some("External Headphones"),
            output_is_bluetooth: false,
            mic_permission: MicPermission::Granted,
            error: None,
        });
        assert_eq!(s.inputs, vec!["ATR2100x-USB Microphone"]);
        assert_eq!(s.outputs, vec!["MacBook Pro Speakers", "External Headphones"]);
        assert_eq!(s.input.as_deref(), Some("ATR2100x-USB Microphone"));
        assert_eq!(s.output.as_deref(), Some("External Headphones"));
        assert!(!s.output_is_bluetooth);
        assert_eq!(s.error, None);
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json["outputIsBluetooth"], false);
        assert_eq!(json["micPermission"], "granted");
    }
}
