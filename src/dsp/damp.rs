//! How long each partial rings, and the frequency above which nobody can tell
//! the partials apart any more.
//!
//! ## The law
//!
//! Applied Acoustics publish it quantitatively for the engine inside the
//! device this one answers, and the wording is unambiguous: at a control
//! value of −1 "the decay time will be inversely proportional to the
//! frequency of the partial", at 0 "all partials decay at the same rate", and
//! at +1 "the decay time is proportional to the frequency of the partial".
//! That is a power law whose exponent *is* the control:
//!
//! ```text
//!   T60(f) = T60(f₁) · (f/f₁)^m,     m ∈ [−1, +1]
//! ```
//!
//! Three sources agree on it: AAS's own prose, Ableton's qualitative
//! description of the same knob ("at lower values, low frequency components
//! decay slower … at higher values, high frequency components decay slower"),
//! and the range their file serialises the parameter over, which is exactly
//! −1 … +1.
//!
//! ## Why there is a second exponent
//!
//! Because one is not enough, and the literature says so from two directions.
//! Djoharian derives the physically correct shape for a viscoelastic solid —
//! damping going as `ω²` at low frequency and **flattening toward a constant**
//! at high frequency — which is two regimes and not one. Bilbao, Webb, Wang
//! and Ducceschi name what the simulation literature actually ships: "a basic
//! two-parameter loss model", fitted to give a chosen T60 at two frequencies.
//! A single exponent cannot be both.
//!
//! So [`Damping`] has a corner and a second exponent above it. **At the
//! default corner of 20 kHz the second exponent never comes into play and the
//! law is exactly AAS's single-exponent one**, which is the honest default:
//! the extra freedom is there to be used, not to change the device out from
//! under the sourced behaviour.
//!
//! ## Storing the decrement rather than the pole radius
//!
//! A long decay puts the pole radius `r = exp(−ln1000/(T60·fs))` very close to
//! 1, where `f32` has spent all its precision on the leading digit. Storing
//! the **decrement** `d = 1 − r` instead puts the same number where `f32` has
//! full relative precision, and `d` is what the resonator's arithmetic wants
//! anyway. `MODAL.md` §6.4 measured what that is worth: asked for a 1,000
//! second decay, storing `r` gives 1,207 seconds and storing `d` gives
//! 1,000.000. `tests.rs` re-measures it here rather than quoting it.
//!
//! And `d` is computed by `expm1` rather than as `1.0 - exp(x)`, because that
//! subtraction is the same cancellation one step earlier.

/// `ln(1000)`: the decay in nepers that a T60 asks for.
pub const LN1000: f32 = 6.907_755_3;

/// The decrement `d = 1 − r` for a wanted T60, at a sample rate.
///
/// `T60 ≤ 0` means never decaying, which is a freeze rather than a mistake:
/// `d = 0` is exactly representable and is reached deliberately instead of by
/// rounding, which is the other half of what storing the decrement buys.
#[inline]
pub fn decrement(t60_s: f32, sr: f32) -> f32 {
    if t60_s <= 0.0 || !t60_s.is_finite() {
        return 0.0;
    }
    let k = LN1000 / (t60_s * sr);
    // -expm1(-k) is 1 - exp(-k) without the cancellation.
    (-((-k).exp_m1())).clamp(0.0, 1.0)
}

/// The T60 a decrement gives, the inverse of [`decrement`]. Used by the
/// readouts and by the tests that measure what was asked for.
#[inline]
pub fn t60_of(d: f32, sr: f32) -> f32 {
    if d <= 0.0 {
        return f32::INFINITY;
    }
    if d >= 1.0 {
        return 0.0;
    }
    // ln(1 - d) with the cancellation taken out, again.
    -LN1000 / ((-d).ln_1p() * sr)
}

/// The −3 dB bandwidth of a mode with this T60, in hertz.
///
/// `B = ln(1000)/(π·T60)`. It is what decides whether two neighbouring modes
/// can be told apart, so it is the other half of the modal overlap factor.
#[inline]
pub fn bandwidth_hz(t60_s: f32) -> f32 {
    if t60_s <= 0.0 {
        f32::INFINITY
    } else {
        LN1000 / (std::f32::consts::PI * t60_s)
    }
}

/// The two-parameter damping law, and everything that follows from it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Damping {
    /// The fundamental, which is where `t60` is quoted.
    pub f0: f32,
    /// T60 at the fundamental, seconds. This is what the Decay control reads.
    pub t60: f32,
    /// The exponent below the corner: AAS's Material, −1 … +1.
    pub exponent: f32,
    /// Where the second exponent takes over. At the top of the band it never
    /// does, and the law is AAS's.
    pub corner_hz: f32,
    /// The exponent above the corner.
    pub exponent_hi: f32,
}

impl Default for Damping {
    fn default() -> Self {
        Damping {
            f0: 220.0,
            t60: 2.0,
            exponent: -0.5,
            corner_hz: 20_000.0,
            exponent_hi: -1.0,
        }
    }
}

impl Damping {
    /// T60 at a frequency, in seconds.
    ///
    /// Continuous at the corner by construction: the upper branch starts from
    /// the value the lower branch reached there, so moving the corner never
    /// steps the decay of a partial sitting on it.
    pub fn t60_at(&self, hz: f32) -> f32 {
        let f0 = self.f0.max(1e-3);
        let f = hz.max(1e-3);
        let corner = self.corner_hz.max(f0);
        if f <= corner {
            self.t60 * (f / f0).powf(self.exponent)
        } else {
            let at_corner = self.t60 * (corner / f0).powf(self.exponent);
            at_corner * (f / corner).powf(self.exponent_hi)
        }
    }

    /// The decrement for a partial at `hz`.
    pub fn decrement_at(&self, hz: f32, sr: f32) -> f32 {
        decrement(self.t60_at(hz), sr)
    }
}

/// Where the partials stop standing apart, in hertz.
///
/// The measure is the **modal overlap factor** `M(f) = n(f)·B(f)`: the modal
/// density in modes per hertz times each mode's own −3 dB bandwidth. Below
/// `M = 1` the modes are separated by more than their own width and the
/// response is a comb of peaks; above it they merge into a continuum that is
/// statistically a reverb tail, and no listener and no analyser can count
/// them.
///
/// This is the same split room acoustics has used since Schroeder — exact
/// modes below the crossover, a statistical description above it — applied to
/// an object instead of a room. It is what decides how much of the band the
/// mode bank is being asked to cover and how much the tail should.
///
/// **The crossover moves with the decay setting and that is not a defect.** A
/// short decay widens every mode until they all merge; a long metal ring
/// keeps them apart to the top of the band. So the budget a bank needs is a
/// function of the decay control rather than a fixed number, which is exactly
/// why the modes are ordered by contribution and truncated rather than taken
/// in frequency order.
///
/// `density` gives modes per hertz at a frequency; the caller supplies it
/// because only the object knows its own law.
pub fn crossover_hz(damping: &Damping, f_max: f32, density: impl Fn(f32) -> f32) -> f32 {
    let f0 = damping.f0.max(1e-3);
    if f_max <= f0 {
        return f_max;
    }
    const STEPS: usize = 96;
    let ratio = (f_max / f0).powf(1.0 / STEPS as f32);
    let mut f = f0;
    let mut prev = (f, overlap(damping, f, &density));
    if prev.1 >= 1.0 {
        return f0;
    }
    for _ in 0..STEPS {
        f *= ratio;
        let m = overlap(damping, f, &density);
        if m >= 1.0 {
            // Log-linear interpolation between the two grid points, which is
            // as much resolution as this number deserves.
            let (f_a, m_a) = prev;
            let t = ((1.0 - m_a) / (m - m_a)).clamp(0.0, 1.0);
            return f_a * (f / f_a).powf(t);
        }
        prev = (f, m);
    }
    f_max
}

/// `M(f) = n(f)·B(f)` at one frequency.
fn overlap(damping: &Damping, hz: f32, density: &impl Fn(f32) -> f32) -> f32 {
    let b = bandwidth_hz(damping.t60_at(hz));
    if !b.is_finite() {
        return f32::INFINITY;
    }
    density(hz) * b
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn decrement_round_trips() {
        for &t in &[0.05f32, 1.0, 10.0, 60.0, 300.0, 1000.0] {
            let d = decrement(t, 48_000.0);
            let back = t60_of(d, 48_000.0);
            assert!(
                (back - t).abs() / t < 1e-4,
                "T60 {t} came back as {back} through d = {d}"
            );
        }
    }
}
