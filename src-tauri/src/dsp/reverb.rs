//! Mono Freeverb (Jezar's tunings): eight damped combs into four allpasses.

const COMB_TUNINGS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASS_TUNINGS: [usize; 4] = [556, 441, 341, 225];
const ROOM_FEEDBACK: f32 = 0.84;
const DAMPING: f32 = 0.2;
const ALLPASS_GAIN: f32 = 0.5;
const INPUT_GAIN: f32 = 0.015;
const WET_GAIN: f32 = 3.0;

struct Comb {
    buf: Vec<f32>,
    idx: usize,
    store: f32,
}

impl Comb {
    fn process(&mut self, x: f32) -> f32 {
        let out = self.buf[self.idx];
        self.store = out * (1.0 - DAMPING) + self.store * DAMPING;
        self.buf[self.idx] = x + self.store * ROOM_FEEDBACK;
        self.idx = (self.idx + 1) % self.buf.len();
        out
    }
}

struct Allpass {
    buf: Vec<f32>,
    idx: usize,
}

impl Allpass {
    fn process(&mut self, x: f32) -> f32 {
        let delayed = self.buf[self.idx];
        self.buf[self.idx] = x + delayed * ALLPASS_GAIN;
        self.idx = (self.idx + 1) % self.buf.len();
        delayed - x
    }
}

pub struct Reverb {
    combs: Vec<Comb>,
    allpasses: Vec<Allpass>,
}

impl Reverb {
    pub fn new(sample_rate: f32) -> Reverb {
        let scale = |n: usize| ((n as f32 * sample_rate / 44_100.0) as usize).max(1);
        Reverb {
            combs: COMB_TUNINGS
                .iter()
                .map(|&n| Comb { buf: vec![0.0; scale(n)], idx: 0, store: 0.0 })
                .collect(),
            allpasses: ALLPASS_TUNINGS
                .iter()
                .map(|&n| Allpass { buf: vec![0.0; scale(n)], idx: 0 })
                .collect(),
        }
    }

    /// Returns only the wet reverb for input `x`.
    pub fn process(&mut self, x: f32) -> f32 {
        let input = x * INPUT_GAIN;
        let summed: f32 = self.combs.iter_mut().map(|c| c.process(input)).sum();
        let out = self.allpasses.iter_mut().fold(summed, |acc, a| a.process(acc));
        out * WET_GAIN
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    fn energy(v: &[f32]) -> f32 {
        v.iter().map(|x| x * x).sum()
    }

    #[test]
    fn reverb_tail_decays() {
        let mut r = Reverb::new(SR);
        let out: Vec<f32> = (0..96_000).map(|n| r.process(if n == 0 { 1.0 } else { 0.0 })).collect();
        let early = energy(&out[..24_000]);
        let late = energy(&out[72_000..96_000]);
        assert!(early > 0.0);
        assert!(late < 0.01 * early, "early {early} late {late}");
    }
}
