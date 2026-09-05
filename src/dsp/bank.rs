//! The mode bank: one damped sinusoid per partial, four thousand of them, in
//! the arithmetic and the loop order that were measured rather than assumed.
//!
//! Four decisions, and every one of them came from a measurement in
//! `MODAL.md` rather than from a preference. **Where a figure below is ours it
//! says so, and where it is that document's it says that too** — a number
//! taken from a probe written against a different program is evidence about
//! that program, not about this one, so `tests.rs` and `src/bin/benchmark.rs`
//! measure each decision again here.
//!
//! ## 1. The complex-multiply coupled form, not the classic two-pole
//!
//! Every second-order resonator is two state words and three coefficients;
//! they differ only in what the two state words *mean*. The two-pole reson
//! stores `2r·cos θ`, and as `θ → 0` that coefficient goes to a number near
//! 1, where `f32` has spent all its precision on the leading digit and has
//! almost none left for the tuning. The coupled form's `sin θ` goes to *zero*
//! instead and keeps full relative precision all the way down.
//!
//! Measured here, both structures in `f32` and the two-pole written out from
//! van den Doel and Pai's own equation (6): **the two-pole is 8.1 cents out at
//! 20 Hz and 2.1 cents at the bottom of a piano, and the coupled form is
//! 0.0002 cents at both.** `MODAL.md` §6.3 measured 6.99 and 2.10 for the same
//! structure at its own decay setting — the same defect to the same order.
//!
//! That document's §6.1 adds the other half, and it is its measurement rather
//! than one repeated here: under per-sample pitch modulation the coupled
//! form's amplitude error is 17× smaller, because its state is a rotating
//! vector whose length a change of angle cannot touch. It costs about 1 % more
//! than the two-pole — also its figure. There is no trade in either.
//!
//! ## 2. The decrement, not the pole radius
//!
//! ```text
//!   xr = c·x − s·y            c = cos θ,  s = sin θ,  d = 1 − r
//!   yr = s·x + c·y
//!   x' = xr − d·xr + b·u
//!   y' = yr − d·yr
//! ```
//!
//! The damping is a *subtraction* of `d·xr` rather than a multiplication by
//! `r`, which keeps `cos θ` and `sin θ` free of the decay and lets `d` be
//! stored where `f32` still has precision. Asked for a thousand-second decay,
//! storing `r` gives 1,207 seconds; storing `d` gives 1,000.000.
//!
//! **It is not free here and I am not going to say it is.** Folding `r` into
//! the coefficients would make this four multiplies; keeping it separate
//! makes it six. `docs/BENCHMARK.md` prints what those two multiplies cost,
//! measured, beside what they buy.
//!
//! ## 3. Mode-major with a register block, not sample-major
//!
//! One mode's recursion is a chain of dependent multiply-adds, so a single
//! mode in flight is latency-bound and sixteen are not. `MODAL.md` §4.2
//! measured processing all the samples of one group of sixteen modes before
//! moving to the next at **3.3× a sample-major loop** and **4.2× a single mode
//! at a time**; those are its figures. What the benchmark here measures is the
//! block-size half of the same effect, and it finds **about 2.9×** between a
//! one-sample block and a 128-sample one at 1,024 modes — the exact figure is
//! in `docs/BENCHMARK.md`, which is regenerated rather than transcribed
//! because a busy machine moves it by a few per cent. The vectorisation is
//! the compiler's either way, and none of this is written in intrinsics.
//!
//! ## 4. Lane-buffered accumulation, not a reduction in the inner loop
//!
//! Two pickup positions read the same state, which should cost one extra
//! multiply-add per mode and which `MODAL.md` §4.5 measured costing **95 %
//! more**. The expense is not the multiply, it is collapsing sixteen lanes to
//! one scalar twice per group per sample. Accumulating lane-wise into a
//! `block × 16` buffer and reducing **once per sample at the end of the whole
//! bank** turns `groups × block` horizontal reductions into `block` of them,
//! which it measured at 2.3× to 4× faster than the obvious stereo loop and
//! faster than the obvious *mono* one. That is the layout used here; the
//! comparison against the alternatives is its measurement and not one repeated
//! in this repository.

use crate::dsp::damp;

/// Modes the bank can hold. The design target, and the point past which more
/// modes stop buying anything: it is more than the entire physical mode set
/// of every object here except a membrane, and for a membrane it is far more
/// than the modal-overlap argument says can be resolved.
pub const MAX_MODES: usize = 4096;

/// Modes carried in registers at once. The AVX-512 width on the machine the
/// technique was measured on, and the width its sweep found best; the
/// arithmetic is written as a plain loop over this many lanes and the
/// compiler vectorises whatever the target actually has.
pub const LANES: usize = 16;

/// The bank's internal block. Per-sample processing was measured at 8× the
/// cost of 128-sample blocks, and above 128 the lane buffer leaves the level-1
/// data cache. Whatever the host hands us is cut into pieces of this size.
pub const BLOCK: usize = 128;

/// Below this the state is zeroed at the next block boundary, so that a
/// decaying tail cannot park in the subnormal range where some hardware runs
/// it slowly.
///
/// It is thirteen orders of magnitude above where `f32` subnormals begin, so
/// it prevents the stall completely, and it is far below any level a signal
/// carries, so it cannot silence anything audible. Checking once per block
/// rather than once per sample costs one comparison per mode per 128 samples;
/// checking per sample would cost more than the recursion.
const FLUSH: f32 = 1e-25;

/// The smallest decrement the coefficients are built with.
///
/// `d = 0` is a pole exactly on the unit circle — a perfect, never-decaying
/// oscillator. That is a legitimate freeze, but it also makes the resonance
/// gain infinite, so the normalisation would divide by zero. This floor is a
/// T60 of about four hours at 48 kHz.
const MIN_D: f32 = 1e-8;

/// One partial's readout, kept beside the coefficients for the streams.
#[derive(Clone, Copy, Debug, Default)]
pub struct ModeInfo {
    pub hz: f32,
    pub t60: f32,
    /// Peak level at the left pickup, linear.
    pub amp_l: f32,
    pub amp_r: f32,
    /// The partial's own index in the object's series, which is what a
    /// per-mode edit addresses.
    pub i: u16,
    pub j: u16,
}

/// A parallel bank of coupled-form resonators with two pickup taps.
pub struct Bank {
    // Coefficients and state, structure-of-arrays, padded to a whole number
    // of lane groups so the kernel never has a partial tail to special-case.
    c: Vec<f32>,
    s: Vec<f32>,
    d: Vec<f32>,
    /// Input gain: the strike's mode shape and the spectral tilt, divided by
    /// the section's own resonance gain so that no single mode can peak above
    /// unity however long it rings.
    b: Vec<f32>,
    gl: Vec<f32>,
    gr: Vec<f32>,
    /// The peak amplitude each mode was asked for, kept so that a retune can
    /// divide out the section's new resonance gain without being told again.
    ai: Vec<f32>,
    x: Vec<f32>,
    y: Vec<f32>,
    /// Where the coefficients are heading, and the per-sample step that gets
    /// them there. Holding a coefficient still for a whole block and stepping
    /// it at the boundary puts a block-rate sideband at −60 dB on a modulated
    /// partial; ramping it across the block was measured to be worth a flat
    /// 10.6 dB for three adds per mode per sample.
    ct: Vec<f32>,
    st: Vec<f32>,
    dt: Vec<f32>,
    /// Lane-wise accumulators, `BLOCK × LANES` per tap.
    acc_l: Vec<f32>,
    acc_r: Vec<f32>,
    info: Vec<ModeInfo>,
    len: usize,
    /// Whether the coefficients differ from their targets, so the cheap
    /// kernel can run whenever nothing is moving.
    ramping: bool,
    sr: f32,
}

impl Bank {
    pub fn new(sr: f32) -> Bank {
        let n = MAX_MODES;
        Bank {
            c: vec![0.0; n],
            s: vec![0.0; n],
            d: vec![1.0; n],
            b: vec![0.0; n],
            gl: vec![0.0; n],
            gr: vec![0.0; n],
            ai: vec![0.0; n],
            x: vec![0.0; n],
            y: vec![0.0; n],
            ct: vec![0.0; n],
            st: vec![0.0; n],
            dt: vec![1.0; n],
            acc_l: vec![0.0; BLOCK * LANES],
            acc_r: vec![0.0; BLOCK * LANES],
            info: vec![ModeInfo::default(); n],
            len: 0,
            ramping: false,
            sr,
        }
    }

    pub fn set_sample_rate(&mut self, sr: f32) {
        self.sr = sr;
        self.reset();
    }

    pub fn sample_rate(&self) -> f32 {
        self.sr
    }

    /// Forget every ringing partial. Coefficients survive.
    pub fn reset(&mut self) {
        self.x.fill(0.0);
        self.y.fill(0.0);
    }

    /// How many modes are live.
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The readouts, one per live mode.
    pub fn info(&self) -> &[ModeInfo] {
        &self.info[..self.len]
    }

    /// Start filling the bank from scratch. Modes past `n` are silenced but
    /// their state is left alone, so a mode that survives a rebuild keeps
    /// ringing through it rather than being cut off.
    pub fn begin(&mut self, n: usize) {
        let n = n.min(MAX_MODES);
        for k in n..self.len {
            self.b[k] = 0.0;
            self.gl[k] = 0.0;
            self.gr[k] = 0.0;
        }
        self.len = n;
        // Pad the last group with silent, stable modes so the kernel can run
        // whole groups without a tail.
        let pad = self.groups() * LANES;
        for k in n..pad.min(MAX_MODES) {
            self.c[k] = 0.0;
            self.s[k] = 0.0;
            self.d[k] = 1.0;
            self.ct[k] = 0.0;
            self.st[k] = 0.0;
            self.dt[k] = 1.0;
            self.b[k] = 0.0;
            self.gl[k] = 0.0;
            self.gr[k] = 0.0;
            self.x[k] = 0.0;
            self.y[k] = 0.0;
        }
    }

    fn groups(&self) -> usize {
        self.len.div_ceil(LANES)
    }

    /// Set one mode. `amp_l` and `amp_r` are the peak levels the partial
    /// should reach at the two pickups; the section's own resonance gain is
    /// divided out so they mean what they say.
    ///
    /// `snap` writes the coefficients directly rather than ramping to them,
    /// which is what a rebuild wants for a mode that was not there before.
    ///
    /// The resonance gain of this section, from `H(z) = b·r sinθ z⁻²/(1 −
    /// 2r cosθ z⁻¹ + r²z⁻²)` evaluated on the unit circle at `θ`:
    ///
    /// ```text
    ///   G = (1−d)·sin θ / [ d · √( d²cos²θ + (2−d)²sin²θ ) ]
    /// ```
    ///
    /// which at 440 Hz and a three-second decay is +80.4 dB. That is why the
    /// output stage has a limiter and why nobody's does not: a bank of these
    /// driven by programme material has enormous gain at its own frequencies,
    /// and no choice of constants removes it — peak-normalising each mode
    /// leaves the bank hot when the input excites everything at once, and
    /// normalising the bank leaves each mode inaudible.
    #[allow(clippy::too_many_arguments)]
    pub fn set_mode(
        &mut self,
        k: usize,
        hz: f32,
        t60: f32,
        amp_in: f32,
        amp_l: f32,
        amp_r: f32,
        info: ModeInfo,
        snap: bool,
    ) {
        if k >= MAX_MODES {
            return;
        }
        let theta = std::f32::consts::TAU * hz / self.sr;
        let (s, c) = theta.sin_cos();
        let d = damp::decrement(t60, self.sr).max(MIN_D);
        let r = 1.0 - d;
        let denom = d * (d * d * c * c + (2.0 - d) * (2.0 - d) * s * s).sqrt();
        let g = if denom > 0.0 { r * s / denom } else { 0.0 };
        let b = if g > 1e-12 { amp_in / g } else { 0.0 };

        self.ct[k] = c;
        self.st[k] = s;
        self.dt[k] = d;
        if snap {
            self.c[k] = c;
            self.s[k] = s;
            self.d[k] = d;
        } else {
            self.ramping = true;
        }
        self.b[k] = b;
        self.ai[k] = amp_in;
        self.gl[k] = amp_l;
        self.gr[k] = amp_r;
        self.info[k] = info;
    }

    /// Move one mode to a new frequency, keeping its decay and its wanted
    /// amplitude.
    ///
    /// This is the oscillator's path and it exists because the full
    /// [`set_mode`](Self::set_mode) is dominated by two transcendentals — a
    /// power for the damping law and an `expm1` for the decrement — that a
    /// change of pitch does not touch. What it cannot avoid is the sine and
    /// cosine: the published trick that makes retuning a *single* resonator
    /// free is a rotation of its coefficient pair by a fixed angle, and in a
    /// bank every mode turns by a different one, so the rotation costs
    /// exactly what the transcendental it replaced did.
    pub fn retune(&mut self, k: usize, hz: f32) {
        if k >= MAX_MODES {
            return;
        }
        let theta = std::f32::consts::TAU * hz / self.sr;
        let (s, c) = theta.sin_cos();
        let d = self.dt[k];
        let r = 1.0 - d;
        let denom = d * (d * d * c * c + (2.0 - d) * (2.0 - d) * s * s).sqrt();
        let g = if denom > 0.0 { r * s / denom } else { 0.0 };
        self.b[k] = if g > 1e-12 { self.ai[k] / g } else { 0.0 };
        self.ct[k] = c;
        self.st[k] = s;
        self.info[k].hz = hz;
        self.ramping = true;
    }

    /// Zero a mode's state, for one that has just been given to a different
    /// partial by a rebuild.
    pub fn clear_state(&mut self, k: usize) {
        if k < MAX_MODES {
            self.x[k] = 0.0;
            self.y[k] = 0.0;
        }
    }

    /// Run one block. `input` drives every mode; `out_l` and `out_r` are
    /// written, not added to. All three must be the same length, and no
    /// longer than [`BLOCK`].
    pub fn process(&mut self, input: &[f32], out_l: &mut [f32], out_r: &mut [f32]) {
        let n = input.len().min(out_l.len()).min(out_r.len()).min(BLOCK);
        if n == 0 {
            return;
        }
        let groups = self.groups();
        if groups == 0 {
            out_l[..n].fill(0.0);
            out_r[..n].fill(0.0);
            return;
        }
        if self.ramping {
            self.run::<true>(input, n, groups);
        } else {
            self.run::<false>(input, n, groups);
        }
        // The one horizontal reduction, once per sample for the whole bank
        // rather than once per sample per group.
        for i in 0..n {
            let base = i * LANES;
            let mut l = 0.0f32;
            let mut r = 0.0f32;
            for k in 0..LANES {
                l += self.acc_l[base + k];
                r += self.acc_r[base + k];
            }
            out_l[i] = l;
            out_r[i] = r;
        }
        self.flush_quiet();
    }

    /// The kernel. `RAMP` says whether the coefficients are moving; when they
    /// are not, three adds per mode per sample disappear.
    fn run<const RAMP: bool>(&mut self, input: &[f32], n: usize, groups: usize) {
        let inv = 1.0 / n as f32;
        self.acc_l[..n * LANES].fill(0.0);
        self.acc_r[..n * LANES].fill(0.0);
        for g in 0..groups {
            let base = g * LANES;
            let mut c = [0.0f32; LANES];
            let mut s = [0.0f32; LANES];
            let mut d = [0.0f32; LANES];
            let mut dc = [0.0f32; LANES];
            let mut ds = [0.0f32; LANES];
            let mut dd = [0.0f32; LANES];
            let mut b = [0.0f32; LANES];
            let mut gl = [0.0f32; LANES];
            let mut gr = [0.0f32; LANES];
            let mut x = [0.0f32; LANES];
            let mut y = [0.0f32; LANES];
            for k in 0..LANES {
                c[k] = self.c[base + k];
                s[k] = self.s[base + k];
                d[k] = self.d[base + k];
                b[k] = self.b[base + k];
                gl[k] = self.gl[base + k];
                gr[k] = self.gr[base + k];
                x[k] = self.x[base + k];
                y[k] = self.y[base + k];
                if RAMP {
                    dc[k] = (self.ct[base + k] - c[k]) * inv;
                    ds[k] = (self.st[base + k] - s[k]) * inv;
                    dd[k] = (self.dt[base + k] - d[k]) * inv;
                }
            }
            let al = self.acc_l[..n * LANES].chunks_exact_mut(LANES);
            let ar = self.acc_r[..n * LANES].chunks_exact_mut(LANES);
            for ((&u, la), ra) in input[..n].iter().zip(al).zip(ar) {
                for k in 0..LANES {
                    let xr = c[k] * x[k] - s[k] * y[k];
                    let yr = s[k] * x[k] + c[k] * y[k];
                    x[k] = xr - d[k] * xr + b[k] * u;
                    y[k] = yr - d[k] * yr;
                    la[k] += gl[k] * y[k];
                    ra[k] += gr[k] * y[k];
                    if RAMP {
                        c[k] += dc[k];
                        s[k] += ds[k];
                        d[k] += dd[k];
                    }
                }
            }
            self.x[base..base + LANES].copy_from_slice(&x);
            self.y[base..base + LANES].copy_from_slice(&y);
        }
        if RAMP {
            // The ramp lands exactly on the target rather than wherever the
            // accumulated steps got to.
            let end = groups * LANES;
            self.c[..end].copy_from_slice(&self.ct[..end]);
            self.s[..end].copy_from_slice(&self.st[..end]);
            self.d[..end].copy_from_slice(&self.dt[..end]);
            self.ramping = false;
        }
    }

    /// Zero any state that has decayed out of usefulness, once per block.
    fn flush_quiet(&mut self) {
        for k in 0..self.groups() * LANES {
            if self.x[k].abs() + self.y[k].abs() < FLUSH {
                self.x[k] = 0.0;
                self.y[k] = 0.0;
            }
        }
    }

    /// The bank's magnitude response at a frequency, linear, summed over the
    /// modes with the left and right taps power-averaged.
    ///
    /// This is the engine's own answer to what the object does, which is what
    /// the panel draws instead of arithmetic of its own.
    pub fn response(&self, hz: f32) -> f32 {
        let w = std::f32::consts::TAU * hz / self.sr;
        let (sw, cw) = w.sin_cos();
        // e^{-jw} and e^{-2jw}.
        let (s2, c2) = (2.0 * w).sin_cos();
        let mut acc_l = (0.0f32, 0.0f32);
        let mut acc_r = (0.0f32, 0.0f32);
        for k in 0..self.len {
            let (c, s, d) = (self.c[k], self.s[k], self.d[k]);
            let r = 1.0 - d;
            // 1 − 2r·c·e^{-jw} + r²·e^{-2jw}
            let dr = 1.0 - 2.0 * r * c * cw + r * r * c2;
            let di = 2.0 * r * c * sw - r * r * s2;
            let den = dr * dr + di * di;
            if den <= 1e-30 {
                continue;
            }
            // Numerator b·r·s·e^{-2jw}; only the magnitude of the whole
            // quotient survives the phase of the exponential, but the modes
            // add coherently so the phase has to be carried.
            let num = self.b[k] * r * s;
            let (nr, ni) = (num * c2, -num * s2);
            let hr = (nr * dr + ni * di) / den;
            let hi = (ni * dr - nr * di) / den;
            acc_l.0 += hr * self.gl[k];
            acc_l.1 += hi * self.gl[k];
            acc_r.0 += hr * self.gr[k];
            acc_r.1 += hi * self.gr[k];
        }
        let pl = acc_l.0 * acc_l.0 + acc_l.1 * acc_l.1;
        let pr = acc_r.0 * acc_r.0 + acc_r.1 * acc_r.1;
        (0.5 * (pl + pr)).sqrt()
    }
}
