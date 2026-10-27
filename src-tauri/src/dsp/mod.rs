//! Voice DSP: echo, reverb, limiter and the chain that combines them.

pub mod chain;
pub mod echo;
pub mod limiter;
pub mod reverb;

use serde::{Deserialize, Serialize};

pub const MIC_GAIN_MAX: f32 = 2.0;
pub const DELAY_MS_MIN: f32 = 60.0;
pub const DELAY_MS_MAX: f32 = 500.0;
pub const REPEAT_MAX: f32 = 0.85;

/// One snapshot of every fader plus mute. Mirrors the TypeScript `Params`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Params {
    pub mic_gain: f32,
    pub echo: f32,
    pub delay_ms: f32,
    pub repeat: f32,
    pub reverb: f32,
    pub out_gain: f32,
    pub muted: bool,
}

impl Params {
    /// Returns a copy with every value forced into its fader range.
    pub fn clamped(self) -> Params {
        Params {
            mic_gain: clamp(self.mic_gain, 0.0, MIC_GAIN_MAX),
            echo: clamp(self.echo, 0.0, 1.0),
            delay_ms: clamp(self.delay_ms, DELAY_MS_MIN, DELAY_MS_MAX),
            repeat: clamp(self.repeat, 0.0, REPEAT_MAX),
            reverb: clamp(self.reverb, 0.0, 1.0),
            out_gain: clamp(self.out_gain, 0.0, 1.0),
            muted: self.muted,
        }
    }
}

/// Like `f32::clamp`, but a NaN becomes the minimum instead of propagating.
fn clamp(v: f32, min: f32, max: f32) -> f32 {
    if v.is_nan() { min } else { v.clamp(min, max) }
}

impl Default for Params {
    fn default() -> Self {
        Params {
            mic_gain: 1.0,
            echo: 0.35,
            delay_ms: 230.0,
            repeat: 0.45,
            reverb: 0.25,
            out_gain: 0.8,
            muted: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamped_forces_ranges() {
        let p = Params {
            mic_gain: 5.0,
            echo: -1.0,
            delay_ms: 1000.0,
            repeat: 1.0,
            reverb: 2.0,
            out_gain: f32::NAN,
            muted: true,
        }
        .clamped();
        assert_eq!(p.mic_gain, 2.0);
        assert_eq!(p.echo, 0.0);
        assert_eq!(p.delay_ms, 500.0);
        assert_eq!(p.repeat, 0.85);
        assert_eq!(p.reverb, 1.0);
        assert_eq!(p.out_gain, 0.0);
        assert!(p.muted);
        assert_eq!(Params { delay_ms: 10.0, ..Params::default() }.clamped().delay_ms, 60.0);
    }
}
