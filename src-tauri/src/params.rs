//! Lock-free hand-off of fader values and the input level between the UI
//! thread and the audio callbacks.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use crate::dsp::Params;

pub struct SharedParams {
    mic_gain: AtomicU32,
    echo: AtomicU32,
    delay_ms: AtomicU32,
    repeat: AtomicU32,
    reverb: AtomicU32,
    out_gain: AtomicU32,
    muted: AtomicBool,
}

impl SharedParams {
    pub fn new(p: Params) -> SharedParams {
        let bits = |v: f32| AtomicU32::new(v.to_bits());
        SharedParams {
            mic_gain: bits(p.mic_gain),
            echo: bits(p.echo),
            delay_ms: bits(p.delay_ms),
            repeat: bits(p.repeat),
            reverb: bits(p.reverb),
            out_gain: bits(p.out_gain),
            muted: AtomicBool::new(p.muted),
        }
    }

    pub fn store(&self, p: Params) {
        let set = |a: &AtomicU32, v: f32| a.store(v.to_bits(), Ordering::Relaxed);
        set(&self.mic_gain, p.mic_gain);
        set(&self.echo, p.echo);
        set(&self.delay_ms, p.delay_ms);
        set(&self.repeat, p.repeat);
        set(&self.reverb, p.reverb);
        set(&self.out_gain, p.out_gain);
        self.muted.store(p.muted, Ordering::Relaxed);
    }

    pub fn load(&self) -> Params {
        let get = |a: &AtomicU32| f32::from_bits(a.load(Ordering::Relaxed));
        Params {
            mic_gain: get(&self.mic_gain),
            echo: get(&self.echo),
            delay_ms: get(&self.delay_ms),
            repeat: get(&self.repeat),
            reverb: get(&self.reverb),
            out_gain: get(&self.out_gain),
            muted: self.muted.load(Ordering::Relaxed),
        }
    }
}

/// Peak level of the most recent input blocks, read and reset by the UI.
#[derive(Default)]
pub struct LevelMeter {
    peak: AtomicU32,
}

impl LevelMeter {
    pub fn push_block(&self, samples: &[f32]) {
        let peak = samples.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        // For non-negative floats the bit patterns sort the same way as the values.
        self.peak.fetch_max(peak.to_bits(), Ordering::Relaxed);
    }

    pub fn take(&self) -> f32 {
        f32::from_bits(self.peak.swap(0, Ordering::Relaxed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_params_roundtrip() {
        let p = Params { mic_gain: 1.5, echo: 0.1, delay_ms: 321.0, repeat: 0.2, reverb: 0.3, out_gain: 0.4, muted: true };
        let shared = SharedParams::new(Params::default());
        shared.store(p);
        assert_eq!(shared.load(), p);
    }

    #[test]
    fn level_meter_keeps_peak_until_taken() {
        let m = LevelMeter::default();
        m.push_block(&[0.1, -0.7, 0.3]);
        m.push_block(&[0.2]);
        assert_eq!(m.take(), 0.7);
        assert_eq!(m.take(), 0.0);
    }
}
