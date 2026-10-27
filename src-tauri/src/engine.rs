//! Opens the input and output devices and runs the real-time path:
//! input callback → ring buffer → resampler → voice chain → output callback.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, Device, Host, SampleFormat, Stream, StreamConfig, SupportedBufferSize};
use ringbuf::HeapRb;
use ringbuf::traits::{Producer, Split};

use crate::bridge::{Resampler, downmix};
use crate::devices::DeviceInfo;
use crate::dsp::chain::VoiceChain;
use crate::params::{LevelMeter, SharedParams};

const PREFERRED_RATE: u32 = 48_000;
const PREFERRED_BUFFER_FRAMES: u32 = 128;
const TARGET_FILL_SECONDS: f32 = 0.010;
const QUEUE_SECONDS: f32 = 1.0;

/// State shared between the manager thread and the audio callbacks.
#[derive(Clone)]
pub struct EngineShared {
    pub params: Arc<SharedParams>,
    pub meter: Arc<LevelMeter>,
    pub failed: Arc<AtomicBool>,
}

/// A running input + output pair. Dropping it stops both streams.
pub struct Engine {
    pub input: String,
    pub output: String,
    _input_stream: Stream,
    _output_stream: Stream,
}

pub fn list_inputs(host: &Host) -> Vec<DeviceInfo> {
    let default = host.default_input_device().map(|d| d.to_string());
    let devices = host.input_devices().map(|it| it.collect::<Vec<_>>()).unwrap_or_default();
    to_infos(devices, default)
}

pub fn list_outputs(host: &Host) -> Vec<DeviceInfo> {
    let default = host.default_output_device().map(|d| d.to_string());
    let devices = host.output_devices().map(|it| it.collect::<Vec<_>>()).unwrap_or_default();
    to_infos(devices, default)
}

fn to_infos(devices: Vec<Device>, default: Option<String>) -> Vec<DeviceInfo> {
    devices
        .into_iter()
        .map(|d| d.to_string())
        .map(|name| DeviceInfo { is_default: Some(&name) == default.as_ref(), name })
        .collect()
}

impl Engine {
    /// Opens both devices with a small fixed buffer, retrying with the
    /// devices' default buffer size if they refuse it.
    pub fn start(host: &Host, input: &str, output: &str, shared: &EngineShared) -> Result<Engine, String> {
        Engine::start_with(host, input, output, shared, true)
            .or_else(|_| Engine::start_with(host, input, output, shared, false))
    }

    fn start_with(
        host: &Host,
        input: &str,
        output: &str,
        shared: &EngineShared,
        small_buffer: bool,
    ) -> Result<Engine, String> {
        let in_dev = find(host.input_devices().ok(), input)?;
        let out_dev = find(host.output_devices().ok(), output)?;
        let in_cfg = pick_config(&in_dev, true, small_buffer)?;
        let out_cfg = pick_config(&out_dev, false, small_buffer)?;

        let in_rate = in_cfg.sample_rate as f32;
        let (mut producer, mut consumer) = HeapRb::<f32>::new((in_rate * QUEUE_SECONDS) as usize).split();
        let target_fill = (in_rate * TARGET_FILL_SECONDS) as usize;

        let in_channels = in_cfg.channels as usize;
        let meter = shared.meter.clone();
        let input_stream = in_dev
            .build_input_stream(
                in_cfg,
                move |data: &[f32], _: &_| {
                    meter.push_block(data);
                    for frame in data.chunks(in_channels) {
                        let _ = producer.try_push(downmix(frame));
                    }
                },
                on_error(shared),
                None,
            )
            .map_err(|e| e.to_string())?;

        let out_channels = out_cfg.channels as usize;
        let out_rate = out_cfg.sample_rate as f32;
        let params = shared.params.clone();
        let mut resampler = Resampler::new(in_rate, out_rate, target_fill);
        let mut chain = VoiceChain::new(out_rate);
        let output_stream = out_dev
            .build_output_stream(
                out_cfg,
                move |data: &mut [f32], _: &_| {
                    let p = params.load();
                    for frame in data.chunks_mut(out_channels) {
                        let y = chain.process(resampler.next(&mut consumer), &p);
                        frame.fill(y);
                    }
                },
                on_error(shared),
                None,
            )
            .map_err(|e| e.to_string())?;

        input_stream.play().map_err(|e| e.to_string())?;
        output_stream.play().map_err(|e| e.to_string())?;
        Ok(Engine {
            input: input.to_string(),
            output: output.to_string(),
            _input_stream: input_stream,
            _output_stream: output_stream,
        })
    }
}

fn find(devices: Option<impl Iterator<Item = Device>>, name: &str) -> Result<Device, String> {
    devices
        .and_then(|mut it| it.find(|d| d.to_string() == name))
        .ok_or_else(|| format!("Device \"{name}\" not found"))
}

/// f32 at 48 kHz with a small fixed buffer when the device allows it,
/// otherwise the device's default config.
fn pick_config(device: &Device, input: bool, small_buffer: bool) -> Result<StreamConfig, String> {
    let ranges: Vec<_> = if input {
        device.supported_input_configs().map(|it| it.collect()).unwrap_or_default()
    } else {
        device.supported_output_configs().map(|it| it.collect()).unwrap_or_default()
    };
    let preferred = ranges
        .into_iter()
        .filter(|r| r.sample_format() == SampleFormat::F32)
        .find_map(|r| r.try_with_sample_rate(PREFERRED_RATE));
    let supported = match preferred {
        Some(cfg) => cfg,
        None if input => device.default_input_config().map_err(|e| e.to_string())?,
        None => device.default_output_config().map_err(|e| e.to_string())?,
    };
    if supported.sample_format() != SampleFormat::F32 {
        return Err(format!("Unsupported sample format {}", supported.sample_format()));
    }
    let buffer_size = match supported.buffer_size() {
        SupportedBufferSize::Range { min, max } if small_buffer => {
            BufferSize::Fixed(PREFERRED_BUFFER_FRAMES.clamp(*min, *max))
        }
        _ => BufferSize::Default,
    };
    Ok(StreamConfig { buffer_size, ..supported.config() })
}

/// Only losing the device warrants a restart. Overloads (xruns) and other
/// transient reports are survivable glitches, not a reason to reopen devices.
pub fn is_fatal(kind: cpal::ErrorKind) -> bool {
    matches!(kind, cpal::ErrorKind::DeviceNotAvailable | cpal::ErrorKind::StreamInvalidated)
}

/// Stream errors (a device unplugged, for example) raise the `failed` flag
/// so the manager thread restarts the engine.
fn on_error(shared: &EngineShared) -> impl FnMut(cpal::Error) + Send + 'static {
    let failed = shared.failed.clone();
    move |err: cpal::Error| {
        if is_fatal(err.kind()) {
            failed.store(true, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::{choose_input, choose_output};
    use crate::dsp::Params;

    #[test]
    fn only_device_loss_restarts_engine() {
        use cpal::ErrorKind;
        assert!(is_fatal(ErrorKind::DeviceNotAvailable));
        assert!(is_fatal(ErrorKind::StreamInvalidated));
        assert!(!is_fatal(ErrorKind::Xrun));
        assert!(!is_fatal(ErrorKind::DeviceChanged));
    }

    #[test]
    #[ignore = "needs real audio hardware"]
    fn engine_runs_on_real_devices() {
        let host = cpal::default_host();
        let input = choose_input(&list_inputs(&host), None).expect("an input device");
        let output = choose_output(&list_outputs(&host), None).expect("an output device");
        let shared = EngineShared {
            // Silent output so the smoke test cannot howl through the speaker.
            params: Arc::new(SharedParams::new(Params { out_gain: 0.0, ..Params::default() })),
            meter: Arc::new(LevelMeter::default()),
            failed: Arc::new(AtomicBool::new(false)),
        };
        let engine = Engine::start(&host, &input, &output, &shared).expect("engine starts");
        std::thread::sleep(std::time::Duration::from_secs(2));
        println!("input={} output={} level={} failed={}", engine.input, engine.output, shared.meter.take(), shared.failed.load(Ordering::Relaxed));
        assert!(!shared.failed.load(Ordering::Relaxed));
    }
}
