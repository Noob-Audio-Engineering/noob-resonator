//! The pitch oscillator.
//!
//! Seven shapes, because that is the set the device this one answers has, and
//! the ordinals are the ones recovered from its own binary rather than
//! guessed from its manual's prose.
//!
//! It is evaluated **once per block** and the resonator ramps its
//! coefficients across the block to meet it. Holding a coefficient still for
//! a whole block and stepping it at the boundary was measured to leave a
//! block-rate sideband at −60 dB on a modulated partial at 128 samples;
//! ramping across the block was worth a flat 10.6 dB at every block size, for
//! three adds per mode per sample.
//!
//! **The published trick for making this free does not transfer here, and
//! that is worth saying rather than quietly not doing.** For a *single*
//! resonator, retuning by a fixed step is one rotation of the coefficient
//! pair and needs no sine or cosine at all. For a *bank*, a change of
//! fundamental moves every mode by a different angle — mode `k` turns by
//! `k` times as much — so the rotation is per-mode and costs exactly what the
//! transcendental it replaced did. `docs/BENCHMARK.md` prints what a retune
//! of the whole bank costs instead of assuming it away.

/// The shapes, in the order the device this one answers stores them.
pub const LFO_NAMES: [&str; 7] = [
    "Sine",
    "Square",
    "Triangle",
    "Ramp Up",
    "Ramp Down",
    "S&H",
    "Random Ramp",
];

/// One oscillator, shared by both channels.
#[derive(Clone, Debug)]
pub struct Lfo {
    phase: f32,
    rng: u32,
    /// The held value of the stepped shape, and the endpoints the smooth one
    /// is travelling between.
    held: f32,
    from: f32,
    to: f32,
}

impl Default for Lfo {
    fn default() -> Self {
        Lfo {
            phase: 0.0,
            rng: 0x9E37_79B9,
            held: 0.0,
            from: 0.0,
            to: 0.0,
        }
    }
}

impl Lfo {
    pub fn reset(&mut self) {
        self.phase = 0.0;
        self.held = 0.0;
        self.from = 0.0;
        self.to = 0.0;
    }

    fn next_random(&mut self) -> f32 {
        // xorshift32, which is plenty for a modulation source and costs
        // nothing.
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng >> 8) as f32 / 8_388_608.0 - 1.0
    }

    /// Move the oscillator on by `samples` and re-roll the noise shapes if it
    /// wrapped.
    pub fn advance(&mut self, rate_hz: f32, sr: f32, samples: usize) {
        let step = rate_hz.max(0.0) * samples as f32 / sr.max(1.0);
        self.phase += step;
        while self.phase >= 1.0 {
            self.phase -= 1.0;
            self.held = self.next_random();
            self.from = self.to;
            self.to = self.next_random();
        }
    }

    /// The oscillator's value in −1 … 1.
    ///
    /// `offset` is the stereo phase offset in turns. The two noise shapes
    /// ignore it, exactly as the device this one answers documents for its
    /// own: a phase offset on a random sequence is not a phase offset, it is
    /// a different sequence.
    pub fn value(&self, shape: usize, offset: f32) -> f32 {
        let p = (self.phase + offset).rem_euclid(1.0);
        match shape {
            0 => (std::f32::consts::TAU * p).sin(),
            1 => {
                if p < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            2 => 1.0 - 4.0 * (p - 0.5).abs(),
            3 => 2.0 * p - 1.0,
            4 => 1.0 - 2.0 * p,
            5 => self.held,
            _ => self.from + (self.to - self.from) * self.phase,
        }
    }
}
