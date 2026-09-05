//! Demo signals for the standalone. None of this is reachable from the
//! plug-in, which is fed by its host.
//!
//! The set is chosen for the one thing a resonator needs from its input:
//! **energy where its modes are**. A modal bank can only ring what the input
//! contains at each mode's frequency, and no amount of mode count fixes a
//! signal that has nothing up there — so the sources here are mostly
//! broadband.
//!
//! An **impulse train** is the honest probe: a single-sample click is flat to
//! Nyquist, so what comes back is the object's own spectrum and nothing
//! else. **Clicks** are the same thing at a musical rate, so the tail can be
//! heard between them. **Noise bursts** excite everything at once and are
//! what shows the difference between a bank that stops and one that does not.
//! A **saw** is programme-like: harmonic, so it picks out the partials it
//! happens to line up with and leaves the rest silent, which is the honest
//! failure case. And a **sine** is the null: put one into a resonator tuned
//! elsewhere and almost nothing should come out.

/// Names of the sources, in parameter order.
pub const SOURCE_NAMES: [&str; 5] = ["Impulses", "Clicks", "Noise Burst", "Saw", "Sine"];

/// A phase accumulator, an envelope and a noise generator.
pub struct Source {
    phase: f32,
    /// Where we are between one strike and the next, in samples.
    since: f32,
    env: f32,
    rng: u32,
}

impl Source {
    pub fn new(seed: u32) -> Self {
        Source {
            phase: 0.0,
            since: 0.0,
            env: 0.0,
            rng: seed | 1,
        }
    }

    #[inline]
    fn noise(&mut self) -> f32 {
        // Xorshift32: cheap, deterministic, and good enough for a test signal.
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    /// One sample of source `kind` at `hz`, at unit amplitude.
    ///
    /// `hz` is the **strike rate** for the three struck sources and the pitch
    /// for the two tonal ones, because a resonator is an effect and what its
    /// input needs to be is a rhythm rather than a note.
    pub fn next(&mut self, kind: usize, hz: f32, sr: f32) -> f32 {
        let tau = std::f32::consts::TAU;
        match kind {
            // One sample high, the rest silent, twice a second: flat to
            // Nyquist, so what comes back is the object.
            0 => {
                let period = (sr / 2.0).max(1.0);
                self.since += 1.0;
                if self.since >= period {
                    self.since = 0.0;
                    return 1.0;
                }
                0.0
            }
            // The same, at the rate the frequency control asks for, with a
            // couple of milliseconds of decay so it reads as a tap rather
            // than a tick.
            1 => {
                let period = (sr / hz.clamp(0.1, 40.0)).max(1.0);
                self.since += 1.0;
                if self.since >= period {
                    self.since = 0.0;
                    self.env = 1.0;
                }
                let y = self.env;
                self.env *= (-1.0 / (0.002 * sr)).exp();
                y
            }
            // Fifty milliseconds of noise per strike: everything at once, so
            // every mode gets something whether or not the input lines up
            // with it.
            2 => {
                let period = (sr / hz.clamp(0.1, 40.0)).max(1.0);
                self.since += 1.0;
                if self.since >= period {
                    self.since = 0.0;
                    self.env = 1.0;
                }
                let y = self.env * self.noise();
                self.env *= (-1.0 / (0.05 * sr)).exp();
                y
            }
            // Harmonic, so it drives the partials it lines up with and
            // leaves the others silent. That is the failure case and it is
            // worth being able to hear.
            3 => {
                self.phase += (hz / sr).clamp(0.0, 0.49);
                if self.phase >= 1.0 {
                    self.phase -= 1.0;
                }
                2.0 * self.phase - 1.0
            }
            _ => {
                self.phase += (hz / sr).clamp(0.0, 0.49);
                if self.phase >= 1.0 {
                    self.phase -= 1.0;
                }
                (self.phase * tau).sin()
            }
        }
    }
}

impl Default for Source {
    fn default() -> Self {
        Source::new(0x9E37_79B9)
    }
}
