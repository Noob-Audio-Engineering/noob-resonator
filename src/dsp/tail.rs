//! The statistical extension: what happens above the frequency where nobody
//! can tell one partial from the next.
//!
//! A mode is worth computing separately only while it can be told apart from
//! its neighbours. The measure is the modal overlap factor `M(f) = n(f)·B(f)`
//! — modal density times each mode's own bandwidth — and below `M = 1` the
//! partials stand apart as a comb of peaks while above it they merge into a
//! continuum that is statistically a reverb tail. `MODAL.md` §8.2 computed it
//! for a 1 : 1.41 membrane tuned to 110 Hz with a realistic falling decay and
//! put the crossover at **729 Hz, with 63 of its 54,749 partials below it**;
//! this engine, on a square membrane at the same tuning and its own default
//! damping, puts it at **1,165 Hz out of 51,673**. Either way the partials
//! above it are real, and no listener and no analyser can resolve them
//! individually.
//!
//! So modelling fifty thousand of them exactly is possible and pointless. It
//! is an expensive way to synthesise a dense tail. What the ear can tell,
//! very easily, is the difference between that tail and **silence** — which is
//! what a bank truncated at a fixed partial count leaves up there.
//!
//! This is the same split room acoustics has used for seventy years — exact
//! modes below the Schroeder frequency, a statistical description above it —
//! applied to an object rather than a room.
//!
//! ## What it matches, and what it does not
//!
//! It matches three things, and they are the three the ear can check:
//!
//! * **The decay against frequency.** Each delay line's loss filter is fitted
//!   to the same `T60(f)` law the mode bank uses, so a partial that crosses
//!   the crossover does not change how long it rings.
//! * **The energy, per band.** The selector accumulates the squared amplitude
//!   of **every** candidate partial it walks past, kept or not, so the
//!   residual is a sum over the modes that were actually left out rather than
//!   an estimate of them. The tail is levelled to that residual.
//! * **The band it occupies.** Nothing below the crossover, where the bank is
//!   exact.
//!
//! **It does not match the modal density law, and it cannot.** A feedback
//! delay network's density is `Σd_i/f_s` modes per hertz, constant with
//! frequency, where a membrane's rises linearly with it. Above the crossover
//! that difference is by construction inaudible — the requirement up there is
//! that the density *exceed* what the ear can resolve, not that it take a
//! particular value — and the network is sized to clear Schroeder and Logan's
//! 0.15 modes per hertz across the whole band with a wide margin.
//! `docs/BENCHMARK.md` prints the density it actually achieves.
//!
//! The other honest limit is the **spectral shape**. The residual is measured
//! in 32 logarithmic bands and fitted with one first-order shelf, which
//! cannot render an arbitrary curve. The benchmark prints the fit error in
//! decibels rather than leaving it to be assumed.

use crate::dsp::damp;
use crate::dsp::filters::{OnePole, Svf};

/// Delay lines in the network.
pub const LINES: usize = 8;
/// Bands the residual energy is measured and fitted in.
pub const BANDS: usize = 32;
/// Longest line, in samples: the base lengths at 192 kHz. A power of two, so
/// the circular buffers wrap with a mask rather than a division — see the note
/// on [`crate::dsp::guide`]'s rails, which measured what that is worth.
const MAX_LINE: usize = 16384;

/// The mask that wraps an index into [`MAX_LINE`].
const LINE_MASK: usize = MAX_LINE - 1;

/// Base line lengths at 48 kHz, in samples.
///
/// Primes, so that no two lines share a period and the network's own modes do
/// not pile up on each other. They sum to 17,477 samples, which is
/// **0.364 modes per hertz** at 48 kHz — well past Schroeder and Logan's
/// 0.15 criterion for a response that reads as a continuum rather than as a
/// set of resonances.
const BASE_LEN: [usize; LINES] = [1237, 1543, 1811, 2053, 2311, 2593, 2851, 3078];

/// One line's state.
struct Line {
    buf: Vec<f32>,
    w: usize,
    len: usize,
    /// The loss filter: one pole, fitted to the damping law.
    g: f32,
    p: f32,
    z: f32,
}

impl Line {
    fn new() -> Line {
        Line {
            buf: vec![0.0; MAX_LINE],
            w: 0,
            len: 1024,
            g: 0.5,
            p: 0.0,
            z: 0.0,
        }
    }

    #[inline]
    fn read(&self) -> f32 {
        self.buf[(self.w + MAX_LINE - self.len) & LINE_MASK]
    }

    #[inline]
    fn write(&mut self, x: f32) {
        self.buf[self.w] = x;
        self.w = (self.w + 1) & LINE_MASK;
    }
}

/// A one-pole filter fitted to a wanted magnitude at two frequencies.
///
/// The same fit the air column's loop filter uses, and for the same reason:
/// a loss that has to follow `T60(f)` needs at least a slope, and one pole is
/// the cheapest thing that has one.
fn fit_one_pole(r1: f32, r2: f32, w1: f32, w2: f32) -> (f32, f32) {
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
            p = if p1.abs() < 0.999 { p1 } else { p2 };
        }
    }
    let p = p.clamp(-0.98, 0.98);
    let mag1 = (1.0 - 2.0 * p * c1 + p * p).sqrt();
    let g = (r1 * mag1 / (1.0 - p).max(1e-6)).clamp(0.0, 0.999);
    (g, p)
}

/// What the tail was asked for and what it worked out, for the readouts.
#[derive(Clone, Copy, Debug, Default)]
pub struct TailReport {
    /// Level relative to the bank, dB. `-inf` when there is nothing left over.
    pub level_db: f32,
    /// Where it starts.
    pub crossover_hz: f32,
    /// Root-mean-square error of the one-shelf fit to the measured residual,
    /// over the bands the tail covers, in dB.
    pub fit_rms_db: f32,
    /// Modes per hertz the network provides.
    pub density: f32,
}

/// The network.
pub struct Tail {
    lines: [Line; LINES],
    sr: f32,
    hp: Svf,
    shelf: OnePole,
    shelf_lo: f32,
    shelf_hi: f32,
    gain: f32,
    report: TailReport,
    on: bool,
}

impl Tail {
    pub fn new(sr: f32) -> Tail {
        let mut t = Tail {
            lines: std::array::from_fn(|_| Line::new()),
            sr,
            hp: Svf::default(),
            shelf: OnePole::default(),
            shelf_lo: 1.0,
            shelf_hi: 1.0,
            gain: 0.0,
            report: TailReport::default(),
            on: false,
        };
        t.set_sample_rate(sr);
        t
    }

    pub fn set_sample_rate(&mut self, sr: f32) {
        self.sr = sr;
        let scale = sr / 48_000.0;
        let mut total = 0usize;
        for (i, line) in self.lines.iter_mut().enumerate() {
            let want = ((BASE_LEN[i] as f32 * scale) as usize).clamp(64, MAX_LINE - 1);
            // Odd lengths, all different, so no two lines beat against each
            // other at a low order.
            line.len = want | 1;
            total += line.len;
        }
        self.report.density = total as f32 / sr;
        self.reset();
    }

    pub fn reset(&mut self) {
        for line in self.lines.iter_mut() {
            line.buf.fill(0.0);
            line.w = 0;
            line.z = 0.0;
        }
        self.hp.reset();
        self.shelf.reset();
    }

    pub fn report(&self) -> TailReport {
        self.report
    }

    /// Set the network up from the damping law and the residual the selector
    /// measured.
    ///
    /// `residual[b]` is `Σ a²·B` over the partials in band `b` that were
    /// **not** kept: each one's squared peak amplitude times its own −3 dB
    /// bandwidth, which is the power a resonator actually passes from a
    /// broadband input rather than its height at one frequency. Dividing by
    /// the band's width turns it into the mean-square transfer the tail has to
    /// supply, which is a quantity the network can be levelled against.
    pub fn configure(
        &mut self,
        damping: &damp::Damping,
        crossover_hz: f32,
        residual: &[f32; BANDS],
        on: bool,
    ) {
        self.on = on;
        let sr = self.sr;
        let nyq = sr * 0.45;
        let f_c = crossover_hz.clamp(20.0, nyq);
        self.report.crossover_hz = f_c;
        self.hp.set(f_c, std::f32::consts::FRAC_1_SQRT_2, sr);

        // Each line's loss, fitted to the same T60(f) the bank uses so a
        // partial does not change how long it rings by crossing the boundary.
        let f_lo = f_c.max(20.0);
        let f_hi = (f_c * 8.0).min(nyq);
        let (w_lo, w_hi) = (
            std::f32::consts::TAU * f_lo / sr,
            std::f32::consts::TAU * f_hi / sr,
        );
        for line in self.lines.iter_mut() {
            let trip = line.len as f32 / sr;
            let rho = |f: f32| {
                let t = damping.t60_at(f).max(1e-4);
                (-damp::LN1000 * trip / t).exp().clamp(1e-5, 0.999_9)
            };
            let (g, p) = fit_one_pole(rho(f_lo), rho(f_hi), w_lo, w_hi);
            line.g = g;
            line.p = p;
        }

        // The residual's own shape, in the bands the tail covers, fitted with
        // one shelf by least squares on the log magnitude — which is what the
        // ear reads and what the error below is quoted in.
        let pivot = (f_c * nyq).sqrt().max(20.0);
        let mut used = 0usize;
        let (mut sum_x, mut sum_y, mut sum_xx, mut sum_xy) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
        let mut power_sum = 0.0f64;
        let mut width_sum = 0.0f64;
        for b in 0..BANDS {
            let f = band_centre(b, sr);
            let w = band_width(b, sr);
            if f < f_c || f > nyq || residual[b] <= 0.0 {
                continue;
            }
            power_sum += residual[b] as f64;
            width_sum += w as f64;
            let x = (f / pivot).log2() as f64;
            let y = 10.0 * (residual[b] as f64 / w as f64).log10();
            sum_x += x;
            sum_y += y;
            sum_xx += x * x;
            sum_xy += x * y;
            used += 1;
        }
        if used < 2 || power_sum <= 0.0 || width_sum <= 0.0 {
            self.gain = 0.0;
            self.report.level_db = f32::NEG_INFINITY;
            self.report.fit_rms_db = 0.0;
            self.shelf_lo = 1.0;
            self.shelf_hi = 1.0;
            return;
        }
        let n = used as f64;
        let den = n * sum_xx - sum_x * sum_x;
        let slope_db_oct = if den.abs() > 1e-12 {
            ((n * sum_xy - sum_x * sum_y) / den) as f32
        } else {
            0.0
        };
        let intercept = if den.abs() > 1e-12 {
            ((sum_y * sum_xx - sum_x * sum_xy) / den) as f32
        } else {
            (sum_y / n) as f32
        };

        // One pole spans a finite range, so the slope is realised across the
        // octaves the tail actually occupies and clamped to what a first-order
        // shelf can give.
        let span = (nyq / f_c).log2().max(0.5);
        let total_db = (slope_db_oct * span).clamp(-24.0, 24.0);
        self.shelf.set(pivot, sr);
        self.shelf_lo = 10f32.powf(-0.5 * total_db / 20.0);
        self.shelf_hi = 10f32.powf(0.5 * total_db / 20.0);

        // What the fit missed, measured rather than assumed.
        let mut err = 0.0f64;
        for b in 0..BANDS {
            let f = band_centre(b, sr);
            let w = band_width(b, sr);
            if f < f_c || f > nyq || residual[b] <= 0.0 {
                continue;
            }
            let modelled = intercept + slope_db_oct * (f / pivot).log2();
            let actual = 10.0 * (residual[b] / w).log10();
            err += ((modelled - actual) as f64).powi(2);
        }
        self.report.fit_rms_db = (err / n).sqrt() as f32;

        // Level. A delay line whose round trip multiplies by `g` has power
        // gain `1/(1 − g²)` for a white input; the drive is split over the
        // lines and each output tap takes half of them, so the network's own
        // mean-square gain is that average divided by the line count. Dividing
        // it out leaves the tail carrying exactly the mean-square transfer the
        // selector measured as missing.
        let mut power = 0.0f32;
        for line in self.lines.iter() {
            let g = line.g.clamp(0.0, 0.999_9);
            power += 1.0 / (1.0 - g * g).max(1e-4);
        }
        power /= LINES as f32;
        let want = (power_sum / width_sum) as f32;
        self.gain = (want * LINES as f32 / power.max(1e-6))
            .sqrt()
            .clamp(0.0, 64.0);
        self.report.level_db = 20.0 * self.gain.max(1e-9).log10();
    }

    /// One block, **added** to the outputs rather than written, because the
    /// tail sits beside the bank rather than after it.
    pub fn process(&mut self, input: &[f32], out_l: &mut [f32], out_r: &mut [f32]) {
        if !self.on || self.gain <= 0.0 {
            return;
        }
        let n = input.len().min(out_l.len()).min(out_r.len());
        let gain = self.gain;
        let norm = 1.0 / (LINES as f32).sqrt();
        let tap = 1.0 / ((LINES / 2) as f32).sqrt();
        for i in 0..n {
            // Only the band the bank left empty, shaped to the residual's own
            // slope.
            let hp = self.hp.hp(input[i]);
            let lo = self.shelf.lp(hp);
            let drive = (self.shelf_lo * lo + self.shelf_hi * (hp - lo)) * gain * norm;

            let mut s = [0.0f32; LINES];
            for (k, line) in self.lines.iter_mut().enumerate() {
                let raw = line.read();
                line.z = line.g * (1.0 - line.p) * raw + line.p * line.z;
                s[k] = line.z;
            }
            // Two halves of the network to two channels, so the pair is
            // decorrelated without a second network.
            let mut l = 0.0f32;
            let mut r = 0.0f32;
            for (k, v) in s.iter().enumerate() {
                if k < LINES / 2 {
                    l += *v;
                } else {
                    r += *v;
                }
            }
            out_l[i] += l * tap;
            out_r[i] += r * tap;

            // An eight-point Walsh–Hadamard transform is an orthogonal mixing
            // matrix in 24 additions: it spreads every line into every other
            // without changing the total energy, which is what keeps the decay
            // the loss filters asked for.
            let mut v = s;
            hadamard8(&mut v);
            for (k, line) in self.lines.iter_mut().enumerate() {
                let sign = if k % 2 == 0 { 1.0 } else { -1.0 };
                line.write(v[k] * norm + drive * sign);
            }
        }
    }
}

/// The band layout the residual is measured in: 32 logarithmic bands from
/// 20 Hz to Nyquist. Both the selector and the tail read it from here, so they
/// cannot disagree about which partial fell in which band.
pub fn band_edge(b: usize, sr: f32) -> f32 {
    let lo = 20.0f32;
    let hi = (sr * 0.5).max(lo * 2.0);
    lo * (hi / lo).powf(b as f32 / BANDS as f32)
}

/// A band's geometric centre.
pub fn band_centre(b: usize, sr: f32) -> f32 {
    (band_edge(b, sr) * band_edge(b + 1, sr)).sqrt()
}

/// A band's width in hertz.
pub fn band_width(b: usize, sr: f32) -> f32 {
    (band_edge(b + 1, sr) - band_edge(b, sr)).max(1e-3)
}

/// Which band a frequency falls in.
pub fn band_of(hz: f32, sr: f32) -> usize {
    let lo = 20.0f32;
    let hi = (sr * 0.5).max(lo * 2.0);
    if hz <= lo {
        return 0;
    }
    let t = (hz / lo).log2() / (hi / lo).log2();
    ((t * BANDS as f32) as usize).min(BANDS - 1)
}

/// The eight-point Walsh–Hadamard transform, in place.
#[inline]
fn hadamard8(v: &mut [f32; LINES]) {
    let mut step = 1;
    while step < LINES {
        let mut i = 0;
        while i < LINES {
            for k in i..i + step {
                let (a, b) = (v[k], v[k + step]);
                v[k] = a + b;
                v[k + step] = a - b;
            }
            i += step * 2;
        }
        step *= 2;
    }
}
