//! The full voice path: gain → dry + echo + reverb → output gain → mute → limiter.

use super::Params;
use super::echo::Echo;
use super::limiter::soft_clip;
use super::reverb::Reverb;

const MUTE_RAMP_SECONDS: f32 = 0.005;

pub struct VoiceChain {
    echo: Echo,
    reverb: Reverb,
    mute_gain: f32,
    mute_step: f32,
}

impl VoiceChain {
    pub fn new(sample_rate: f32) -> VoiceChain {
        VoiceChain {
            echo: Echo::new(sample_rate),
            reverb: Reverb::new(sample_rate),
            mute_gain: 1.0,
            mute_step: 1.0 / (MUTE_RAMP_SECONDS * sample_rate),
        }
    }

    pub fn process(&mut self, x: f32, p: &Params) -> f32 {
        let target = if p.muted { 0.0 } else { 1.0 };
        self.mute_gain = if self.mute_gain < target {
            (self.mute_gain + self.mute_step).min(target)
        } else {
            (self.mute_gain - self.mute_step).max(target)
        };

        let v = x * p.mic_gain;
        let echo = self.echo.process(v, p.delay_ms, p.repeat);
        let reverb = self.reverb.process(v);
        let mixed = v + p.echo * echo + p.reverb * reverb;
        soft_clip(mixed * p.out_gain * self.mute_gain)
    }
}

#[cfg(test)]
mod tests {
    use super::super::limiter::soft_clip;
    use super::*;

    const SR: f32 = 48_000.0;

    fn rms(v: &[f32]) -> f32 {
        (v.iter().map(|x| x * x).sum::<f32>() / v.len() as f32).sqrt()
    }

    #[test]
    fn chain_off_is_dry() {
        let p = Params { echo: 0.0, reverb: 0.0, mic_gain: 1.0, out_gain: 1.0, ..Params::default() };
        let mut c = VoiceChain::new(SR);
        for n in 0..4_800 {
            let x = 0.9 * (n as f32 * 0.05).sin();
            assert!((c.process(x, &p) - soft_clip(x)).abs() < 1e-6);
        }
    }

    #[test]
    fn chain_muted_is_silent() {
        let p = Params { muted: true, ..Params::default() };
        let mut c = VoiceChain::new(SR);
        let out: Vec<f32> = (0..4_800).map(|_| c.process(0.5, &p)).collect();
        assert!(out[240..].iter().all(|v| *v == 0.0));
    }

    #[test]
    fn chain_bounded_and_decays_at_max_settings() {
        let p = Params {
            mic_gain: 2.0,
            echo: 1.0,
            delay_ms: 500.0,
            repeat: 0.85,
            reverb: 1.0,
            out_gain: 1.0,
            muted: false,
        };
        let mut c = VoiceChain::new(SR);
        let sr = SR as usize;
        let square = |n: usize| if (n / 120).is_multiple_of(2) { 1.0 } else { -1.0 };
        let out: Vec<f32> = (0..sr * 31)
            .map(|n| c.process(if n < sr { square(n) } else { 0.0 }, &p))
            .collect();
        assert!(out.iter().all(|v| v.abs() <= 1.0 && v.is_finite()));
        let window = |s: usize| rms(&out[sr * s..sr * (s + 1)]);
        assert!(window(5) < window(2), "echo must keep decaying");
        assert!(window(30) < 0.01, "tail at 30 s: {}", window(30));
    }
}
