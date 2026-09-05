//! The two small filters the rest of the engine is built out of.
//!
//! Both are in Zavalishin's topology-preserving transform form, which is the
//! one whose state variables are integrator memories rather than delayed
//! signal samples. That matters here for the same reason it matters to the
//! resonators: retuning a trapezoidal integrator does not disturb the energy
//! it is holding, so a filter whose frequency is being swept does not click.

/// A one-pole, used as the low half of a shelf.
#[derive(Default, Clone, Copy, Debug)]
pub struct OnePole {
    g: f32,
    s: f32,
}

impl OnePole {
    pub fn set(&mut self, hz: f32, sr: f32) {
        let t = (std::f32::consts::PI * hz.clamp(1.0, sr * 0.49) / sr).tan();
        self.g = t / (1.0 + t);
    }

    #[inline]
    pub fn lp(&mut self, x: f32) -> f32 {
        let v = (x - self.s) * self.g;
        let y = v + self.s;
        self.s = y + v;
        y
    }

    pub fn reset(&mut self) {
        self.s = 0.0;
    }
}

/// A state-variable filter with all three outputs available from one pass.
#[derive(Default, Clone, Copy, Debug)]
pub struct Svf {
    g: f32,
    k: f32,
    a1: f32,
    a2: f32,
    a3: f32,
    ic1: f32,
    ic2: f32,
}

impl Svf {
    /// Set the corner and the damping. `q` is the quality factor; a
    /// Butterworth second-order response is `q = 1/√2`.
    pub fn set(&mut self, hz: f32, q: f32, sr: f32) {
        self.g = (std::f32::consts::PI * hz.clamp(1.0, sr * 0.49) / sr).tan();
        self.k = 1.0 / q.clamp(0.05, 100.0);
        self.a1 = 1.0 / (1.0 + self.g * (self.g + self.k));
        self.a2 = self.g * self.a1;
        self.a3 = self.g * self.a2;
    }

    /// One sample, returning `(lowpass, bandpass, highpass)`.
    #[inline]
    pub fn run(&mut self, x: f32) -> (f32, f32, f32) {
        let v3 = x - self.ic2;
        let v1 = self.a1 * self.ic1 + self.a2 * v3;
        let v2 = self.ic2 + self.a2 * self.ic1 + self.a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        (v2, v1, x - self.k * v1 - v2)
    }

    #[inline]
    pub fn bp(&mut self, x: f32) -> f32 {
        self.run(x).1
    }

    #[inline]
    pub fn hp(&mut self, x: f32) -> f32 {
        self.run(x).2
    }

    pub fn reset(&mut self) {
        self.ic1 = 0.0;
        self.ic2 = 0.0;
    }
}

/// The quality factor of a band-pass of a given width in octaves.
///
/// `Q = √(2^BW) / (2^BW − 1)`, the standard relation between a geometrically
/// symmetric bandwidth and a two-pole section's damping. The device this one
/// answers publishes its filter's width as a bare 0.5 … 9 with no unit
/// anywhere on disk; octaves is the reading its range and its display name
/// support, and it is the unit ours is calibrated in and prints.
pub fn q_from_octaves(bw: f32) -> f32 {
    let a = 2f32.powf(bw.clamp(0.05, 12.0));
    (a.sqrt() / (a - 1.0)).clamp(0.05, 100.0)
}

/// A zero-latency peak limiter.
///
/// The device this one answers reports **64 samples of latency
/// unconditionally**, and its manual attributes them to its built-in
/// limiter. A driven modal bank does need a limiter — a single mode with a
/// three-second decay has +80 dB of gain at its own frequency, and sixty-four
/// of them in parallel have more — but it does not need lookahead to have
/// one, and lookahead is what costs the samples.
///
/// So this one applies its gain **instantly** on the way down and releases
/// slowly, which is a real trade rather than a free lunch: it can never
/// exceed the ceiling and it can distort a fast transient. For a resonator's
/// output, which is a sum of decaying sinusoids rather than a drum hit, that
/// is the right side of the trade, and the plug-in reports zero latency and
/// means it.
#[derive(Clone, Copy, Debug)]
pub struct Limiter {
    gain: f32,
    release: f32,
    ceiling: f32,
    /// The lowest gain applied since the last read, linear.
    worst: f32,
}

impl Default for Limiter {
    fn default() -> Self {
        Limiter {
            gain: 1.0,
            release: 0.999,
            ceiling: 1.0,
            worst: 1.0,
        }
    }
}

impl Limiter {
    /// 100 ms of release, which is slow enough not to modulate a decaying
    /// tail and fast enough to recover between strikes.
    pub fn set(&mut self, ceiling_db: f32, sr: f32) {
        self.ceiling = 10f32.powf(ceiling_db.clamp(-60.0, 0.0) / 20.0);
        self.release = (-1.0 / (0.1 * sr.max(1.0))).exp();
    }

    pub fn reset(&mut self) {
        self.gain = 1.0;
        self.worst = 1.0;
    }

    #[inline]
    pub fn run(&mut self, l: f32, r: f32) -> (f32, f32) {
        let peak = l.abs().max(r.abs());
        let want = if peak > self.ceiling {
            self.ceiling / peak
        } else {
            1.0
        };
        if want < self.gain {
            self.gain = want;
        } else {
            self.gain += (want - self.gain) * (1.0 - self.release);
        }
        if self.gain < self.worst {
            self.worst = self.gain;
        }
        (l * self.gain, r * self.gain)
    }

    /// The worst gain reduction since the last call, in dB (never positive),
    /// and start again.
    pub fn take_reduction_db(&mut self) -> f32 {
        let w = 20.0 * self.worst.clamp(1e-6, 1.0).log10();
        self.worst = 1.0;
        w
    }
}
