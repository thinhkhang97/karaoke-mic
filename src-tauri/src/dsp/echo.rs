//! Feedback delay line with a low-pass filter in the loop, so every repeat
//! sounds a little darker, like the echo on a karaoke-bar mixer.

const BUFFER_SECONDS: f32 = 0.6;
const LOWPASS_HZ: f32 = 3500.0;
/// Fraction of the remaining distance the delay moves toward its target each sample.
const DELAY_SMOOTHING: f32 = 0.0015;
/// Upper bound on how fast the read head may move, in samples per sample.
/// Keeps a DELAY fader move from turning into a click or a big pitch jump.
const MAX_DELAY_SLEW: f32 = 0.25;

pub struct Echo {
    buf: Vec<f32>,
    write: usize,
    delay: Option<f32>,
    lp: f32,
    lp_coef: f32,
    sample_rate: f32,
}

impl Echo {
    pub fn new(sample_rate: f32) -> Echo {
        let len = (sample_rate * BUFFER_SECONDS) as usize;
        Echo {
            buf: vec![0.0; len],
            write: 0,
            delay: None,
            lp: 0.0,
            lp_coef: 1.0 - (-2.0 * std::f32::consts::PI * LOWPASS_HZ / sample_rate).exp(),
            sample_rate,
        }
    }

    /// Returns only the wet echo for input `x`.
    pub fn process(&mut self, x: f32, delay_ms: f32, feedback: f32) -> f32 {
        let len = self.buf.len();
        let target = (delay_ms * 0.001 * self.sample_rate).clamp(1.0, (len - 2) as f32);
        let delay = match self.delay {
            None => target,
            Some(d) => d + ((target - d) * DELAY_SMOOTHING).clamp(-MAX_DELAY_SLEW, MAX_DELAY_SLEW),
        };
        self.delay = Some(delay);

        let tap = self.read(delay);
        self.lp += self.lp_coef * (tap - self.lp);
        self.buf[self.write] = x + self.lp * feedback;
        self.write = (self.write + 1) % len;
        tap
    }

    /// Linear-interpolated read `delay` samples behind the write head.
    fn read(&self, delay: f32) -> f32 {
        let len = self.buf.len();
        let pos = self.write as f32 + len as f32 - delay;
        let i = pos.floor() as usize;
        let frac = pos - pos.floor();
        let a = self.buf[i % len];
        let b = self.buf[(i + 1) % len];
        a + (b - a) * frac
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    fn run(echo: &mut Echo, input: &[f32], delay_ms: f32, fb: f32) -> Vec<f32> {
        input.iter().map(|&x| echo.process(x, delay_ms, fb)).collect()
    }

    fn impulse(len: usize) -> Vec<f32> {
        let mut v = vec![0.0; len];
        v[0] = 1.0;
        v
    }

    fn peak_near(out: &[f32], centre: usize, radius: usize) -> f32 {
        out[centre - radius..centre + radius].iter().fold(0.0f32, |m, v| m.max(v.abs()))
    }

    #[test]
    fn echo_first_repeat_at_delay() {
        let out = run(&mut Echo::new(SR), &impulse(20_000), 230.0, 0.0);
        let (idx, _) = out
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
            .unwrap();
        assert!((idx as i64 - 11_040).abs() <= 2, "peak at {idx}");
        assert!(out[11_100..].iter().all(|v| v.abs() < 1e-3));
    }

    #[test]
    fn echo_repeats_decay_with_feedback() {
        let d = 11_040;
        let out = run(&mut Echo::new(SR), &impulse(d * 5 + 200), 230.0, 0.45);
        let peaks: Vec<f32> = (1..=4).map(|k| peak_near(&out, k * d, 60)).collect();
        for w in peaks.windows(2) {
            assert!(w[1] < w[0], "{peaks:?}");
            assert!(w[1] <= 0.45 * w[0] + 1e-6, "{peaks:?}");
        }
    }

    #[test]
    fn delay_change_is_smoothed() {
        let mut echo = Echo::new(SR);
        let sine = |n: usize| 0.5 * (2.0 * std::f32::consts::PI * 440.0 * n as f32 / SR).sin();
        let warm = 30_000;
        for n in 0..warm {
            echo.process(sine(n), 230.0, 0.0);
        }
        let after: Vec<f32> = (warm..warm + 2_400).map(|n| echo.process(sine(n), 60.0, 0.0)).collect();
        let max_step = after.windows(2).fold(0.0f32, |m, w| m.max((w[1] - w[0]).abs()));
        assert!(max_step < 0.2, "max step {max_step}");
    }
}
