//! The waveguide: the Pipe and the Tube, where the object is only a boundary
//! and what rings is the air inside.
//!
//! A bar, a plate or a membrane vibrates as a solid. It has a material, a
//! stiffness, a strike point, mode shapes with nodal lines, and a per-mode
//! damping set by internal friction. **A pipe has none of that.** Applied
//! Acoustics say it plainly of their own engine: "standing waves in a tube do
//! not result from the vibrations of the walls of the tube but rather by
//! vibrations of the air column inside … The material of the tube is
//! therefore not a relevant parameter in that case."
//!
//! So this is not a mode bank with different ratios in it. It is two delay
//! lines and a reflection at each end, it costs the same whatever number of
//! harmonics come out, and there is no mode count to truncate.
//!
//! ## The sign at the far end is the whole difference
//!
//! A **closed** end is a rigid wall: air cannot move there, so it is a
//! pressure antinode and a pressure wave reflects with the **same** sign. An
//! **open** end vents to the atmosphere: pressure falls to ambient, so it is a
//! pressure node and the wave reflects **inverted**. That one sign decides
//! everything:
//!
//! | ends | round trip | series |
//! |---|---|---|
//! | open–open | `2ℓ` | all harmonics — a **Tube** |
//! | open–closed | `4ℓ` | odd harmonics only, fundamental an octave lower — a **Pipe** |
//!
//! It is why a stopped organ pipe sounds an octave below an open one of the
//! same length, and why a clarinet overblows a twelfth rather than an octave.
//!
//! ## Opening is a real termination, not a crossfade
//!
//! A partly open end is a hole with the mass of air in it, so its load
//! impedance is an inertance and its reflection coefficient is
//! `R = (jωM − Z_c)/(jωM + Z_c)` — magnitude one, and a phase that runs from
//! **open at low frequency to closed at high frequency**, because a small
//! hole cannot move enough air to relieve a fast pressure swing. That is a
//! **first-order allpass with a sign**, and `Opening` is its transition
//! frequency:
//!
//! ```text
//!   R(z) = −(a + z⁻¹)/(1 + a z⁻¹),      a = tan(½π(o − ½))
//! ```
//!
//! `o = 0` gives `a = −1` and `R ≡ +1`, a perfectly closed end; `o = 1` gives
//! `a = +1` and `R ≡ −1`, a perfectly open one; and everything between is one
//! filter rather than a blend of two spectra. The even partials fade in out of
//! nothing, the odd ones shift, and the fundamental climbs an octave — one
//! continuous physical process.
//!
//! **The tuning is held at the fundamental while that happens**, which means
//! the length changes rather than the pitch: a stopped pipe at 220 Hz is 39 cm
//! of air and an open one at the same note is 78. The engine publishes the
//! length it derived so the octave stays visible rather than being hidden by
//! the choice.
//!
//! ## Radius is wall loss and end correction, and both are laws
//!
//! Sound in a tube loses energy in a viscous and thermal boundary layer whose
//! thickness goes as `1/√f`, giving an attenuation `α ∝ √f / a` for bore
//! radius `a`. A wider bore has less wall area per unit volume, so it decays
//! **slower**, and because the loss rises as `√f` it also keeps its high
//! frequencies **longer** — which is exactly the behaviour Ableton describe
//! for their Radius control, with no inversion needed to explain it. Their own
//! engine's vendor says the opposite mechanism dominates and inverts their
//! control to compensate; both produce the same knob and disagree about the
//! object. Ours is the wall-loss reading, the radius is the physical radius in
//! millimetres, and turning it up makes the bore wider.
//!
//! The other half of the sentence — "at very large sizes, the fundamental
//! pitch of the resonator also changes" — is the end correction. An open end
//! does not reflect at the geometric end; the effective length is longer by
//! `ΔL = 0.6133·a` unflanged (Levine and Schwinger). On a narrow bore that is
//! nothing and on a fat one it is a real fraction of the length.
//!
//! ## What this deliberately is not
//!
//! A real clarinet or organ pipe is a **nonlinear exciter in a feedback loop
//! with the bore** — a reed, a lip, an air jet — and that coupling is what
//! lets a wind instrument self-oscillate and lock to a bore resonance. There
//! is none of it here and there should not be: this is a passive linear
//! resonator driven by whatever audio is put into it, so **its pipe will never
//! blow**. It rings like a tapped length of plastic pipe, because that is what
//! it is, and that is a design boundary rather than a gap.

use crate::dsp::damp::{self, Damping};

/// Speed of sound in air at 20 °C, m/s. Only the air columns use it, and
/// every length derived from it is published beside it.
pub const C_AIR: f32 = 343.0;

/// Levine and Schwinger's end correction for an unflanged open end, as a
/// multiple of the bore radius.
pub const END_CORRECTION: f32 = 0.6133;

/// The bore that `Decay` is quoted at, in millimetres. A wider one rings
/// longer in proportion, because the wall loss goes with the surface and the
/// volume it has to drain goes with the bore.
pub const RADIUS_REF_MM: f32 = 20.0;

/// Longest one-way delay, in samples. Twenty hertz at 192 kHz with the LFO an
/// octave down needs 9,600; this is the next power of two above it.
///
/// **A power of two on purpose**, so the circular buffers wrap with a mask.
/// A waveguide's cost is index arithmetic rather than signal processing: the
/// two loss filters are a handful of multiplies and everything else is
/// working out where in a buffer to read. Written with `%` it is six integer
/// divisions per sample, which was measured at three times the cost of the
/// whole rest of the loop.
const MAX_RAIL: usize = 16384;

/// The mask that wraps an index into [`MAX_RAIL`].
const RAIL_MASK: usize = MAX_RAIL - 1;

/// Shortest total loop, in samples. Below this the interpolator's four taps
/// and the two filters have nowhere to sit.
const MIN_LOOP: f32 = 12.0;

/// Resonances published on the modes stream.
pub const MAX_RESONANCES: usize = 64;

/// A minimal complex number, so the closed-form loop response can be written
/// the way it is derived.
#[derive(Clone, Copy)]
struct C(f32, f32);

impl C {
    fn mul(self, o: C) -> C {
        C(self.0 * o.0 - self.1 * o.1, self.0 * o.1 + self.1 * o.0)
    }
    fn add(self, o: C) -> C {
        C(self.0 + o.0, self.1 + o.1)
    }
    fn scale(self, k: f32) -> C {
        C(self.0 * k, self.1 * k)
    }
    fn div(self, o: C) -> C {
        let den = o.0 * o.0 + o.1 * o.1;
        if den <= 1e-30 {
            return C(0.0, 0.0);
        }
        C(
            (self.0 * o.0 + self.1 * o.1) / den,
            (self.1 * o.0 - self.0 * o.1) / den,
        )
    }
    fn abs(self) -> f32 {
        (self.0 * self.0 + self.1 * self.1).sqrt()
    }
    /// `e^{jθ}`.
    fn unit(theta: f32) -> C {
        let (s, c) = theta.sin_cos();
        C(c, s)
    }
}

/// What the far end of the column is doing.
///
/// The two limits are held **exactly** rather than as an allpass with its
/// coefficient pushed against a stop, because the closed limit is where the
/// allpass is at its stiffest: at `a = -0.9999` its phase is still 2.4
/// milliradians short of π at the second resonance, which is 3.4 cents of
/// mistuning on a stopped pipe. Special-casing the two ends costs one branch
/// per sample and removes both the error and the near-unit-circle pole that
/// caused it.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Far {
    /// A rigid wall: a pressure antinode, reflecting with the same sign.
    Closed,
    /// Vented to the atmosphere: a pressure node, reflecting inverted.
    Open,
    /// A hole with the mass of air in it: open below its own transition
    /// frequency and closed above it.
    Hole(f32),
}

/// A delay line with a fractional read.
///
/// **The rails carry pressure**, and every sign in this file is that
/// convention: a closed end is a pressure antinode and reflects with the same
/// sign, an open end is a pressure node and reflects inverted. Carrying
/// particle velocity instead inverts the entire series, so the choice has to
/// be stated where the samples are rather than left to be inferred from the
/// reflection coefficients.
///
/// Third-order Lagrange rather than linear interpolation, because this delay
/// sits inside a feedback loop: linear interpolation is a lowpass, and a
/// lowpass applied once per round trip becomes the loop's dominant loss and
/// takes the damping law away from the control that is supposed to set it.
/// Lagrange is also better behaved than an allpass when the length is moving,
/// which it is whenever the oscillator is on.
struct Rail {
    buf: Vec<f32>,
    w: usize,
}

impl Rail {
    fn new() -> Rail {
        Rail {
            buf: vec![0.0; MAX_RAIL],
            w: 0,
        }
    }

    fn clear(&mut self) {
        self.buf.fill(0.0);
        self.w = 0;
    }

    #[inline]
    fn advance(&mut self) {
        self.w = (self.w + 1) & RAIL_MASK;
    }

    /// Write at the input (delay zero), replacing whatever was there.
    #[inline]
    fn write(&mut self, x: f32) {
        self.buf[self.w] = x;
    }

    /// Add into the line at a fractional delay, for a source part-way along
    /// the column. Split linearly between the two neighbouring samples,
    /// which is exact in energy and only smooths the injection point by a
    /// fraction of a sample.
    #[inline]
    fn inject(&mut self, delay: f32, x: f32) {
        if x == 0.0 {
            return;
        }
        let d = delay.clamp(0.0, (MAX_RAIL - 2) as f32);
        let i = d.floor() as usize;
        let f = d - i as f32;
        let a = (self.w + MAX_RAIL - i) & RAIL_MASK;
        let b = (self.w + MAX_RAIL - i - 1) & RAIL_MASK;
        self.buf[a] += x * (1.0 - f);
        self.buf[b] += x * f;
    }

    /// Read at a fractional delay, third-order Lagrange.
    #[inline]
    fn read(&self, delay: f32) -> f32 {
        let d = delay.clamp(1.0, (MAX_RAIL - 3) as f32);
        let i = d.floor() as usize;
        let f = d - i as f32;
        let g = |k: usize| self.buf[(self.w + MAX_RAIL - k) & RAIL_MASK];
        let (y0, y1, y2, y3) = (g(i - 1), g(i), g(i + 1), g(i + 2));
        // Lagrange 3 about the interval [y1, y2].
        let c0 = y1;
        let c1 = 0.5 * (y2 - y0);
        let c2 = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
        let c3 = 0.5 * (y3 - y0) + 1.5 * (y1 - y2);
        c0 + f * (c1 + f * (c2 + f * c3))
    }
}

/// Everything the loop is built from, recomputed when a control moves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// The sounding fundamental, hertz.
    pub f0: f32,
    /// 0 stopped … 1 open.
    pub opening: f32,
    /// Bore radius, millimetres.
    pub radius_mm: f32,
    /// T60 at the fundamental for the reference bore, seconds.
    pub decay: f32,
    /// Spectral tilt in decibels per octave, applied to the excitation.
    pub tilt_db_oct: f32,
    /// Where the column is struck and where it is heard, as fractions of the
    /// length from the permanently open end.
    pub hit: f32,
    pub pos_l: f32,
    pub pos_r: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            f0: 220.0,
            opening: 0.0,
            radius_mm: RADIUS_REF_MM,
            decay: 2.0,
            tilt_db_oct: 0.0,
            hit: 0.2,
            pos_l: 0.3,
            pos_r: 0.7,
        }
    }
}

/// One resonance of the column, for the readouts.
#[derive(Clone, Copy, Debug, Default)]
pub struct Resonance {
    pub hz: f32,
    pub t60: f32,
    pub amp_l: f32,
    pub amp_r: f32,
    /// What it would reach with no comb from the contact points.
    pub bare: f32,
    /// Which resonance of the loop it is, one-based; the same index a
    /// per-mode edit addresses.
    pub n: u16,
}

/// The air column.
pub struct Guide {
    right: Rail,
    left: Rail,
    sr: f32,
    set: Settings,
    /// Half the round trip, in samples: the one-way trip the response
    /// algebra is written in terms of.
    half: f32,
    /// The two rails' own delays, which sum to the round trip. They are
    /// split so each lands near the middle of an interpolation interval,
    /// where third-order Lagrange is at its most accurate; they differ by
    /// less than a sample, so the contact positions they define are the same
    /// physical points to well inside the width of one.
    half_r: f32,
    half_l: f32,
    /// How many resonances are live.
    live: usize,
    /// The far end, and the allpass state it needs when it is a hole.
    far: Far,
    ap_x: f32,
    ap_y: f32,
    /// The round-trip loss: one pole, fitted to the damping law at two
    /// frequencies.
    loss_g: f32,
    loss_p: f32,
    loss_z: f32,
    /// Excitation scaling, so the loop's resonance gain does not run away.
    drive: f32,
    resonances: Vec<Resonance>,
    damping: Damping,
}

impl Guide {
    pub fn new(sr: f32) -> Guide {
        let mut g = Guide {
            right: Rail::new(),
            left: Rail::new(),
            sr,
            set: Settings::default(),
            half: 100.0,
            half_r: 100.0,
            half_l: 100.0,
            live: 0,
            far: Far::Closed,
            ap_x: 0.0,
            ap_y: 0.0,
            loss_g: 0.99,
            loss_p: 0.0,
            loss_z: 0.0,
            drive: 1.0,
            resonances: vec![Resonance::default(); MAX_RESONANCES],
            damping: Damping::default(),
        };
        g.configure(&Settings::default());
        g
    }

    pub fn set_sample_rate(&mut self, sr: f32) {
        self.sr = sr;
        self.reset();
        let s = self.set;
        self.configure(&s);
    }

    pub fn reset(&mut self) {
        self.right.clear();
        self.left.clear();
        self.ap_x = 0.0;
        self.ap_y = 0.0;
        self.loss_z = 0.0;
    }

    pub fn settings(&self) -> &Settings {
        &self.set
    }

    /// The damping law the column ended up with, for the readouts.
    pub fn damping(&self) -> &Damping {
        &self.damping
    }

    /// The far end's open-to-closed transition frequency, hertz.
    ///
    /// Below it the end behaves open and above it closed, which is what a
    /// hole with air in it does. `Opening` is this frequency as a fraction of
    /// Nyquist, **squared**, so that the part of the morph that happens
    /// inside the audible band occupies the middle of the control's travel
    /// rather than its first tenth.
    pub fn open_hz(&self) -> f32 {
        let o = self.set.opening.clamp(0.0, 1.0);
        o * o * self.sr * 0.5
    }

    /// The geometric length of the column, metres.
    ///
    /// **Derived from the terminations rather than from the delay line**, and
    /// the difference matters. The delay the loop actually runs is shortened
    /// by the loss filter's own phase, so that the *fundamental* comes out
    /// exactly where the Tune control asks for it whatever the decay is set
    /// to. That shortening is an implementation detail of a one-pole filter
    /// and not a fact about a pipe, and printing it as a length would be a
    /// small lie on the panel: at a heavy setting it is three per cent.
    ///
    /// So the length published here is the one the physics gives — an open
    /// tube at `f` is `c/2f` long and a stopped pipe at the same note is half
    /// that — less the end corrections, which are physics as well.
    pub fn column_m(&self) -> f32 {
        let acoustic = C_AIR * self.ideal_half() / self.sr;
        let a = self.set.radius_mm * 1e-3;
        // The permanently open end always corrects; the far one corrects in
        // proportion to how open it is.
        let corr = END_CORRECTION * a * (1.0 + self.set.opening.clamp(0.0, 1.0));
        (acoustic - corr).max(1e-4)
    }

    /// The round trip the physics implies, in milliseconds. See
    /// [`column_m`](Self::column_m) for why this is not the delay line's own
    /// length.
    pub fn loop_ms(&self) -> f32 {
        2000.0 * self.ideal_half() / self.sr
    }

    /// Half the round trip the terminations alone imply, in samples.
    fn ideal_half(&self) -> f32 {
        let f0 = self.set.f0.clamp(1.0, self.sr * 0.45);
        let w0 = std::f32::consts::TAU * f0 / self.sr;
        (0.5 * ((std::f32::consts::TAU + self.allpass_phase(w0)) / w0)).max(MIN_LOOP * 0.5)
    }

    /// The resonances the loop currently has, lowest first.
    pub fn resonances(&self) -> &[Resonance] {
        &self.resonances[..self.live]
    }

    /// Rebuild the loop from a settings snapshot.
    pub fn configure(&mut self, s: &Settings) {
        self.set = *s;
        let sr = self.sr;
        let f0 = s.f0.clamp(1.0, sr * 0.45);
        let w0 = std::f32::consts::TAU * f0 / sr;

        // The far end.
        let o = s.opening.clamp(0.0, 1.0);
        let ob = o * o;
        self.far = if ob <= 1e-6 {
            Far::Closed
        } else if ob >= 1.0 - 1e-6 {
            Far::Open
        } else {
            Far::Hole(
                (std::f32::consts::FRAC_PI_2 * (ob - 0.5))
                    .tan()
                    .clamp(-0.999, 0.999),
            )
        };

        // The damping law: wall loss makes T60 fall as 1/√f and rise in
        // proportion to the bore, so the exponent is the physics' rather than
        // a control's. `Decay` sets the level at the fundamental for a 20 mm
        // bore, which is what makes it mean the same thing on both engines.
        let radius_scale = (s.radius_mm.max(0.1) / RADIUS_REF_MM).clamp(0.05, 20.0);
        self.damping = Damping {
            f0,
            t60: (s.decay * radius_scale).max(1e-3),
            exponent: -0.5,
            corner_hz: sr,
            exponent_hi: -0.5,
        };

        // The one-pole loss, fitted so the round trip loses exactly what the
        // damping law asks for at the fundamental and at a decade above it.
        let f_hi = (f0 * 10.0).min(sr * 0.45);
        // A first guess at the round trip is needed to turn a T60 into a
        // per-trip loss, and the trip length depends on the filter it sizes;
        // one pass is plenty, because the correction is a few per cent.
        let mut half = 0.5 * (std::f32::consts::TAU / w0);
        for _ in 0..3 {
            let trip = 2.0 * half / sr;
            let rho = |f: f32| {
                let t = self.damping.t60_at(f).max(1e-4);
                (-damp::LN1000 * trip / t).exp().clamp(1e-4, 0.999_95)
            };
            self.fit_loss(rho(f0), rho(f_hi), w0, std::f32::consts::TAU * f_hi / sr);
            let extra = self.loss_phase(w0) + self.allpass_phase(w0);
            half = 0.5 * ((std::f32::consts::TAU + extra) / w0).max(MIN_LOOP);
            half = half.min((MAX_RAIL - 8) as f32);
        }
        self.half = half;
        let total = 2.0 * half;
        self.half_r = (total * 0.5).floor() + 0.5;
        self.half_l = (total - self.half_r).max(2.0);

        // The peak of a resonance is roughly one over what the loop loses, so
        // dividing it out keeps a struck column at a sane level whatever the
        // decay is set to — the same reason each mode of the bank is
        // peak-normalised.
        let trip = 2.0 * self.half / sr;
        let rho0 = (-damp::LN1000 * trip / self.damping.t60_at(f0).max(1e-4)).exp();
        self.drive = (1.0 - rho0).clamp(1e-4, 1.0);

        self.find_resonances();
    }

    /// Solve for `p` and `g` so the one pole has magnitude `r1` at `w1` and
    /// `r2` at `w2`.
    ///
    /// `|H|² = g²(1−p)²/(1 − 2p cos ω + p²)`, so the ratio of the two
    /// magnitudes is a quadratic in `p` once `g` is eliminated.
    fn fit_loss(&mut self, r1: f32, r2: f32, w1: f32, w2: f32) {
        let (c1, c2) = (w1.cos(), w2.cos());
        let k = (r1 / r2.max(1e-6)).powi(2);
        let mut p = 0.0f32;
        if (k - 1.0).abs() > 1e-6 {
            let b = k * c1 - c2;
            let disc = b * b - (k - 1.0) * (k - 1.0);
            if disc >= 0.0 {
                let sq = disc.sqrt();
                let p1 = (b + sq) / (k - 1.0);
                let p2 = (b - sq) / (k - 1.0);
                // The stable root; the other is its reciprocal.
                p = if p1.abs() < 0.999 { p1 } else { p2 };
            }
        }
        self.loss_p = p.clamp(-0.99, 0.99);
        let p = self.loss_p;
        let mag1 = (1.0 - 2.0 * p * c1 + p * p).sqrt();
        self.loss_g = (r1 * mag1 / (1.0 - p).max(1e-6)).clamp(0.0, 0.999_95);
    }

    /// The one-pole loss filter's phase at `ω`, which detunes the loop and so
    /// has to be in the length solve.
    fn loss_phase(&self, w: f32) -> f32 {
        let (s, c) = w.sin_cos();
        // arg of g(1−p)/(1 − p e^{-jw}) = −arg(1 − p e^{-jw}).
        -(self.loss_p * s).atan2(1.0 - self.loss_p * c)
    }

    /// The far end's allpass phase at `ω`.
    ///
    /// `A(e^{jω}) = e^{-jω}·conj(1 + a e^{-jω})/(1 + a e^{-jω})`, so
    /// `φ = −ω − 2·arg(1 + a e^{-jω})`, which runs monotonically from 0 to
    /// −π across the band whatever `a` is; `a` only decides how fast. The
    /// two limits are the constants that phase runs between.
    fn allpass_phase(&self, w: f32) -> f32 {
        match self.far {
            Far::Open => 0.0,
            Far::Closed => -std::f32::consts::PI,
            Far::Hole(a) => {
                let (s, c) = w.sin_cos();
                -w - 2.0 * (-a * s).atan2(1.0 + a * c)
            }
        }
    }

    /// The far end's reflection `R(z) = −A(z)` at `ω`.
    fn far_reflection(&self, w: f32) -> C {
        match self.far {
            Far::Open => C(-1.0, 0.0),
            Far::Closed => C(1.0, 0.0),
            Far::Hole(a) => {
                let e = C::unit(-w);
                let num = C(a + e.0, e.1);
                let den = C(1.0 + a * e.0, a * e.1);
                num.div(den).scale(-1.0)
            }
        }
    }

    /// The loss filter at `ω`.
    fn loss_at(&self, w: f32) -> C {
        let e = C::unit(-w);
        C(self.loss_g * (1.0 - self.loss_p), 0.0)
            .div(C(1.0 - self.loss_p * e.0, -self.loss_p * e.1))
    }

    /// The complex response from the input to one pickup, from the loop's own
    /// algebra rather than from a measurement of it.
    ///
    /// With `A = z^{-N}` for a one-way trip of `N` samples, injection at `u`
    /// and pickup at `v`:
    ///
    /// ```text
    ///   P = −H_loss·X·(R·A^{2−u} + A^u) / (1 + H_loss·R·A²)
    ///   Q = R·(P·A + X·A^{1−u})
    ///   out = P·A^v + Q·A^{1−v} + X·A^{|v−u|}
    /// ```
    ///
    /// The last term is the wave that reaches the pickup without having
    /// reflected yet, and it is what puts the comb on the response: injecting
    /// a third of the way along a delay loop cancels every third harmonic,
    /// which is the same physics as a strike landing on a string's node.
    fn response_at(&self, hz: f32, pickup: f32) -> C {
        let w = std::f32::consts::TAU * hz / self.sr;
        let n = self.half;
        let u = self.set.hit.clamp(0.0, 1.0);
        let v = pickup.clamp(0.0, 1.0);
        let ax = |k: f32| C::unit(-w * n * k);
        let r = self.far_reflection(w);
        let hl = self.loss_at(w);
        let loop_den = C(1.0, 0.0).add(hl.mul(r).mul(ax(2.0)));
        let p = hl
            .mul(r.mul(ax(2.0 - u)).add(ax(u)))
            .scale(-1.0)
            .div(loop_den);
        let q = r.mul(p.mul(ax(1.0)).add(ax(1.0 - u)));
        p.mul(ax(v))
            .add(q.mul(ax(1.0 - v)))
            .add(ax((v - u).abs()))
            .scale(self.drive * tilt(hz, self.set.f0, self.set.tilt_db_oct))
    }

    /// What a resonance would reach with no comb from the contact points:
    /// the loop's own gain, times the spectral tilt.
    ///
    /// The panel draws this behind the bars so a partial the strike or the
    /// pickup has nulled reads as energy **removed** rather than energy that
    /// was never there. The two are the same height on a display that only
    /// draws what came out.
    pub fn bare(&self, hz: f32) -> f32 {
        let w = std::f32::consts::TAU * hz / self.sr;
        let den = C(1.0, 0.0).add(
            self.loss_at(w)
                .mul(self.far_reflection(w))
                .mul(C::unit(-w * 2.0 * self.half)),
        );
        let m = den.abs().max(1e-9);
        self.drive * tilt(hz, self.set.f0, self.set.tilt_db_oct) / m
    }

    /// The magnitude the panel draws: the two pickups, power-averaged.
    pub fn response(&self, hz: f32) -> f32 {
        let l = self.response_at(hz, self.set.pos_l).abs();
        let r = self.response_at(hz, self.set.pos_r).abs();
        (0.5 * (l * l + r * r)).sqrt()
    }

    /// Where the loop resonates, and how loud each one is.
    ///
    /// A resonance is where the round trip comes back in phase:
    /// `−ωD + φ_loss(ω) + φ_A(ω) = −2πn`. Both filter phases move slowly
    /// with frequency, so the fixed point converges in a handful of passes
    /// from the pure-delay guess.
    fn find_resonances(&mut self) {
        let d = 2.0 * self.half;
        let nyq = self.sr * 0.5;
        let mut count = 0usize;
        for n in 1..=(MAX_RESONANCES * 4) {
            let mut w = std::f32::consts::TAU * n as f32 / d;
            for _ in 0..6 {
                let phase = self.loss_phase(w) + self.allpass_phase(w);
                let next = (std::f32::consts::TAU * n as f32 + phase) / d;
                if next <= 0.0 {
                    break;
                }
                if (next - w).abs() < 1e-9 {
                    w = next;
                    break;
                }
                w = next;
            }
            let hz = w * self.sr / std::f32::consts::TAU;
            if hz <= 0.0 {
                continue;
            }
            if hz >= nyq * 0.98 || count >= MAX_RESONANCES {
                break;
            }
            let l = self.response_at(hz, self.set.pos_l).abs();
            let r = self.response_at(hz, self.set.pos_r).abs();
            self.resonances[count] = Resonance {
                hz,
                t60: self.damping.t60_at(hz),
                amp_l: l,
                amp_r: r,
                bare: self.bare(hz),
                n: n as u16,
            };
            count += 1;
        }
        for slot in self.resonances[count..].iter_mut() {
            *slot = Resonance::default();
        }
        self.live = count;
    }

    /// One block. `out_l` and `out_r` are written, not added to.
    ///
    /// The order is the one a delay loop needs: read both rails at the far
    /// and near ends first, so each read sees a sample written a whole trip
    /// ago; then reflect, advance and write; then inject the strike, so a
    /// strike at the very end of the column lands on the sample just
    /// written rather than one behind it.
    pub fn process(&mut self, input: &[f32], out_l: &mut [f32], out_r: &mut [f32]) {
        let n = input.len().min(out_l.len()).min(out_r.len());
        let (hr, hl) = (self.half_r, self.half_l);
        let u = self.set.hit.clamp(0.0, 1.0);
        let vl = self.set.pos_l.clamp(0.0, 1.0);
        let vr = self.set.pos_r.clamp(0.0, 1.0);
        let far = self.far;
        let g = self.loss_g * (1.0 - self.loss_p);
        let p = self.loss_p;
        let drive = self.drive;
        for i in 0..n {
            let arriving = self.right.read(hr);
            let back = self.left.read(hl);

            // The far end. Fully closed and fully open are held exactly;
            // between them it is one first-order allpass with a sign, which
            // is the reflection of a hole with the mass of air in it:
            //   A(z) = (a + z^-1)/(1 + a z^-1),   R = -A
            let a_out = match far {
                Far::Closed => -arriving,
                Far::Open => arriving,
                Far::Hole(a) => {
                    let y = a * arriving + self.ap_x - a * self.ap_y;
                    self.ap_x = arriving;
                    self.ap_y = y;
                    y
                }
            };

            // The near end: permanently open, so a plain inversion, with the
            // whole round trip's loss in one pole.
            self.loss_z = g * back + p * self.loss_z;

            self.right.advance();
            self.left.advance();
            self.right.write(-self.loss_z);
            self.left.write(-a_out);

            // The strike goes in at its point on the column, in both
            // directions, because a source is not one-sided.
            let x = input[i] * drive;
            self.right.inject(u * hr, x);
            self.left.inject((1.0 - u) * hl, x);

            out_l[i] = self.right.read(vl * hr) + self.left.read((1.0 - vl) * hl);
            out_r[i] = self.right.read(vr * hr) + self.left.read((1.0 - vr) * hl);
        }
    }
}

/// A spectral tilt in decibels per octave about the fundamental.
///
/// Applied Acoustics calibrate the equivalent control on the same engine in
/// exactly this unit — "a value of −6 dB/octave results in the amplitude of
/// the partials being inversely proportional to their frequency" — and it is
/// printed in that unit here for the same reason: a bare −1 … +1 is a number
/// nobody can reason about.
pub fn tilt(hz: f32, f0: f32, db_per_oct: f32) -> f32 {
    if db_per_oct == 0.0 {
        return 1.0;
    }
    let oct = (hz.max(1e-3) / f0.max(1e-3)).log2();
    10f32.powf(db_per_oct * oct / 20.0)
}
