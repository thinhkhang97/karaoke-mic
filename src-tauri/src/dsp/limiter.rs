//! Soft clipper: transparent below the knee, tanh above it, never past full scale.

const KNEE: f32 = 0.5;

pub fn soft_clip(x: f32) -> f32 {
    let mag = x.abs();
    if mag <= KNEE {
        return x;
    }
    let headroom = 1.0 - KNEE;
    (KNEE + headroom * ((mag - KNEE) / headroom).tanh()).copysign(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soft_clip_bounded_and_transparent() {
        assert!((soft_clip(0.3) - 0.3).abs() < 0.02);
        for x in [-100.0, -2.0, 2.0, 100.0] {
            assert!(soft_clip(x).abs() <= 1.0, "{x}");
        }
        assert!(soft_clip(2.0) > soft_clip(0.6));
    }
}
