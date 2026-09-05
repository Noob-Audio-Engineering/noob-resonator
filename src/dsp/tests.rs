//! Every test here that checks a real figure asserts that figure with its
//! source named in a comment.
//!
//! Two rules govern the file and neither is negotiable. **Never widen an
//! assertion until it passes** — if the model cannot meet a number, the model
//! is wrong or the miss is recorded. And **never assert a value the model
//! produced**: a test that compares the engine with itself is not a test, it
//! is a tautology with a green tick, and an audit of this project found nine
//! of them across five plug-ins.
//!
//! So the physics here is checked against Leissa, Abramowitz and Stegun,
//! Russell, Lehtonen and Fletcher, and the behaviour is checked by
//! **measuring the audio** — a frequency read off zero crossings, a decay
//! fitted to an envelope — rather than by reading the coefficients back out
//! of the object that wrote them. `scratchpad/resprobe/p1_physics.py` does
//! the same job from outside the repository, implementing Bessel functions
//! from their integral representation and beam eigenvalues by bisection, and
//! agrees with every series here to under a ten-thousandth of a cent.

use super::*;
use crate::dsp::bank::{Bank, ModeInfo};
use crate::dsp::object::{
    Object, Point, Shape, Walk, bar_targets, beam_eigenvalue, beam_shape, bessel_jn, bessel_zero,
};

const SR: f32 = 48_000.0;

// ---------------------------------------------------------------------------
// Measurement helpers. These read the audio, never the coefficients.
// ---------------------------------------------------------------------------

/// The frequency of a decaying sinusoid, from its zero crossings.
///
/// A long-window FFT cannot resolve a cent at 20 Hz — the bin spacing alone
/// is most of one — but the time between the first and the last of two
/// hundred upward crossings can, because the error in each crossing is a
/// fraction of a sample and it is divided by the whole span.
fn frequency_of(sig: &[f32], sr: f32) -> f32 {
    let mut first = 0.0f64;
    let mut last = 0.0f64;
    let mut count = 0usize;
    for i in 1..sig.len() {
        let (a, b) = (sig[i - 1] as f64, sig[i] as f64);
        if a <= 0.0 && b > 0.0 {
            let t = (i - 1) as f64 + (-a) / (b - a);
            if count == 0 {
                first = t;
            }
            last = t;
            count += 1;
        }
    }
    if count < 3 {
        return 0.0;
    }
    let period = (last - first) / (count - 1) as f64;
    (sr as f64 / period) as f32
}

/// The T60 of a decaying signal, fitted to the part of the envelope between
/// −5 and −35 dB below its peak.
///
/// The window is the one `MODAL.md` §7.2 arrived at by getting it wrong:
/// starting at the peak catches the attack and finishing at the noise floor
/// catches the numerical floor, and both bias the answer.
fn t60_of(sig: &[f32], sr: f32) -> f32 {
    let win = (sr as usize / 200).max(16);
    let mut env: Vec<(f64, f64)> = Vec::new();
    let mut i = 0;
    while i + win <= sig.len() {
        let mut acc = 0.0f64;
        for k in 0..win {
            acc += (sig[i + k] as f64).powi(2);
        }
        let rms = (acc / win as f64).sqrt();
        if rms > 0.0 {
            env.push(((i + win / 2) as f64 / sr as f64, 20.0 * rms.log10()));
        }
        i += win;
    }
    if env.len() < 4 {
        return 0.0;
    }
    let peak = env.iter().map(|e| e.1).fold(f64::NEG_INFINITY, f64::max);
    let pts: Vec<(f64, f64)> = env
        .iter()
        .copied()
        .filter(|e| e.1 <= peak - 5.0 && e.1 >= peak - 35.0)
        .collect();
    if pts.len() < 4 {
        return 0.0;
    }
    let n = pts.len() as f64;
    let sx: f64 = pts.iter().map(|p| p.0).sum();
    let sy: f64 = pts.iter().map(|p| p.1).sum();
    let sxx: f64 = pts.iter().map(|p| p.0 * p.0).sum();
    let sxy: f64 = pts.iter().map(|p| p.0 * p.1).sum();
    let slope = (n * sxy - sx * sy) / (n * sxx - sx * sx);
    if slope >= 0.0 {
        return 0.0;
    }
    (-60.0 / slope) as f32
}

fn cents(a: f32, b: f32) -> f32 {
    1200.0 * (a / b).log2()
}

/// Run one mode of a bank and return its output.
fn ring_one(hz: f32, t60: f32, samples: usize) -> Vec<f32> {
    let mut b = Bank::new(SR);
    b.begin(1);
    b.set_mode(0, hz, t60, 1.0, 1.0, 1.0, ModeInfo::default(), true);
    let mut inp = vec![0.0f32; bank::BLOCK];
    inp[0] = 1.0;
    let mut out = vec![0.0f32; samples];
    let mut l = vec![0.0f32; bank::BLOCK];
    let mut r = vec![0.0f32; bank::BLOCK];
    let mut done = 0;
    while done < samples {
        let n = (samples - done).min(bank::BLOCK);
        b.process(&inp[..n], &mut l[..n], &mut r[..n]);
        out[done..done + n].copy_from_slice(&l[..n]);
        inp[..n].fill(0.0);
        done += n;
    }
    out
}

/// Run the whole device on an impulse and return the two channels.
fn ring_engine(set: &Settings, samples: usize) -> (Vec<f32>, Vec<f32>) {
    let mut e = Resonator::new(SR);
    e.configure(set);
    let mut l = vec![0.0f32; samples];
    let mut r = vec![0.0f32; samples];
    l[0] = 1.0;
    r[0] = 1.0;
    // Let the mode search settle before the strike lands.
    let mut silence_l = vec![0.0f32; bank::BLOCK];
    let mut silence_r = vec![0.0f32; bank::BLOCK];
    for _ in 0..600 {
        e.process(&mut silence_l, &mut silence_r);
    }
    e.process(&mut l, &mut r);
    (l, r)
}

// ---------------------------------------------------------------------------
// The physics
// ---------------------------------------------------------------------------

#[test]
fn beam_eigenvalues_match_leissa() {
    // Leissa, *Vibration of Plates*, NASA SP-160, Table 4.23: the roots of
    // cos β cosh β = 1, which is the free–free (and clamped–clamped) beam.
    let published = [
        4.730_041, 7.853_205, 10.995_608, 14.137_165, 17.278_760, 20.420_352,
    ];
    for (k, want) in published.iter().enumerate() {
        let got = beam_eigenvalue(k + 1);
        assert!(
            (got - want).abs() < 5e-6,
            "beta_{} is {got} and Leissa Table 4.23 says {want}",
            k + 1
        );
    }
}

#[test]
fn free_beam_ratios_are_the_published_series() {
    // The ratios (β_n/β_1)² that make a struck bar sound like a bar, as
    // MODAL.md §2.3 prints them from the same eigenvalues.
    let published = [1.0000, 2.7565, 5.4039, 8.9330, 13.3443];
    let shape = Shape {
        object: Object::Beam,
        ..Shape::default()
    };
    for (k, want) in published.iter().enumerate() {
        let got = shape.ratio(k as u16 + 1, 0) as f32;
        assert!(
            (got - want).abs() < 6e-4,
            "beam partial {} is {got} and the published series says {want}",
            k + 1
        );
    }
}

#[test]
fn the_asymptotic_bar_formula_is_thirteen_cents_wrong_and_we_do_not_use_it() {
    // Dan Russell's Penn State page gives the bar frequency with
    // β_n → (2n+1)π/2, which is an excellent approximation for high modes and
    // wrong by 13.3 cents on the **first overtone** — exactly the partial a
    // listener uses to identify the object. MODAL.md §2.3 measures the same
    // number out of synthesised audio.
    let exact = {
        let b = beam_eigenvalue(2) / beam_eigenvalue(1);
        b * b
    } as f32;
    let asymptotic = {
        let b = 5.0 / 3.0; // (2·2+1)/(2·1+1)
        b * b
    } as f32;
    let err = cents(asymptotic, exact);
    assert!(
        (err - 13.36).abs() < 0.2,
        "the asymptotic bar formula is {err:.2} cents out; MODAL.md §2.3 measured 13.36"
    );
    // And what the engine actually produces is the exact one.
    let shape = Shape {
        object: Object::Beam,
        ..Shape::default()
    };
    let ours = shape.ratio(2, 0) as f32;
    assert!(cents(ours, exact).abs() < 0.01);
}

#[test]
fn bessel_zeros_match_abramowitz_and_stegun() {
    // Abramowitz and Stegun, Table 9.5.
    assert!((bessel_zero(0, 1) - 2.404_825_558).abs() < 1e-6);
    assert!((bessel_zero(1, 1) - 3.831_705_970).abs() < 1e-6);
    assert!((bessel_zero(0, 2) - 5.520_078_110).abs() < 1e-6);
    assert!((bessel_zero(2, 1) - 5.135_622_302).abs() < 1e-6);
}

#[test]
fn round_membrane_ratios_match_russell() {
    // Daniel A. Russell, Penn State, circular membrane demonstration: the
    // ideal-membrane ratios of the (0,1) (1,1) (2,1) (0,2) (1,2) (0,3) modes.
    let published = [
        ((0u16, 1u16), 1.000f32),
        ((1, 1), 1.593),
        ((2, 1), 2.135),
        ((0, 2), 2.295),
        ((1, 2), 2.917),
        ((0, 3), 3.598),
    ];
    let shape = Shape {
        object: Object::MembraneRound,
        ..Shape::default()
    };
    for ((m, n), want) in published {
        let got = shape.ratio(m, n) as f32;
        assert!(
            (got - want).abs() < 6e-4,
            "round membrane ({m},{n}) is {got} and Russell publishes {want}"
        );
    }
}

#[test]
fn square_membrane_and_plate_ratios_are_exact() {
    // Elementary, and Russell publishes the membrane form: a membrane's
    // frequency goes as √(m²+n²) and a plate's as (m²+n²), which is the whole
    // difference between a two-dimensional wave equation and a
    // two-dimensional flexural one.
    let mem = Shape {
        object: Object::Membrane,
        ..Shape::default()
    };
    let plate = Shape {
        object: Object::Plate,
        ..Shape::default()
    };
    for (m, n) in [(1u16, 1u16), (1, 2), (2, 2), (1, 3), (3, 3)] {
        let want_m = (((m * m + n * n) as f32) / 2.0).sqrt();
        let want_p = ((m * m + n * n) as f32) / 2.0;
        assert!((mem.ratio(m, n) as f32 - want_m).abs() < 1e-5);
        assert!((plate.ratio(m, n) as f32 - want_p).abs() < 1e-5);
    }
}

#[test]
fn stiff_string_inharmonicity_matches_lehtonen() {
    // Lehtonen, Välimäki and colleagues, *Analysis of Piano Tones Using an
    // Inharmonic Inverse Comb Filter*, DAFx-08, equation (2), with the
    // B = 3.0 × 10⁻⁴ they measured for a piano C4. MODAL.md §2.1 tabulates
    // what that does to partials 8, 16 and 32.
    let shape = Shape {
        object: Object::String,
        inharm_b: 3.0e-4,
        ..Shape::default()
    };
    for (n, want) in [(8u16, 16.5f32), (16, 64.1), (32, 231.9)] {
        let got = cents(shape.ratio(n, 0) as f32, n as f32);
        assert!(
            (got - want).abs() < 0.2,
            "partial {n} is {got:.2} cents sharp; Lehtonen et al. give {want}"
        );
    }
}

#[test]
fn inharmonicity_is_one_signed_for_a_real_string() {
    // A stiff string's partials are stretched and never compressed: B ≥ 0
    // always. The negative half of the control is the reciprocal, which is a
    // legitimate synthetic extension and is not a string — so the test checks
    // that it is exactly the reciprocal rather than pretending it is physics.
    let up = Shape {
        object: Object::String,
        inharm_b: 1e-3,
        ..Shape::default()
    };
    let down = Shape {
        object: Object::String,
        inharm_b: -1e-3,
        ..Shape::default()
    };
    for n in [2u16, 8, 24] {
        let a = cents(up.ratio(n, 0) as f32, n as f32);
        let b = cents(down.ratio(n, 0) as f32, n as f32);
        assert!(a > 0.0 && b < 0.0);
        assert!((a + b).abs() < 1e-2, "the two halves are not reciprocal");
    }
}

#[test]
fn partial_counts_match_the_published_table() {
    // MODAL.md §2.7 counted the partials below 20 kHz from a 55 Hz
    // fundamental with an out-of-tree probe; the same counts come out here.
    // The plate figure is CORPUS.md §4.6's, which is the **square** plate;
    // MODAL.md §2.7's 579 is the same object at a 1 : 1.41 aspect, and the
    // aspect is the whole difference between them.
    let cases: [(Object, f32, usize); 4] = [
        (Object::Beam, 0.0, 28),
        (Object::String, 0.0, 363),
        (Object::String, 3.0e-4, 139),
        (Object::Plate, 0.0, 545),
    ];
    for (object, b, want) in cases {
        let shape = Shape {
            object,
            inharm_b: b,
            ..Shape::default()
        };
        let got = shape.available(20_000.0 / 55.0);
        assert_eq!(
            got, want,
            "{object:?} with B={b} has {got} partials below 20 kHz; the published count is {want}"
        );
    }
}

#[test]
fn membrane_count_matches_weyl() {
    // Weyl's law for a two-dimensional wave equation, N(k) ≈ Ak²/4π − Pk/4π,
    // which MODAL.md §2.7 used to cross-check the same count. The rectangular
    // membrane's count is exact rather than asymptotic, so the two should
    // agree to a fraction of a per cent.
    let shape = Shape {
        object: Object::Membrane,
        ..Shape::default()
    };
    let r = 20_000.0f64 / 55.0;
    let counted = shape.available(r) as f64;
    // Unit area, sides √a and 1/√a with a = 1, so A = 1 and P = 4. The
    // ratio-to-wavenumber scale is k = π·r·√2 for the unit square.
    let k = std::f64::consts::PI * r * std::f64::consts::SQRT_2;
    let weyl = k * k / (4.0 * std::f64::consts::PI) - 4.0 * k / (4.0 * std::f64::consts::PI);
    assert!(
        (counted / weyl - 1.0).abs() < 0.01,
        "counted {counted} against Weyl's {weyl:.0}"
    );
    // And MODAL.md §2.7's published count, which is for the **1 : 1.41**
    // membrane rather than the square one. A rectangle of the same area has a
    // higher fundamental than a square, so at a fixed fundamental it is a
    // bigger object and it has more partials; the two figures differ by that
    // and by nothing else.
    let oblong = Shape {
        object: Object::Membrane,
        aspect: 1.41,
        ..Shape::default()
    };
    let got = oblong.available(r) as f64;
    assert!(
        (got - 219_541.0).abs() < 400.0,
        "MODAL.md §2.7 counts 219,541 partials for a 1 : 1.41 membrane at 55 Hz and this counts {got}"
    );
}

#[test]
fn mode_shapes_are_mass_normalised() {
    // Every ψ is normalised so its mean square over the object is one, which
    // is the physical normalisation and is what makes mode amplitudes
    // comparable. Checked by integrating rather than by trusting the algebra.
    let n = 20_001;
    for mode in [1usize, 2, 4, 8] {
        let mut acc = 0.0f64;
        for i in 0..n {
            let x = i as f64 / (n - 1) as f64;
            let w = if i == 0 || i == n - 1 { 0.5 } else { 1.0 };
            acc += w * beam_shape(mode, x).powi(2);
        }
        let mean = acc / (n - 1) as f64;
        assert!(
            (mean - 1.0).abs() < 1e-4,
            "beam mode {mode} integrates to {mean}"
        );
    }
    // The round membrane's radial normalisation uses J_{m+1} at the zero,
    // which is the identity ∫₀¹ J_m(jr)² r dr = J_{m+1}(j)²/2.
    for (m, k) in [(0usize, 1usize), (1, 1), (2, 2), (3, 1)] {
        let z = bessel_zero(m, k);
        let jm1 = bessel_jn(m + 1, z).abs();
        let mut acc = 0.0f64;
        let steps = 20_001;
        for i in 0..steps {
            let r = i as f64 / (steps - 1) as f64;
            let w = if i == 0 || i == steps - 1 { 0.5 } else { 1.0 };
            acc += w * bessel_jn(m, z * r).powi(2) * r;
        }
        let integral = acc / (steps - 1) as f64;
        assert!(
            (integral - jm1 * jm1 / 2.0).abs() < 1e-6,
            "the Bessel normalisation identity fails at ({m},{k})"
        );
    }
}

#[test]
fn marimba_tuning_targets_are_the_published_ones() {
    // Fletcher and Rossing, and Rossing's percussion volume, on the arch-cut
    // bar: a marimba bar's first overtone is tuned to two octaves and a
    // xylophone's to a twelfth. The second tuned overtone is quoted at about
    // 9.2× by Woodhouse's *Euphonics* §3.3 and at 10× by Fletcher and
    // Rossing; that disagreement is a builder's choice and this control is
    // it.
    assert_eq!(bar_targets(0, 0), (4.0, 9.2));
    assert_eq!(bar_targets(0, 1), (4.0, 10.0));
    assert_eq!(bar_targets(1, 0).0, 3.0);
    let m = Shape {
        object: Object::Marimba,
        ..Shape::default()
    };
    assert!((m.ratio(2, 0) - 4.0).abs() < 1e-9);
    assert!((m.ratio(3, 0) - 9.2).abs() < 1e-9);
    // The series has to stay increasing past the tuned pair, or partials
    // cross over each other and the ordering the whole engine assumes fails.
    let mut prev = 0.0;
    for n in 1..40u16 {
        let r = m.ratio(n, 0);
        assert!(
            r > prev,
            "marimba partial {n} is not above partial {}",
            n - 1
        );
        prev = r;
    }
}

// ---------------------------------------------------------------------------
// The damping law
// ---------------------------------------------------------------------------

#[test]
fn the_material_exponent_is_the_published_law() {
    // Applied Acoustics, on the same control in their own product: at −1
    // "the decay time will be inversely proportional to the frequency of the
    // partial", at 0 all partials decay at the same rate, at +1 "the decay
    // time is proportional to the frequency of the partial".
    for (m, factor) in [(-1.0f32, 0.5f32), (0.0, 1.0), (1.0, 2.0)] {
        let d = damp::Damping {
            f0: 220.0,
            t60: 2.0,
            exponent: m,
            corner_hz: 20_000.0,
            exponent_hi: m,
        };
        let got = d.t60_at(440.0) / d.t60_at(220.0);
        assert!(
            (got - factor).abs() < 1e-5,
            "at Material {m} an octave up multiplies T60 by {got}, and the published law says {factor}"
        );
    }
}

#[test]
fn storing_the_decrement_beats_storing_the_radius() {
    // MODAL.md §6.4: asked for a thousand-second decay, storing the pole
    // radius in f32 gives 1,207 seconds and storing the decrement gives
    // 1,000.000. The radius arithmetic is reproduced here from its own
    // definition rather than taken from the engine, so the comparison is
    // between two ways of writing the same number and not between the engine
    // and itself.
    let sr = 48_000.0f32;
    for &want in &[10.0f32, 60.0, 200.0, 1000.0] {
        let d = damp::decrement(want, sr);
        let ours = damp::t60_of(d, sr);
        assert!(
            (ours - want).abs() / want < 1e-4,
            "the decrement gives {ours} for a wanted {want}"
        );
        // The other way: build r = exp(−ln1000/(T60·fs)) in f32 and read it
        // back.
        let r = (-damp::LN1000 / (want * sr)).exp();
        let naive = -damp::LN1000 / (r.ln() * sr);
        if want >= 1000.0 {
            assert!(
                naive > 1150.0,
                "storing the radius should be far off at 1000 s and gave {naive}"
            );
        }
    }
    // And the headline figure, to a couple of seconds.
    let r = (-damp::LN1000 / (1000.0 * 48_000.0f32)).exp();
    let naive = -damp::LN1000 / (r.ln() * 48_000.0);
    assert!(
        (naive - 1207.0).abs() < 40.0,
        "MODAL.md §6.4 measured 1,207 s here and this gives {naive}"
    );
}

// ---------------------------------------------------------------------------
// The mode bank, measured from its output
// ---------------------------------------------------------------------------

#[test]
fn a_mode_rings_at_the_frequency_it_was_given() {
    // The pass mark is MODAL.md §7.4's: **every partial within one cent**,
    // which is roughly the threshold of pitch discrimination. Measured from
    // the audio by zero crossings, not read back from the coefficients.
    for hz in [20.0f32, 27.5, 55.0, 110.0, 440.0, 1000.0, 4000.0, 12_000.0] {
        let sig = ring_one(hz, 60.0, (SR as usize) * 4);
        let got = frequency_of(&sig, SR);
        let err = cents(got, hz);
        assert!(
            err.abs() < 1.0,
            "a mode asked for {hz} Hz rang at {got} Hz, {err:.4} cents out"
        );
    }
}

#[test]
fn the_coupled_form_holds_its_tuning_where_a_two_pole_does_not() {
    // MODAL.md §6.3 measured the classic two-pole reson **7 cents out at
    // 20 Hz** in single precision, because it stores 2r·cos θ and cos θ has
    // no relative precision left as θ → 0. The two-pole is implemented here
    // from van den Doel and Pai's own equation (6) rather than borrowed from
    // the engine, so this compares two published structures rather than the
    // engine with itself.
    let hz = 20.0f32;
    let t60 = 2.0f32;
    let r = (-damp::LN1000 / (t60 * SR)).exp();
    let theta = std::f32::consts::TAU * hz / SR;
    // v(m) = 2R cos θ v(m−1) − R² v(m−2) + a R sin θ F(m−1)
    let a1: f32 = 2.0 * r * theta.cos();
    let a2: f32 = r * r;
    let n = (SR as usize) * 4;
    let mut v1 = r * theta.sin();
    let mut v2 = 0.0f32;
    let mut out = vec![0.0f32; n];
    for s in out.iter_mut() {
        let v = a1 * v1 - a2 * v2;
        v2 = v1;
        v1 = v;
        *s = v;
    }
    let two_pole = frequency_of(&out, SR);
    let two_pole_err = cents(two_pole, hz).abs();
    assert!(
        two_pole_err > 2.0,
        "the two-pole should be badly out at 20 Hz in f32 and was {two_pole_err:.3} cents"
    );

    let ours = frequency_of(&ring_one(hz, t60, n), SR);
    let our_err = cents(ours, hz).abs();
    assert!(
        our_err < 0.1,
        "the coupled form should be inside a tenth of a cent at 20 Hz and was {our_err:.4}"
    );
}

#[test]
fn a_mode_decays_for_as_long_as_it_was_told() {
    // MODAL.md §7.4's second pass mark: **within 2 %** on every partial with
    // a T60 above 0.05 s, measured by fitting the envelope between −5 and
    // −35 dB.
    for (hz, t60) in [
        (110.0f32, 0.5f32),
        (440.0, 1.0),
        (440.0, 3.0),
        (2000.0, 0.2),
        (2000.0, 8.0),
    ] {
        let samples = ((t60 * 1.5 * SR) as usize).clamp(SR as usize, 20 * SR as usize);
        let sig = ring_one(hz, t60, samples);
        let got = t60_of(&sig, SR);
        let err = (got - t60).abs() / t60;
        assert!(
            err < 0.02,
            "a {t60} s decay at {hz} Hz measured {got:.4} s, {:.2} % out",
            err * 100.0
        );
    }
}

#[test]
fn each_mode_is_peak_normalised() {
    // The section's own resonance gain is divided out so a mode's amplitude
    // means what it says, whatever its decay. Checked by driving one mode at
    // its own frequency and reading the steady state, which is the definition
    // of the quantity rather than a restatement of the formula.
    for (hz, t60) in [(440.0f32, 0.5f32), (440.0, 3.0), (1000.0, 1.0)] {
        let mut b = Bank::new(SR);
        b.begin(1);
        b.set_mode(0, hz, t60, 1.0, 1.0, 1.0, ModeInfo::default(), true);
        let n = ((t60 * 4.0 * SR) as usize).max(SR as usize);
        let mut peak = 0.0f32;
        let mut phase = 0.0f32;
        let mut inp = vec![0.0f32; bank::BLOCK];
        let mut l = vec![0.0f32; bank::BLOCK];
        let mut r = vec![0.0f32; bank::BLOCK];
        let mut done = 0usize;
        while done < n {
            for s in inp.iter_mut() {
                *s = (std::f32::consts::TAU * phase).sin();
                phase = (phase + hz / SR).fract();
            }
            b.process(&inp, &mut l, &mut r);
            if done > n / 2 {
                for v in l.iter() {
                    peak = peak.max(v.abs());
                }
            }
            done += bank::BLOCK;
        }
        let db = 20.0 * peak.max(1e-9).log10();
        assert!(
            db.abs() < 0.5,
            "a peak-normalised mode driven at its own frequency reached {db:.2} dB"
        );
    }
}

// ---------------------------------------------------------------------------
// The waveguide
// ---------------------------------------------------------------------------

#[test]
fn an_open_tube_gives_every_harmonic_and_a_stopped_pipe_gives_the_odd_ones() {
    // The standard result, and the whole difference between the two objects:
    // an open–open column resonates at n·c/2ℓ and an open–closed one at
    // (2n−1)·c/4ℓ. The pass mark is the same one cent the mode bank is held
    // to.
    let f0 = 220.0f32;
    for (opening, odd_only) in [(1.0f32, false), (0.0, true)] {
        let mut g = guide::Guide::new(SR);
        g.configure(&guide::Settings {
            f0,
            opening,
            radius_mm: 20.0,
            decay: 4.0,
            tilt_db_oct: 0.0,
            hit: 0.13,
            pos_l: 0.31,
            pos_r: 0.53,
        });
        let res = g.resonances();
        assert!(res.len() > 8, "only {} resonances came out", res.len());
        for (k, r) in res.iter().take(8).enumerate() {
            let want = if odd_only {
                (2 * k + 1) as f32 * f0
            } else {
                (k + 1) as f32 * f0
            };
            let err = cents(r.hz, want);
            assert!(
                err.abs() < 1.0,
                "opening {opening}: resonance {k} is at {} Hz, wanted {want} Hz ({err:.3} cents)",
                r.hz
            );
        }
    }
}

#[test]
fn a_stopped_pipe_is_half_the_length_of_an_open_tube_at_the_same_pitch() {
    // Why a stopped organ pipe sounds an octave below an open one of the same
    // length, stated the other way round: hold the pitch and the stopped one
    // is half as long. The engine publishes the length it derived, so the
    // octave stays visible rather than being hidden by the choice to hold the
    // fundamental.
    let mut open = guide::Guide::new(SR);
    let mut stopped = guide::Guide::new(SR);
    let base = guide::Settings {
        f0: 220.0,
        radius_mm: 1.0,
        decay: 2.0,
        ..guide::Settings::default()
    };
    open.configure(&guide::Settings {
        opening: 1.0,
        ..base
    });
    stopped.configure(&guide::Settings {
        opening: 0.0,
        ..base
    });
    let ratio = open.column_m() / stopped.column_m();
    assert!(
        (ratio - 2.0).abs() < 0.01,
        "an open tube came out {ratio:.4}× the stopped pipe's length, not 2"
    );
    // And the arithmetic that says how long: c/2f for an open tube, less
    // Levine and Schwinger's end correction at each open end, which on a 1 mm
    // bore is a millimetre and a bit.
    let want = guide::C_AIR / (2.0 * 220.0) - 2.0 * guide::END_CORRECTION * 0.001;
    assert!(
        (open.column_m() - want).abs() / want < 0.005,
        "220 Hz open is {} m and c/2f less the end corrections is {want} m",
        open.column_m()
    );
}

#[test]
fn striking_a_column_at_a_third_of_its_length_nulls_every_third_harmonic() {
    // The mode-shape null, in the waveguide rather than in the bank: a source
    // at 1/k of the length puts nothing into every k-th partial. It is a
    // derivation and not a tone control, and it is the same physics as a
    // string plucked at a twelfth.
    //
    // Measured by **moving the strike and leaving everything else alone**, so
    // that the pickup's own comb — which is a second correct null structure —
    // divides out instead of being mistaken for this one.
    //
    // The decay is long on purpose. A null can only be as deep as the
    // resonance it is cancelling is tall, and a resonance is 1/(1 − ρ) tall:
    // at a two-second decay the sixth partial's loop only reaches 28 dB, so a
    // 25 dB assertion there would be measuring the Q and not the comb.
    let run = |hit: f32| -> Vec<f32> {
        let mut g = guide::Guide::new(SR);
        g.configure(&guide::Settings {
            f0: 220.0,
            opening: 1.0,
            radius_mm: 20.0,
            decay: 20.0,
            tilt_db_oct: 0.0,
            hit,
            pos_l: 0.13,
            pos_r: 0.13,
        });
        g.resonances()
            .iter()
            .take(9)
            .map(|r| r.amp_l.max(1e-12))
            .collect()
    };
    let third = run(1.0 / 3.0);
    let plain = run(0.19);
    for k in 0..9usize {
        let change = 20.0 * (third[k] / plain[k]).log10();
        if (k + 1) % 3 == 0 {
            assert!(
                change < -25.0,
                "partial {} should collapse when the strike moves to a third and only fell {change:.1} dB",
                k + 1
            );
        } else {
            assert!(
                change > -6.0,
                "partial {} is not a multiple of three and fell {change:.1} dB",
                k + 1
            );
        }
    }
}

// ---------------------------------------------------------------------------
// The selection, which is the point
// ---------------------------------------------------------------------------

/// The mean power of a signal between two frequencies, in dB, from its own
/// spectrum.
///
/// A magnitude curve sampled once per display column cannot be used for this:
/// where the partials are closer together than the sampling, it reads
/// whichever part of each peak it happens to land on and invents structure
/// that is not there. So the measurement is a transform of the audio.
fn band_power_db(sig: &[f32], sr: f32, lo: f32, hi: f32) -> f32 {
    use rustfft::num_complex::Complex;
    let n = 1 << 17;
    let mut buf: Vec<Complex<f32>> = (0..n)
        .map(|i| {
            let x = sig.get(i).copied().unwrap_or(0.0);
            // Hann, so a strong low partial's skirt does not leak into the
            // band being measured.
            let w = 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / (n - 1) as f32).cos();
            Complex::new(x * w, 0.0)
        })
        .collect();
    rustfft::FftPlanner::new()
        .plan_fft_forward(n)
        .process(&mut buf);
    let mut acc = 0.0f64;
    let mut count = 0usize;
    for (k, v) in buf.iter().enumerate().take(n / 2) {
        let hz = k as f32 * sr / n as f32;
        if hz >= lo && hz <= hi {
            acc += (v.norm_sqr() as f64).max(1e-30);
            count += 1;
        }
    }
    if count == 0 {
        return -200.0;
    }
    10.0 * (acc / count as f64).log10() as f32
}

#[test]
fn keeping_the_loudest_modes_beats_keeping_the_lowest_by_tens_of_decibels() {
    // The finding the whole plug-in is built on, measured here on this engine
    // rather than quoted: at a fixed budget on a membrane, **which** modes
    // are kept is worth far more in the presence band than any affordable
    // change in **how many**.
    //
    // MODAL.md §8.1 measured 65 dB in the 4–10 kHz band for a 512-mode budget
    // on a 110 Hz rectangular membrane, with its own strike positions and its
    // own tilt. This asserts a conservative floor rather than that number,
    // because the two experiments differ in both; what it establishes is that
    // the effect is there and is tens of decibels, which is the claim.
    let base = Settings {
        object: 3, // Membrane
        tune_hz: 110.0,
        modes: 512,
        decay_s: 2.0,
        material: -0.5,
        tail: false,
        limiter: false,
        // The device's own default tilt, because that is what a listener
        // hears. At exactly flat the criterion degenerates and `select.rs`
        // says so rather than hiding it behind a test that avoids the case.
        ..Settings::default()
    };
    let mut power = [0.0f32; 2];
    let mut top = [0.0f32; 2];
    let mut above4k = [0usize; 2];
    for order in [0usize, 1] {
        let set = Settings { order, ..base };
        let (l, _) = ring_engine(&set, (SR as usize) * 2);
        power[order] = band_power_db(&l, SR, 4000.0, 10_000.0);
        let mut e = Resonator::new(SR);
        e.configure(&set);
        let mut a = vec![0.0f32; bank::BLOCK];
        let mut b = vec![0.0f32; bank::BLOCK];
        for _ in 0..600 {
            e.process(&mut a, &mut b);
        }
        top[order] = e.bank().info().iter().fold(0.0f32, |m, i| m.max(i.hz));
        above4k[order] = e.bank().info().iter().filter(|i| i.hz > 4000.0).count();
    }
    let gain = power[0] - power[1];
    assert!(
        gain > 20.0,
        "keeping the loudest gave {:.1} dB in 4–10 kHz and keeping the lowest gave {:.1} dB, a difference of only {gain:.1} dB",
        power[0],
        power[1]
    );

    // And the mechanism, which is not a matter of degree: keeping the lowest
    // 512 of a 110 Hz membrane reaches about 2 kHz and nothing above it.
    // MODAL.md §8.1 measured 1,980 Hz for the 1 : 1.41 membrane; a square one
    // is a slightly smaller object at the same fundamental, so it reaches a
    // little higher.
    assert!(
        top[1] > 1900.0 && top[1] < 2200.0,
        "the lowest 512 reached {} Hz and the published probe put it at 1,980",
        top[1]
    );
    assert_eq!(
        above4k[1], 0,
        "keeping the lowest 512 should leave the presence band empty"
    );
    assert!(
        above4k[0] > 100,
        "keeping the loudest 512 put only {} partials above 4 kHz",
        above4k[0]
    );
}

#[test]
fn the_search_settles_and_keeps_what_it_should() {
    // A membrane has far more partials than any budget, so the selection has
    // to finish and it has to finish with the budget full.
    let mut e = Resonator::new(SR);
    e.configure(&Settings {
        object: 3,
        tune_hz: 110.0,
        modes: 256,
        ..Settings::default()
    });
    let mut l = vec![0.0f32; bank::BLOCK];
    let mut r = vec![0.0f32; bank::BLOCK];
    let mut blocks = 0;
    loop {
        e.process(&mut l, &mut r);
        blocks += 1;
        if e.info_frame()[10] >= 1.0 || blocks >= 20_000 {
            break;
        }
    }
    let info = e.info_frame();
    assert!(blocks < 20_000, "the mode search never settled");
    assert_eq!(info[0] as usize, 256, "the budget was not filled");
    assert!(
        info[1] > 40_000.0,
        "a 110 Hz membrane should have tens of thousands of partials and this found {}",
        info[1]
    );
    // And it settled inside a tenth of a second, which is what the bounded
    // per-block work budget is for.
    assert!(
        (blocks * bank::BLOCK) as f32 / SR < 0.2,
        "the search took {:.3} s to settle",
        (blocks * bank::BLOCK) as f32 / SR
    );
}

// ---------------------------------------------------------------------------
// The tail
// ---------------------------------------------------------------------------

#[test]
fn the_tail_is_dense_enough_to_read_as_a_continuum() {
    // Schroeder and Logan's criterion, quoted by Smith's *Physical Audio
    // Signal Processing*: a response reads as a continuum rather than as a
    // set of resonances above about **0.15 modes per hertz**.
    for sr in [44_100.0f32, 48_000.0, 96_000.0, 192_000.0] {
        let t = tail::Tail::new(sr);
        let d = t.report().density;
        assert!(
            d > 0.15,
            "the tail has {d:.3} modes per hertz at {sr} Hz, under Schroeder and Logan's 0.15"
        );
    }
}

#[test]
fn the_tail_is_silent_when_nothing_was_left_out() {
    // A bar has 28 partials in the whole band, so a budget of 1,024 leaves
    // nothing behind and the tail must add nothing at all.
    let set = Settings {
        object: 0,
        tune_hz: 220.0,
        modes: 1024,
        tail: true,
        ..Settings::default()
    };
    let mut e = Resonator::new(SR);
    e.configure(&set);
    let mut l = vec![0.0f32; bank::BLOCK];
    let mut r = vec![0.0f32; bank::BLOCK];
    for _ in 0..64 {
        e.process(&mut l, &mut r);
    }
    let tail_db = e.info_frame()[3];
    assert!(
        tail_db < -60.0,
        "a bar leaves nothing over and the tail still came out at {tail_db:.1} dB"
    );
}

// ---------------------------------------------------------------------------
// The device
// ---------------------------------------------------------------------------

#[test]
fn every_object_rings_and_none_of_them_diverges() {
    for (i, name) in OBJECT_NAMES.iter().enumerate() {
        let set = Settings {
            object: i,
            tune_hz: 110.0,
            modes: 512,
            decay_s: 1.0,
            ..Settings::default()
        };
        let (l, r) = ring_engine(&set, (SR as usize) / 2);
        let peak = l.iter().chain(r.iter()).fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(peak.is_finite(), "{name} produced a non-finite sample");
        assert!(peak > 1e-4, "{name} produced nothing at all (peak {peak})");
        assert!(peak < 4.0, "{name} peaked at {peak}");
    }
}

#[test]
fn the_plug_in_reports_no_latency_and_has_none() {
    // Zero reported and zero actual. The device this one answers reports 64
    // samples unconditionally and attributes them to its limiter; ours has a
    // limiter and no lookahead, so there is nothing to report.
    let e = Resonator::new(SR);
    assert_eq!(e.latency(), 0);

    // An impulse straight through with the resonator gated off arrives on
    // sample zero.
    let set = Settings {
        mix: 0.0,
        ..Settings::default()
    };
    let mut e = Resonator::new(SR);
    e.configure(&set);
    let mut l = vec![0.0f32; 256];
    let mut r = vec![0.0f32; 256];
    l[0] = 1.0;
    r[0] = 1.0;
    e.process(&mut l, &mut r);
    assert!(
        (l[0] - 1.0).abs() < 1e-6,
        "the dry path is not sample-aligned: sample zero is {}",
        l[0]
    );
}

#[test]
fn bypass_is_bit_exact() {
    let mut e = Resonator::new(SR);
    e.configure(&Settings {
        bypass: true,
        ..Settings::default()
    });
    let mut l: Vec<f32> = (0..512).map(|i| (i as f32 * 0.01).sin()).collect();
    let mut r: Vec<f32> = (0..512).map(|i| (i as f32 * 0.013).cos()).collect();
    let (l0, r0) = (l.clone(), r.clone());
    e.process(&mut l, &mut r);
    assert_eq!(l, l0);
    assert_eq!(r, r0);
}

#[test]
fn the_limiter_never_lets_the_ceiling_past() {
    // Zero lookahead, so it can distort a transient; what it cannot do is
    // exceed the ceiling, and that is the whole reason it is allowed to have
    // no latency.
    let set = Settings {
        object: 2,
        tune_hz: 110.0,
        decay_s: 20.0,
        modes: 256,
        limiter: true,
        limit_ceil_db: -6.0,
        gain_db: 36.0,
        ..Settings::default()
    };
    let mut e = Resonator::new(SR);
    e.configure(&set);
    let mut peak = 0.0f32;
    let mut l = vec![0.0f32; bank::BLOCK];
    let mut r = vec![0.0f32; bank::BLOCK];
    for b in 0..400 {
        for i in 0..bank::BLOCK {
            let v = if (b * bank::BLOCK + i).is_multiple_of(4096) {
                1.0
            } else {
                0.0
            };
            l[i] = v;
            r[i] = v;
        }
        e.process(&mut l, &mut r);
        for v in l.iter().chain(r.iter()) {
            peak = peak.max(v.abs());
        }
    }
    let ceiling = 10f32.powf(-6.0 / 20.0);
    assert!(
        peak <= ceiling * 1.001,
        "the output reached {peak} against a ceiling of {ceiling}"
    );
}

#[test]
fn dry_wet_gates_the_input_and_does_not_chop_the_tail() {
    // The device this one answers gets this right and it is copied
    // deliberately: "turning Dry/Wet down will not cut resonances that are
    // currently sounding, but rather stop new input signals from being
    // processed". A modal bank whose tail is cut by a fader clicks.
    let mut e = Resonator::new(SR);
    let set = Settings {
        object: 2,
        tune_hz: 220.0,
        decay_s: 4.0,
        modes: 128,
        mix: 1.0,
        ..Settings::default()
    };
    e.configure(&set);
    let mut l = vec![0.0f32; bank::BLOCK];
    let mut r = vec![0.0f32; bank::BLOCK];
    for _ in 0..600 {
        e.process(&mut l, &mut r);
    }
    l[0] = 1.0;
    r[0] = 1.0;
    e.process(&mut l, &mut r);
    // Let it ring, then shut the input.
    for _ in 0..8 {
        l.fill(0.0);
        r.fill(0.0);
        e.process(&mut l, &mut r);
    }
    let before = l.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    e.configure(&Settings { mix: 0.0, ..set });
    l.fill(0.0);
    r.fill(0.0);
    e.process(&mut l, &mut r);
    let after = l.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    assert!(
        after > before * 0.5,
        "closing Dry/Wet cut the tail from {before} to {after}"
    );
}

#[test]
fn the_mode_table_reaches_the_audio() {
    // Exposing frequency, gain and decay per partial costs nothing at runtime
    // and is native to this architecture and to no other. This checks that an
    // edit written the way the page writes it actually moves a partial.
    let table = Arc::new(ModeTable::new());
    table.load_json(&json!({ "edits": [{ "i": 0, "cents": 700.0, "db": 0.0, "decay": 1.0 }] }));
    let mut p = Processor::with_table(SR, table.clone());
    let set = Settings {
        object: 2,
        tune_hz: 220.0,
        modes: 8,
        decay_s: 6.0,
        hit: Point::new(0.13, 0.13),
        pos_l: Point::new(0.29, 0.29),
        pos_r: Point::new(0.29, 0.29),
        bright_db_oct: -24.0f32.max(-6.0),
        ..Settings::default()
    };
    p.configure(&set);
    let mut l = vec![0.0f32; bank::BLOCK];
    let mut r = vec![0.0f32; bank::BLOCK];
    for _ in 0..64 {
        p.process(&mut l, &mut r);
    }
    let frame = p.engine().modes_frame();
    // The first published partial should now be a fifth above the
    // fundamental: 220 × 2^(7/12) = 329.6 Hz.
    let hz = frame[2];
    assert!(
        (hz - 329.63).abs() < 1.0,
        "a +700 cent edit on partial 0 put it at {hz} Hz instead of 329.63"
    );
    // And the round trip through the store's own JSON keeps it.
    let json = table.to_json();
    let back = ModeTable::new();
    back.load_json(&json);
    let mut edits = [ModeEdit::default(); MAX_EDITS];
    back.read(&mut edits);
    assert!((edits[0].cents - 700.0).abs() < 1e-3);
}

#[test]
fn the_parameter_and_stream_contract_is_what_it_says_it_is() {
    // The page is built against these ids and layouts, so a change here is a
    // change to an interface somebody else is holding.
    let specs = param_specs(false);
    for id in [
        "type",
        "tune",
        "transpose",
        "fine",
        "modes",
        "select",
        "ratio",
        "bar_tuning",
        "bar_third",
        "radius",
        "opening",
        "decay",
        "material",
        "damp_corner",
        "damp_hi",
        "tail",
        "bright",
        "inharm",
        "hit",
        "hit_y",
        "pos_l",
        "pos_l_y",
        "pos_r",
        "pos_r_y",
        "spread",
        "width",
        "filter_on",
        "filter_freq",
        "filter_width",
        "filter_place",
        "lfo_on",
        "lfo_shape",
        "lfo_rate",
        "lfo_depth",
        "lfo_phase",
        "bleed",
        "mix",
        "gain",
        "limiter",
        "limit_ceil",
        "bypass",
    ] {
        assert!(
            specs.iter().any(|s| s.id == id),
            "the frozen contract has a parameter `{id}` and the engine does not"
        );
    }
    let st = streams(SR);
    assert_eq!(st[STREAM_IX.meter].capacity, METER_LEN);
    assert_eq!(st[STREAM_IX.modes].capacity, MAX_EDITS * MODE_FIELDS);
    assert_eq!(st[STREAM_IX.info].capacity, INFO_LEN);
    assert_eq!(st[STREAM_IX.response].capacity, RESPONSE_POINTS);
    // Every object names the controls it uses, so the panel greys out from
    // the engine's own truth rather than deriving it again.
    let meta = object_meta();
    let list = meta.as_array().expect("objects is an array");
    assert_eq!(list.len(), OBJECT_NAMES.len());
    for (i, o) in list.iter().enumerate() {
        assert_eq!(o["label"], OBJECT_NAMES[i]);
        let uses = o["uses"].as_array().unwrap();
        let guide = o["engine"] == "waveguide";
        let has = |k: &str| uses.iter().any(|u| u == k);
        // An air column has no mode list to truncate and no material.
        assert_eq!(has("modes"), !guide);
        assert_eq!(has("material"), !guide);
        assert_eq!(has("radius"), guide);
    }
    assert!(
        list[5]["uses"]
            .as_array()
            .unwrap()
            .iter()
            .any(|u| u == "opening"),
        "Opening belongs to the Pipe"
    );
    assert!(
        !list[6]["uses"]
            .as_array()
            .unwrap()
            .iter()
            .any(|u| u == "opening"),
        "a Tube is already open at both ends"
    );
}

#[test]
fn a_walk_is_ordered_within_a_column_and_covers_the_object() {
    // The selector's early exits assume both, so if either fails the search
    // silently stops short.
    for object in Object::ALL {
        if object.engine() != object::Engine::Guide {
            let shape = Shape {
                object,
                ..Shape::default()
            };
            let max = 40.0;
            let walked = Walk::new(shape, max).count();
            let counted = shape.available(max);
            assert_eq!(
                walked, counted,
                "{object:?}: the walk yields {walked} partials and the count says {counted}"
            );
        }
    }
}

#[test]
fn the_store_hook_carries_an_edit_to_the_audio_thread() {
    // The mode table's whole delivery path, which no other test covers: the
    // page writes the interface store, a hook parses it, and the audio thread
    // picks it up through atomics without a lock or an allocation. It is also
    // the path that persists, because the plug-in saves that store inside its
    // own state.
    let (bridge, ix) = build_bridge("noob-resonator-test", SR);
    let table = Arc::new(ModeTable::new());
    attach_mode_table(&bridge, table.clone());
    let before = table.generation();

    bridge
        .store_set(
            MODES_KEY,
            json!({ "edits": [{ "i": 2, "cents": -50.0, "db": -3.0, "decay": 2.0 }] }),
        )
        .expect("the store took the table");
    assert_ne!(table.generation(), before, "the hook did not fire");

    let mut edits = [ModeEdit::default(); MAX_EDITS];
    table.read(&mut edits);
    assert!((edits[2].cents + 50.0).abs() < 1e-3);
    assert!((edits[2].db + 3.0).abs() < 1e-3);
    assert!((edits[2].decay - 2.0).abs() < 1e-3);
    assert_eq!(edits[0], ModeEdit::default(), "a sparse table stays sparse");

    // Nonsense from a future version of the page must not be able to silence
    // the plug-in: an unknown key, an index out of range and a wild value are
    // all ignored rather than rejected.
    table.load_json(&json!({
        "edits": [
            { "i": 9999, "cents": 1.0 },
            { "i": 1, "cents": 1e9, "who": "knows" },
        ],
        "future": true
    }));
    table.read(&mut edits);
    assert!(edits[1].cents <= 1200.0, "an absurd offset was not clamped");

    // And every parameter the specs declare resolves on a real bridge, so an
    // id can never drift between the two halves of the contract.
    let _ = ix;
    for spec in param_specs(true) {
        assert!(
            bridge.index_of(&spec.id).is_some(),
            "the bridge has no parameter `{}`",
            spec.id
        );
    }
}
