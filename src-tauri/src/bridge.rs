//! Carries mic samples from the input device's clock to the output device's
//! clock: a linear-interpolating resampler whose ratio is nudged so the queue
//! between the two streams stays near a small target fill.

use std::collections::VecDeque;

use ringbuf::HeapCons;
use ringbuf::traits::{Consumer, Observer};

/// Largest speed-up or slow-down used to correct the queue fill (1 %).
const MAX_CORRECTION: f32 = 0.01;
/// How strongly a fill error turns into a ratio correction.
const CORRECTION_GAIN: f32 = 0.01;
/// Smoothing for the fill estimate, so block-sized jumps don't wobble the pitch.
const FILL_SMOOTHING: f32 = 0.001;
/// A fill this many times the target means a stall left a backlog: drop it
/// instead of slowly speeding up, so the voice never lags behind the singer.
const BACKLOG_FACTOR: f32 = 3.0;
/// The target fill doubles after each underrun (large device blocks) up to this.
const MAX_TARGET_SECONDS: f32 = 0.1;

pub trait SampleSource {
    fn available(&self) -> usize;
    fn pop(&mut self) -> Option<f32>;
}

impl SampleSource for HeapCons<f32> {
    fn available(&self) -> usize {
        self.occupied_len()
    }

    fn pop(&mut self) -> Option<f32> {
        self.try_pop()
    }
}

impl SampleSource for VecDeque<f32> {
    fn available(&self) -> usize {
        self.len()
    }

    fn pop(&mut self) -> Option<f32> {
        self.pop_front()
    }
}

/// Mono sample from one interleaved frame.
pub fn downmix(frame: &[f32]) -> f32 {
    if frame.is_empty() {
        return 0.0;
    }
    frame.iter().sum::<f32>() / frame.len() as f32
}

pub struct Resampler {
    base_ratio: f32,
    target_fill: f32,
    max_target_fill: f32,
    fill_estimate: f32,
    prev: f32,
    curr: f32,
    frac: f32,
    primed: bool,
}

impl Resampler {
    pub fn new(in_rate: f32, out_rate: f32, target_fill: usize) -> Resampler {
        Resampler {
            base_ratio: in_rate / out_rate,
            target_fill: target_fill.max(2) as f32,
            max_target_fill: (in_rate * MAX_TARGET_SECONDS).max(target_fill as f32),
            fill_estimate: 0.0,
            prev: 0.0,
            curr: 0.0,
            frac: 0.0,
            primed: false,
        }
    }

    /// Next output sample. Returns silence until the queue has reached the
    /// target fill, and again after an underrun until it refills.
    pub fn next(&mut self, src: &mut impl SampleSource) -> f32 {
        if !self.primed && !self.prime(src) {
            return 0.0;
        }
        if src.available() as f32 > self.target_fill * BACKLOG_FACTOR {
            self.drop_to_target(src);
        }
        while self.frac >= 1.0 {
            match src.pop() {
                Some(v) => {
                    self.prev = self.curr;
                    self.curr = v;
                    self.frac -= 1.0;
                }
                None => {
                    self.primed = false;
                    self.target_fill = (self.target_fill * 2.0).min(self.max_target_fill);
                    return 0.0;
                }
            }
        }
        let out = self.prev + (self.curr - self.prev) * self.frac;
        self.frac += self.current_ratio(src.available());
        out
    }

    fn prime(&mut self, src: &mut impl SampleSource) -> bool {
        if (src.available() as f32) < self.target_fill {
            return false;
        }
        self.drop_to_target(src);
        self.prev = src.pop().unwrap_or(0.0);
        self.curr = src.pop().unwrap_or(0.0);
        self.frac = 0.0;
        self.fill_estimate = src.available() as f32;
        self.primed = true;
        true
    }

    fn drop_to_target(&mut self, src: &mut impl SampleSource) {
        while src.available() as f32 > self.target_fill {
            src.pop();
        }
        self.fill_estimate = self.target_fill;
    }

    fn current_ratio(&mut self, available: usize) -> f32 {
        self.fill_estimate += FILL_SMOOTHING * (available as f32 - self.fill_estimate);
        let error = (self.fill_estimate - self.target_fill) / self.target_fill;
        self.base_ratio * (1.0 + (error * CORRECTION_GAIN).clamp(-MAX_CORRECTION, MAX_CORRECTION))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downmix_averages() {
        assert!((downmix(&[0.2, 0.4]) - 0.3).abs() < 1e-6);
        assert_eq!(downmix(&[0.5]), 0.5);
        assert_eq!(downmix(&[]), 0.0);
    }

    #[test]
    fn resampler_same_rate_passthrough() {
        let mut src: VecDeque<f32> = (0..100).map(|n| n as f32).collect();
        let mut r = Resampler::new(48_000.0, 48_000.0, 100);
        let out: Vec<f32> = (100..600)
            .map(|n| {
                src.push_back(n as f32);
                r.next(&mut src)
            })
            .collect();
        // Same rate, a live stream at the target fill: output walks the input one sample at a time.
        for w in out.windows(2) {
            assert!((w[1] - w[0] - 1.0).abs() < 0.02, "{w:?}");
        }
    }

    #[test]
    fn resampler_underrun_outputs_silence() {
        let mut src: VecDeque<f32> = VecDeque::new();
        let mut r = Resampler::new(48_000.0, 44_100.0, 480);
        assert_eq!(r.next(&mut src), 0.0);
        src.extend([0.5; 10]);
        assert_eq!(r.next(&mut src), 0.0, "waits for target fill before playing");
    }

    fn constant(n: usize) -> VecDeque<f32> {
        std::iter::repeat_n(1.0, n).collect()
    }

    #[test]
    fn prime_discards_backlog() {
        let mut src = constant(48_000);
        let mut r = Resampler::new(48_000.0, 48_000.0, 480);
        r.next(&mut src);
        assert!(src.len() <= 482, "fill after priming: {}", src.len());
    }

    #[test]
    fn trims_backlog_after_stall() {
        let mut src = constant(480);
        let mut r = Resampler::new(48_000.0, 48_000.0, 480);
        for _ in 0..100 {
            src.extend([1.0; 100]);
            for _ in 0..100 {
                r.next(&mut src);
            }
        }
        // The output stalls while the mic keeps recording: half a second piles up.
        src.extend(std::iter::repeat_n(1.0, 24_000));
        r.next(&mut src);
        assert!(src.len() <= 3 * 480, "fill after stall: {}", src.len());
    }

    #[test]
    fn underruns_grow_target_until_large_blocks_fit() {
        // A Bluetooth-style output that pulls 1024 frames per callback.
        let mut src: VecDeque<f32> = VecDeque::new();
        let mut r = Resampler::new(48_000.0, 48_000.0, 480);
        let mut silent_in_last_100 = 0;
        for block in 0..300 {
            src.extend([1.0; 1024]);
            let out: Vec<f32> = (0..1024).map(|_| r.next(&mut src)).collect();
            if block >= 200 {
                silent_in_last_100 += out.iter().filter(|v| **v == 0.0).count();
            }
        }
        assert_eq!(silent_in_last_100, 0);
    }

    #[test]
    fn resampler_tracks_rate_mismatch_without_drift() {
        let (in_rate, out_rate, target) = (48_000.0f32, 44_100.0f32, 480usize);
        let mut src: VecDeque<f32> = VecDeque::new();
        let mut r = Resampler::new(in_rate, out_rate, target);
        let mut produced = 0usize;
        let mut last_second = Vec::new();
        let steps: usize = 100 * 600; // 10 ms steps for 10 minutes
        for step in 0..steps {
            // The mic clock runs 0.05 % fast: one extra sample every 20 blocks.
            let block = if step.is_multiple_of(20) { 481 } else { 480 };
            for _ in 0..block {
                let t = produced as f32 / in_rate;
                src.push_back((2.0 * std::f32::consts::PI * 1000.0 * t).sin());
                produced += 1;
            }
            let out: Vec<f32> = (0..441).map(|_| r.next(&mut src)).collect();
            if step >= 100 {
                let fill = src.len() as i64;
                assert!((fill - target as i64).abs() <= 2 * 480, "step {step}: fill {fill}");
            }
            if step >= steps - 100 {
                last_second.extend(out);
            }
        }
        let crossings = last_second.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count() as f32;
        assert!((crossings - 1000.0).abs() <= 5.0, "measured {crossings} Hz");
    }
}
