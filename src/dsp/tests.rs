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
//! of the object that wrote them. `tools/physics_probe.py` does
//! the same job from outside the repository, implementing Bessel functions
//! from their integral representation and beam eigenvalues by bisection, and
//! agrees with every series here to under a ten-thousandth of a cent.

use super::*;
use crate::dsp::bank::{Bank, ModeInfo};
use crate::dsp::object::{
    Contacts, Object, Point, Shape, Walk, bar_targets, beam_eigenvalue, beam_shape,
    bessel_i_scaled, bessel_jn, bessel_zero, disc_root, disc_shape, tine_eigenvalue, tine_shape,
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
            disperse: 0.0,
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
            disperse: 0.0,
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

/// Settle the device and return its published partials as `(i, j, hz)`.
fn published(set: &Settings, table: Option<Arc<ModeTable>>) -> Vec<(u16, u16, f32)> {
    let mut p = match table {
        Some(t) => Processor::with_table(SR, t),
        None => Processor::new(SR),
    };
    p.configure(set);
    let mut l = vec![0.0f32; bank::BLOCK];
    let mut r = vec![0.0f32; bank::BLOCK];
    let mut guard = 0;
    while p.engine().info_frame()[10] < 1.0 && guard < 40_000 {
        p.process(&mut l, &mut r);
        guard += 1;
    }
    for _ in 0..32 {
        p.process(&mut l, &mut r);
    }
    let f = p.engine().modes_frame();
    let mut out = Vec::new();
    for k in 0..MAX_EDITS {
        let base = k * MODE_FIELDS;
        let hz = f[base + 2];
        if hz <= 0.0 {
            break;
        }
        out.push((f[base] as u16, f[base + 1] as u16, hz));
    }
    out
}

#[test]
fn the_mode_table_reaches_the_audio() {
    // Exposing frequency, gain and decay per partial costs nothing at runtime
    // and is native to this architecture and to no other. This checks that an
    // edit written the way the page writes it actually moves a partial.
    let table = Arc::new(ModeTable::new());
    table.load_json(
        &json!({ "edits": [{ "i": 1, "j": 0, "cents": 700.0, "db": 0.0, "decay": 1.0 }] }),
    );
    let set = Settings {
        object: 2,
        tune_hz: 220.0,
        modes: 8,
        decay_s: 6.0,
        hit: Point::new(0.13, 0.13),
        pos_l: Point::new(0.29, 0.29),
        pos_r: Point::new(0.29, 0.29),
        ..Settings::default()
    };
    let rows = published(&set, Some(table.clone()));
    let fundamental = rows
        .iter()
        .find(|(i, j, _)| *i == 1 && *j == 0)
        .expect("the fundamental is published");
    // 220 x 2^(7/12) = 329.63 Hz.
    assert!(
        (fundamental.2 - 329.63).abs() < 1.0,
        "a +700 cent edit on partial 1 put it at {} Hz instead of 329.63",
        fundamental.2
    );
    // And the round trip through the store's own JSON keeps it, identity and
    // all.
    let back = ModeTable::new();
    back.load_json(&table.to_json());
    let mut edits = [ModeEdit::default(); MAX_EDITS];
    back.read(&mut edits);
    assert_eq!(edits[0].i, 1);
    assert_eq!(edits[0].j, 0);
    assert!((edits[0].cents - 700.0).abs() < 1e-3);
}

#[test]
fn an_edit_follows_its_partial_when_the_selection_changes() {
    // The ruling this file exists to hold. An override is keyed by the mode's
    // own identity, `(i, j)`, and **not** by its row in the published frame.
    //
    // The failure it prevents is the kind this project keeps catching late: a
    // user drags a bar, retunes it, then changes Selection. The frame is now a
    // different set of partials in a different order, so a position-keyed
    // override would silently move to a resonance they never touched — and the
    // display would look entirely reasonable while doing it.
    let base = Settings {
        object: 3,      // Membrane: far more partials than the budget, so the
        tune_hz: 440.0, // ordering genuinely changes which are published.
        modes: 32,
        decay_s: 2.0,
        ..Settings::default()
    };
    let loud = Settings { order: 0, ..base };
    let low = Settings { order: 1, ..base };

    let plain_loud = published(&loud, None);
    let plain_low = published(&low, None);
    assert!(plain_loud.len() > 8 && plain_low.len() > 8);

    // Pick a partial the two orderings both publish but at **different rows**,
    // which is what makes the test able to fail.
    let mut chosen = None;
    for (pos, (i, j, _)) in plain_loud.iter().enumerate() {
        if let Some(other) = plain_low.iter().position(|(a, b, _)| a == i && b == j)
            && other != pos
        {
            chosen = Some((*i, *j, pos, other));
            break;
        }
    }
    let (ei, ej, row_loud, row_low) =
        chosen.expect("some partial is published at two different rows");

    let table = Arc::new(ModeTable::new());
    table.load_json(&json!({ "edits": [{ "i": ei, "j": ej, "cents": 700.0 }] }));
    let edited_loud = published(&loud, Some(table.clone()));
    let edited_low = published(&low, Some(table.clone()));

    // The edited partial moved, under both orderings, by the same amount.
    for (label, plain, edited) in [
        ("Loudest", &plain_loud, &edited_loud),
        ("Lowest", &plain_low, &edited_low),
    ] {
        let was = plain
            .iter()
            .find(|(i, j, _)| *i == ei && *j == ej)
            .expect("the partial is there before the edit")
            .2;
        let now = edited
            .iter()
            .find(|(i, j, _)| *i == ei && *j == ej)
            .expect("the partial is still there after the edit")
            .2;
        let moved = cents(now, was);
        assert!(
            (moved - 700.0).abs() < 1.0,
            "{label}: partial ({ei},{ej}) moved {moved:.1} cents, not 700"
        );
    }

    // And nothing else moved. Under Lowest the edited partial sits at a
    // different row from the one it had under Loudest, so a position-keyed
    // override would have shifted whatever is at that row instead — this is
    // the assertion that catches it.
    let _ = row_loud;
    let victim = plain_low[row_loud];
    assert_ne!(
        (victim.0, victim.1),
        (ei, ej),
        "the two rows must hold different partials for this test to mean anything"
    );
    let after = edited_low
        .iter()
        .find(|(i, j, _)| *i == victim.0 && *j == victim.1)
        .expect("the partial at the old row is still published");
    assert!(
        cents(after.2, victim.2).abs() < 1.0,
        "the partial at row {row_loud} moved {:.1} cents and nobody edited it",
        cents(after.2, victim.2)
    );
    let _ = row_low;
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
        "mode_budget",
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
    assert_eq!(
        MODE_FIELDS, 8,
        "i, j, hz, db_l, db_r, t60_s, db_bare, base_hz"
    );
    // Thirteen readouts and one available count per voice, appended so every
    // existing index stays where it was.
    assert_eq!(INFO_LEN, 13 + 2 * CHORD_VOICES);
    // `modes` is the stream; the parameter that spends the budget is
    // `mode_budget`. Two things called the same thing on one wire is a
    // collision somebody will hit, and the panel agent hit it.
    assert!(streams(SR).iter().any(|x| x.id == "modes"));
    assert!(!specs.iter().any(|x| x.id == "modes"));
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
        // **Every id named here must be a parameter this build publishes.**
        // Without this the list and the panel can each be right about
        // themselves and wrong about each other: `mode_budget` was published
        // as `modes` here for a whole build, which would have greyed out the
        // headline control on every object in the host while looking correct
        // in design mode. Found by the panel agent, live. An assertion that
        // the list contains a *particular* id cannot catch that — it agrees
        // with the same mistake — so the check is that nothing in the list is
        // unpublished.
        for u in uses {
            let id = u.as_str().expect("a used control is named by a string");
            assert!(
                specs.iter().any(|s| s.id == id),
                "object `{}` says it uses `{id}`, which this build does not publish",
                OBJECT_NAMES[i]
            );
        }
        // An air column has no mode list to truncate and no material.
        assert_eq!(has("mode_budget"), !guide);
        assert_eq!(has("material"), !guide);
        assert_eq!(has("radius"), guide);
        // And the contact coordinates the panel offers must be the ones the
        // audio thread reads. `PlateRound` was published as `xy` while `Walk`
        // and `Contacts::psi` read it as radius and angle, so the panel would
        // have offered a square for an object whose corners land on the rim,
        // where a clamped disc's every mode is zero.
        //
        // **This asserts against behaviour, not against the same `matches!`.**
        // Repeating the predicate here would agree with the mistake it is
        // supposed to catch.
        //
        // It used to read the *walk* — only a two-dimensional object varies
        // `j` — and the chord broke that premise rather than the code:
        // its partials are `(voice, harmonic)` and need two indices, while
        // its contacts are a position along one string and need one
        // coordinate. **Two indices and two coordinates are different
        // questions**, and a test that answered one while asking the other
        // was going to be wrong the first time they came apart.
        //
        // So it asks the contacts directly: move the second coordinate and
        // see whether any mode shape notices. A surface's does, a disc's does
        // because `y` is its angle, and a line's and a chord's do not.
        if !guide {
            let shape = Shape {
                object: Object::ALL[i],
                ..Shape::default()
            };
            // **Move a pickup, not the strike.** A disc takes the strike's
            // own angle as its zero — rotating the whole picture changes
            // nothing, so only the angle *between* the contacts can matter —
            // and probing the excitation would have reported that a round
            // membrane ignores its second coordinate. It does not; the
            // reference does.
            let exc = Point::new(0.37, 0.21);
            let at = |y: f32| {
                let p = Point::new(0.63, y);
                let c = object::Contacts::new(shape, exc, p, p);
                Walk::new(shape, 8.0)
                    .take(24)
                    .map(|q| c.psi(q.i, q.j).1)
                    .collect::<Vec<f32>>()
            };
            let (lo, hi) = (at(0.11), at(0.83));
            let uses_y = lo
                .iter()
                .zip(hi.iter())
                .any(|(a, b)| (a - b).abs() > 1e-6 * a.abs().max(1.0));
            let first_i = Walk::new(shape, 8.0)
                .next()
                .expect("an object has a fundamental")
                .i;
            let want = if !uses_y {
                "line"
            } else if first_i == 0 {
                "polar"
            } else {
                "xy"
            };
            assert_eq!(
                o["coords"], want,
                "`{}` publishes contact coordinates its own mode shapes do not use",
                OBJECT_NAMES[i]
            );
        } else {
            assert_eq!(o["coords"], "line", "an air column is one-dimensional");
        }
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

    // **And in the two states where the object has no end.** A negative
    // Inharm maps a partial to `n / sqrt(1 + |B| n^2)`, which asymptotes, so
    // every partial an ideal string has lies under about eighteen times the
    // fundamental; and a fundamental dragged to 1.25 Hz puts two hundred
    // million of a membrane's under 20 kHz. Both used to make the count run
    // past the end of a `usize` and the walk run without end, and the two
    // have to agree in exactly these cases or one of them is describing an
    // object the other does not render. Counting a million partials is
    // cheap; walking them is the slow half and is still under a second.
    for (object, inharm_b, max_ratio) in [
        (Object::String, -3.0e-3f32, 90.0f64),
        (Object::String, 0.0, 16_000.0),
        (Object::Membrane, -3.0e-3, 90.0),
        (Object::Membrane, 0.0, 16_000.0),
        (Object::MembraneRound, -3.0e-3, 90.0),
        (Object::PlateRound, -3.0e-3, 90.0),
        (Object::Plate, 0.0, 16_000.0),
        (Object::Beam, -3.0e-3, 90.0),
    ] {
        let shape = Shape {
            object,
            inharm_b,
            ..Shape::default()
        };
        let counted = shape.available(max_ratio);
        let walked = Walk::new(shape, max_ratio).count();
        assert_eq!(
            walked, counted,
            "{object:?} at B={inharm_b:e}, ratio {max_ratio}: walk {walked}, count {counted}"
        );
        assert!(
            counted <= object::MAX_CANDIDATES,
            "{object:?} reports {counted} partials, past the bound the search can finish"
        );
    }
}

#[test]
fn a_state_the_search_cannot_finish_does_not_wedge_the_instrument() {
    // **The worst bug of the build, found live by the panel agent.** Choose a
    // Membrane, drive Tune, Transpose and Fine each to its minimum so the
    // fundamental sits at 1.21 Hz, then load a preset. The device did not come
    // back: the object read String while the display still drew `Mode (1, 1)`,
    // a two-index mode a string does not have, and no parameter would bring it
    // round. Reloading the project was the only way out.
    //
    // Two faults, and it took both. At 1.21 Hz a membrane has of the order of
    // a hundred million partials under Nyquist, so the incremental search set
    // itself a task it could not finish. And the restart was written
    // `needs_rebuild(&req) && !searching()`, so every settings change that
    // arrived during a search was dropped — which, for a search that never
    // ended, meant every change for the rest of the session.
    //
    // So this drives the state and then changes the object, and asserts the
    // instrument is the new one and rings. It fails on either fault alone.
    let (bridge, ix) = build_bridge("noob-resonator-wedge", SR);
    let audio = bridge.take_audio().expect("audio handle");
    let set_p = |id: &str, v: f32| {
        let i = bridge
            .index_of(id)
            .unwrap_or_else(|| panic!("no parameter `{id}`"));
        bridge.set_param(i, v);
    };
    let mut e = Resonator::new(SR);
    let mut l = vec![0.0f32; bank::BLOCK];
    let mut r = vec![0.0f32; bank::BLOCK];
    let mut run = |e: &mut Resonator, n: usize| {
        for _ in 0..n {
            let s = read_settings(&audio, &ix);
            e.configure(&s);
            l.fill(0.0);
            r.fill(0.0);
            e.process(&mut l, &mut r);
        }
    };

    // A surface, with every pitch control at its minimum.
    set_p("type", 3.0);
    set_p("tune", 20.0);
    set_p("transpose", -48.0);
    set_p("fine", -50.0);
    run(&mut e, 40);

    // Then a preset, applied one parameter at a time the way the page applies
    // it, so the engine passes through the same intermediate states a user
    // does rather than being handed the answer in one step.
    let nylon = preset::factory()
        .into_iter()
        .find(|p| p.name == "Nylon")
        .expect("the Nylon preset");
    for (id, v) in preset::settings_values(&nylon.settings) {
        set_p(&id, v.as_f64().expect("a plain value") as f32);
        run(&mut e, 1);
    }
    run(&mut e, 400);

    // It is the object the parameters say it is.
    let f = e.info_frame();
    assert_eq!(
        f[10], 1.0,
        "the mode search never settled: build at {}",
        f[10]
    );
    assert!(
        (f[11] - 196.0).abs() < 1.0,
        "the fundamental is {} Hz and the preset asks for 196",
        f[11]
    );
    let info = e.bank().info();
    assert!(!info.is_empty(), "the bank is empty, so nothing can ring");
    assert!(
        info.iter().all(|m| m.j == 0),
        "a string has no two-index partials, and the bank holds {:?}",
        info.iter()
            .find(|m| m.j != 0)
            .map(|m| (m.i, m.j, m.hz))
            .unwrap()
    );
    assert!(
        (info[0].hz - 196.0).abs() < 1.0,
        "the lowest partial is at {} Hz, not the string's own 196",
        info[0].hz
    );

    // And it rings: a strike comes back out, rather than the silence of a
    // bank that was never rebuilt.
    let mut sl = vec![0.0f32; SR as usize / 2];
    let mut sr = vec![0.0f32; SR as usize / 2];
    sl[0] = 1.0;
    sr[0] = 1.0;
    e.process(&mut sl, &mut sr);
    let tail = &sl[SR as usize / 8..];
    let peak = tail.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    assert!(
        peak > 1.0e-4,
        "an eighth of a second after the strike the loudest sample is {peak:e}"
    );
}

#[test]
fn a_settings_change_during_a_search_is_not_dropped() {
    // The half of the wedge that is a fault on its own terms, isolated: a
    // change arriving mid-search used to be ignored until that search
    // finished, so the engine went on answering a question nobody had asked
    // since. This starts a long search, changes the object one block in —
    // long before it could finish — and asserts the engine followed.
    let long = Settings {
        object: 3,
        tune_hz: 20.0,
        transpose: -48.0,
        fine_cents: -50.0,
        ..Settings::default()
    };
    let mut e = Resonator::new(SR);
    let mut l = vec![0.0f32; bank::BLOCK];
    let mut r = vec![0.0f32; bank::BLOCK];
    e.configure(&long);
    e.process(&mut l, &mut r);
    assert!(
        e.info_frame()[10] < 1.0,
        "this state was supposed to take many blocks and settled at once"
    );

    let short = Settings {
        object: 2,
        tune_hz: 196.0,
        ..Settings::default()
    };
    e.configure(&short);
    for _ in 0..40 {
        e.process(&mut l, &mut r);
    }
    let f = e.info_frame();
    assert_eq!(f[10], 1.0, "the search did not restart on the new settings");
    assert!(
        e.bank().info().iter().all(|m| m.j == 0),
        "the bank still holds the abandoned search's two-index modes"
    );
}

#[test]
fn a_control_moving_every_block_still_lets_the_bank_follow() {
    // The failure the fix could have become. Abandoning a search whose
    // settings have changed is right, and abandoning on *every* block is a
    // second way to never finish one: under host automation the settings move
    // continuously, and a bank frozen on its old set looks exactly like a
    // bank that is following. So the abandoning is capped, and this is what
    // the cap is for — Tune swept a little every block, for longer than any
    // single search takes, with the bank required to have moved to the new
    // pitch by the end of it.
    let mut e = Resonator::new(SR);
    let mut l = vec![0.0f32; bank::BLOCK];
    let mut r = vec![0.0f32; bank::BLOCK];
    let base = Settings {
        object: 3,
        tune_hz: 110.0,
        ..Settings::default()
    };
    e.configure(&base);
    for _ in 0..400 {
        e.process(&mut l, &mut r);
    }
    let first = e.bank().info()[0].hz;
    assert!(
        (first - 110.0).abs() < 1.0,
        "the bank did not settle on the starting pitch, it is at {first}"
    );

    // Now move it, every single block, over a full octave.
    let mut hz = 110.0f32;
    for _ in 0..600 {
        hz *= 1.0011;
        e.configure(&Settings {
            tune_hz: hz,
            ..base
        });
        e.process(&mut l, &mut r);
    }
    let moved = e.bank().info()[0].hz;
    assert!(
        (moved - hz).abs() < 0.05 * hz,
        "Tune reached {hz:.1} Hz and the bank's lowest partial is still at {moved:.1}"
    );
    assert!(
        e.info_frame()[10] > 0.0,
        "the search made no progress at all while the control was moving"
    );
}

#[test]
fn the_parameters_that_are_counts_say_so_and_nothing_else_moved() {
    // The panel was printing "24.0" for a bank of 24 modes, and I tried twice
    // to make the *value* whole before concluding it could not be done here:
    // `steps` and a table taper both snap in the normalized domain before the
    // log taper, so a preset asking for 1,024 modes loads as 1,021. Measured,
    // both times, which is the only reason the conclusion is worth anything.
    //
    // `decimals` is the answer the framework grew for it, and it is a
    // statement about the value rather than a change to it. **So the property
    // worth testing is that nothing moved** — the failure it replaces was a
    // fix that quietly moved the value, and a hint that rounds would be that
    // fix wearing a different name.
    let specs = param_specs(false);
    let spec = |id: &str| {
        specs
            .iter()
            .find(|s| s.id == id)
            .unwrap_or_else(|| panic!("no parameter `{id}`"))
    };

    // Exactly the counts, and no others. `fine` is the one that has to stay
    // out: a cent is a real quantity and 12.5 of them means something.
    let declared: Vec<&str> = specs
        .iter()
        .filter(|s| s.decimals == Some(0))
        .map(|s| s.id.as_str())
        .collect();
    assert_eq!(
        declared,
        vec![
            "transpose",
            "mode_budget",
            "voices",
            "voice1",
            "voice2",
            "voice3",
            "voice4",
            "voice5",
            "voice6"
        ],
        "the parameters declaring themselves whole numbers are not the ones that are"
    );
    assert_eq!(spec("fine").decimals, None, "a cent is not a whole number");

    // The values a preset or a saved session actually carries, through the
    // same normalize/denormalize the bridge uses. These are the numbers that
    // came back as 1,021 under both of the fixes that changed the value.
    let budget = spec("mode_budget");
    for want in [4.0f32, 8.0, 24.0, 128.0, 512.0, 1024.0, 4096.0] {
        let got = budget.denormalize(budget.normalize(want));
        assert!(
            (got - want).abs() <= 0.001 * want,
            "Modes at {want} comes back as {got}"
        );
        assert_eq!(
            got.round(),
            want,
            "Modes at {want} rounds to {} in the engine",
            got.round()
        );
    }

    // And Transpose really is whole at every step, which is what lets it be
    // declared without a hint doing any work: 97 steps over a linear -48..48.
    let tr = spec("transpose");
    for step in 0..97 {
        let n = step as f32 / 96.0;
        let got = tr.denormalize(n);
        assert_eq!(
            got,
            got.round(),
            "Transpose step {step} is {got}, which is not a whole semitone"
        );
    }
    assert_eq!(tr.denormalize(0.0), -48.0);
    assert_eq!(tr.denormalize(1.0), 48.0);
}

#[test]
fn the_ruler_and_the_partials_come_from_the_same_moment() {
    // **Two streams, one picture.** The mode table is sticky and goes out only
    // when it changes; `info` goes out every block. So a page holding the
    // newest `info` and the last table it received was dividing one moment's
    // frequencies by another moment's fundamental — and the oscillator is
    // enough to do it, because it moves the pitch every block through the
    // retune path while the table is republished every `READOUT_BLOCKS`.
    // Measured before the fix, with the LFO at ordinary settings: a partial
    // whose ratio is exactly 1 drew at **1.2035x**, and 0.83x the other way.
    //
    // Found by the panel agent, who could see it and could not fix it: the
    // lowest *drawn* partial is not the fundamental in general — a strike on a
    // node removes partial 1 outright — so a page cannot infer its ruler from
    // the bars and must be given one it can trust.
    //
    // This holds the frames the way the page does, which is the whole point:
    // reading both accessors in the same breath cannot see the fault, because
    // in-process they are always current. `Processor::publish` sends the table
    // only when it differs, so the test keeps the last one that differed.
    let mut e = Resonator::new(SR);
    let mut l = vec![0.0f32; bank::BLOCK];
    let mut r = vec![0.0f32; bank::BLOCK];
    let set = Settings {
        object: 2, // String, whose partial `i` has ratio exactly `i`
        tune_hz: 220.0,
        inharm: 0.0,
        lfo_on: true,
        lfo_rate_hz: 2.0,
        lfo_depth_st: 12.0,
        ..Settings::default()
    };
    e.configure(&set);
    for _ in 0..600 {
        e.process(&mut l, &mut r);
    }

    let mut held = e.modes_frame().to_vec();
    let mut worst = 0.0f32;
    let mut worst_at = (0.0f32, 0.0f32);
    let mut checked = 0usize;
    for _ in 0..1200 {
        e.configure(&set);
        e.process(&mut l, &mut r);
        if held != e.modes_frame() {
            held.copy_from_slice(e.modes_frame());
        }
        let f0 = e.info_frame()[11];
        assert!(f0 > 0.0, "the published fundamental is {f0}");
        for row in held.chunks(MODE_FIELDS) {
            let (i, hz) = (row[0], row[2]);
            if hz <= 0.0 {
                break; // the table is terminated by hz = 0
            }
            // Partials the retune had to hold at Nyquist are not part of
            // this claim: `hz` is where the partial actually sounds, and one
            // pushed past the axis by the oscillator is clamped there rather
            // than allowed to fold. A page drawing that at 53 times the
            // fundamental is drawing the truth. So the ruler is checked
            // against the partials that are still where the series puts them.
            if i * f0 > 0.9 * SR * 0.49 {
                continue;
            }
            // An ideal string's partial `i` sits at exactly `i` times its
            // fundamental, so the ratio a page draws has to come back as `i`.
            let drawn = hz / f0;
            let err = (drawn - i).abs() / i;
            if err > worst {
                worst = err;
                worst_at = (i, drawn);
            }
            checked += 1;
        }
    }
    assert!(checked > 10_000, "only {checked} partials were checked");
    assert!(
        worst < 1e-4,
        "partial {} draws at {:.4} times the published fundamental, {:.2}% out",
        worst_at.0,
        worst_at.1,
        worst * 100.0
    );
}

#[test]
fn one_voice_is_bit_identical_to_no_voices_at_all() {
    // **The reason the voice went in `j` rather than `i`.** A mode has always
    // been named `(i, j)` with `j` left at zero on a one-dimensional object,
    // so a voice fits the free field exactly: nothing that anybody has saved
    // moves, the override table keeps its key, and the `modes` array inside
    // every preset keeps its meaning.
    //
    // Putting the voice in `i` — which is what I built before the ruling —
    // would have renamed every existing partial on every existing object.
    // This is the test that says the promise holds, and it is worth more than
    // the argument for it.
    for object in Object::ALL {
        if !object.can_voice() {
            continue;
        }
        let plain = Shape {
            object,
            ..Shape::default()
        };
        let voiced = Shape {
            object,
            voices: 1,
            // Deliberately not zero: turning the count down to one must give
            // the object at its own pitch, not at voice one's tuning.
            voice_semis: [7.0, -5.0, 3.0, 0.0, 0.0, 0.0],
            ..Shape::default()
        };
        let a: Vec<(u16, u16, f32)> = Walk::new(plain, 60.0)
            .map(|p| (p.i, p.j, p.ratio))
            .collect();
        let b: Vec<(u16, u16, f32)> = Walk::new(voiced, 60.0)
            .map(|p| (p.i, p.j, p.ratio))
            .collect();
        assert_eq!(
            a, b,
            "{object:?} with one voice walks differently from {object:?} with none"
        );
        assert!(
            a.iter().all(|(_, j, _)| *j == 0),
            "{object:?} with one voice published a partial with j != 0"
        );
        assert_eq!(
            plain.available(60.0),
            voiced.available(60.0),
            "{object:?}: the count moved when nothing about the object did"
        );
    }
}

#[test]
fn a_voice_transposes_the_objects_own_series_rather_than_replacing_it() {
    // The whole point of the ruling: a chord is a set of roots and each root
    // gets **this object's** series. A chord of beams is six copies of a
    // bar's inharmonic series, not six harmonic ladders — which is the thing
    // a bank of tuned combs cannot do at any setting.
    //
    // Equal temperament is a definition rather than physics, so the intervals
    // are checked against published deviations from just intonation: those
    // come from ratios of small integers rather than twelfth roots of two, so
    // agreement is a real second opinion rather than arithmetic meeting
    // itself.
    for object in [Object::Beam, Object::String, Object::Tine, Object::Marimba] {
        let shape = Shape {
            object,
            voices: 6,
            voice_semis: [0.0, 4.0, 7.0, 9.0, 12.0, 2.0],
            ..Shape::default()
        };
        let plain = Shape {
            object,
            ..Shape::default()
        };
        for (voice, name, just, want) in [
            (0usize, "unison", 1.0f64, 0.0f64),
            (1, "major third against 5:4", 5.0 / 4.0, 13.686),
            (2, "fifth against 3:2", 3.0 / 2.0, -1.955),
            (3, "major sixth against 5:3", 5.0 / 3.0, 15.641),
            (4, "octave against 2:1", 2.0, 0.0),
            (5, "major second against 9:8", 9.0 / 8.0, -3.910),
        ] {
            let got = 1200.0 * (shape.voice_ratio(voice as u16) / just).log2();
            assert!(
                (got - want).abs() < 0.01,
                "{object:?}: the {name} comes out {got:.3} cents from just, published {want:.3}"
            );
            // And every mode of that voice is the object's own mode, moved.
            for i in 1..=12u16 {
                let moved = shape.ratio(i, voice as u16);
                let own = plain.ratio(i, 0) * shape.voice_ratio(voice as u16);
                assert!(
                    (moved - own).abs() < 1e-9 * own.max(1.0),
                    "{object:?} voice {voice} mode {i} is at {moved} and the transposed series puts it at {own}"
                );
            }
        }
    }
}

#[test]
fn a_voiced_walk_covers_every_voice_whatever_order_they_are_tuned_in() {
    // The voices are numbered as the user tuned them, never sorted, because
    // an override is keyed to one — so the walk cannot assume the columns
    // rise and must not stop at the first voice above the ceiling. A voice
    // two octaves below the others is the case that catches it.
    for object in [Object::String, Object::Beam, Object::Tine] {
        let shape = Shape {
            object,
            voices: 4,
            voice_semis: [24.0, -24.0, 19.0, 7.0, 0.0, 0.0],
            ..Shape::default()
        };
        let max = 40.0;
        let walked: Vec<(u16, u16)> = Walk::new(shape, max).map(|p| (p.i, p.j)).collect();
        assert_eq!(
            walked.len(),
            shape.available(max),
            "{object:?}: the walk yields {} partials and the count says {}",
            walked.len(),
            shape.available(max)
        );
        for v in 0..4u16 {
            assert!(
                walked.iter().any(|(_, j)| *j == v),
                "{object:?}: voice {v} has no partials in the walk at all"
            );
        }
        assert!(
            !walked.iter().any(|(_, j)| *j > 3),
            "{object:?}: a voice beyond the count is sounding"
        );
        let mut seen = walked.clone();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(
            seen.len(),
            walked.len(),
            "{object:?}: two partials share an (i, j)"
        );
    }
}

#[test]
fn a_surface_does_not_offer_voices_and_says_why() {
    // The four two-dimensional objects use `j` for their second lattice
    // index, so a voice there needs a third field and a migration. That is a
    // decision deferred on evidence, not a limit of the physics, and the meta
    // has to say which — a greyed control with no reason is the thing this
    // project keeps finding.
    let meta = object_meta();
    let list = meta.as_array().unwrap();
    for (i, object) in Object::ALL.iter().enumerate() {
        let uses = list[i]["uses"].as_array().unwrap();
        let offers = uses.iter().any(|u| u == "voices");
        assert_eq!(
            offers,
            object.can_voice(),
            "`{}` offers voices: {offers}, and can carry them: {}",
            OBJECT_NAMES[i],
            object.can_voice()
        );
        if !object.can_voice() {
            let note = list[i]["note"].as_str().unwrap_or("");
            assert!(
                note.contains("third mode index"),
                "`{}` has no voices and does not say why: {note:?}",
                OBJECT_NAMES[i]
            );
        }
        // Voices are orthogonal to the engine, not to one of them: an air
        // column is one-dimensional and gets them too.
        if matches!(object, Object::Pipe | Object::Tube) {
            assert!(offers, "an air column is a line and should carry voices");
        }
    }
}

#[test]
fn every_published_chord_is_one_the_parameters_can_hold() {
    // The dictionary is applied by writing the voice parameters, so a chord
    // naming a pitch outside their range would load as something other than
    // itself and nothing would say so. Checked against the specs rather than
    // against a repeated constant.
    let specs = param_specs(false);
    let voices = specs.iter().find(|s| s.id == "voices").unwrap();
    let v1 = specs.iter().find(|s| s.id == "voice1").unwrap();
    let mut names: Vec<&str> = Vec::new();
    for c in object::CHORDS {
        assert!(
            !c.semis.is_empty() && c.semis.len() <= CHORD_VOICES,
            "`{}` has {} voices and the engine has {CHORD_VOICES}",
            c.name,
            c.semis.len()
        );
        assert!(
            (c.semis.len() as f32) >= voices.min && (c.semis.len() as f32) <= voices.max,
            "`{}` needs {} voices, outside what `voices` can hold",
            c.name,
            c.semis.len()
        );
        for semi in c.semis {
            assert!(
                *semi >= v1.min && *semi <= v1.max && semi.fract() == 0.0,
                "`{}` names {semi} semitones, which a voice parameter cannot hold",
                c.name
            );
        }
        // Ascending, because the dictionary is a voicing and a reader reads
        // it as one; the *parameters* may be in any order the user likes.
        assert!(
            c.semis.windows(2).all(|w| w[1] > w[0]),
            "`{}` is not written lowest voice first",
            c.name
        );
        names.push(c.name);
    }
    let total = names.len();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), total, "two chords share a name");
}

#[test]
fn every_sounding_voice_keeps_a_partial_in_the_published_set() {
    // **The panel agent's finding, turned into the guard.** The `modes` cut
    // is over the whole table and so distributes by level, and a chord
    // voicing is not level: at an ordinary six-voice spread they measured a
    // string publishing 49/12/3/0/0/0 partials per voice and a membrane
    // 61/3/0/0/0/0. Three sounding voices with no bars, and four \u2014 the user
    // hears six and sees two, with nothing on the face able to tell "silent"
    // from "lost the cut".
    //
    // So this builds exactly that: six voices with a level ladder steep
    // enough that the quiet ones would certainly lose, and asserts each one
    // still has a row. It is the same rule as the edited mode that is always
    // published, and it fails without the fix rather than passing by luck.
    for object in [Object::String, Object::Beam, Object::Tine, Object::Pipe] {
        let ix = Object::ALL.iter().position(|o| *o == object).unwrap();
        let set = Settings {
            object: ix,
            tune_hz: 110.0,
            voices: 6,
            // A spread voicing: the top voices are higher, quieter under the
            // tilt, and much fewer partials fit under the ceiling.
            voice_semis: [0.0, 12.0, 24.0, 31.0, 34.0, 36.0],
            bright_db_oct: -6.0,
            decay_s: 3.0,
            ..Settings::default()
        };
        let mut e = Resonator::new(SR);
        e.configure(&set);
        let mut l = vec![0.0f32; bank::BLOCK];
        let mut r = vec![0.0f32; bank::BLOCK];
        for _ in 0..600 {
            e.process(&mut l, &mut r);
        }
        let frame = e.modes_frame();
        let mut per_voice = [0usize; CHORD_VOICES];
        for row in frame.chunks(MODE_FIELDS) {
            if row[2] <= 0.0 {
                continue;
            }
            let v = row[1] as usize;
            assert!(v < CHORD_VOICES, "{object:?}: a row claims voice {v}");
            per_voice[v] += 1;
        }
        for (v, count) in per_voice.iter().enumerate() {
            assert!(
                *count > 0,
                "{object:?}: voice {v} is sounding and has no partial in the published set, — {per_voice:?}"
            );
        }
    }
}

#[test]
fn a_voice_says_how_many_partials_it_has_and_not_only_how_many_are_drawn() {
    // The other half of the same ruling. A voice reduced to one bar reads as
    // a voice with one partial unless the page can say "one of sixty-four",
    // and it can only say that if the count is published. NaN for a voice
    // that is not sounding, which is this contract's rule for every field
    // that does not apply.
    for object in [Object::String, Object::Beam, Object::Pipe] {
        let ix = Object::ALL.iter().position(|o| *o == object).unwrap();
        for voices in [1usize, 4, 6] {
            let set = Settings {
                object: ix,
                tune_hz: 110.0,
                voices,
                voice_semis: [0.0, 7.0, 12.0, 19.0, 24.0, 31.0],
                ..Settings::default()
            };
            let mut e = Resonator::new(SR);
            e.configure(&set);
            let mut l = vec![0.0f32; bank::BLOCK];
            let mut r = vec![0.0f32; bank::BLOCK];
            for _ in 0..600 {
                e.process(&mut l, &mut r);
            }
            let f = e.info_frame();
            let mut sum = 0.0f32;
            for v in 0..CHORD_VOICES {
                let got = f[13 + v];
                if v < voices {
                    assert!(
                        got.is_finite() && got >= 1.0 && got.fract() == 0.0,
                        "{object:?} with {voices} voices: voice {v} publishes {got}"
                    );
                    sum += got;
                } else {
                    assert!(
                        got.is_nan(),
                        "{object:?}: voice {v} is not sounding and publishes {got}"
                    );
                }
            }
            // A higher voice has less room under the ceiling, so the counts
            // fall as the voices rise — which is the fact that makes the
            // published number worth having.
            if voices > 1 {
                assert!(
                    f[13] >= f[13 + voices - 1],
                    "{object:?}: the root voice has fewer partials than the top one"
                );
            }
            // And they add up to what the whole object reports.
            assert!(
                (sum - f[1]).abs() <= 1.0,
                "{object:?} with {voices} voices: the per-voice counts total {sum} and the object reports {}",
                f[1]
            );
        }
    }
}

#[test]
fn a_held_chord_sounds_at_the_notes_that_are_held() {
    // **The capability a resonator tuned from one note does not have.** Six
    // held notes set six pitches, and each voice has to come out at its own
    // note's frequency whatever Tune is set to — otherwise it is a transposer
    // rather than an instrument you can play a chord into.
    //
    // Measured through the whole path: notes into the processor, settings
    // through the parameter struct, partials out of the published table.
    for root in [55.0f32, 220.0, 440.0] {
        let mut proc = Processor::new(SR);
        // C major seven, played rather than dialled.
        let notes = [60u8, 64, 67, 71];
        for n in notes {
            proc.note_on(n);
        }
        let set = Settings {
            object: 2, // String
            tune_hz: root,
            midi_voices: true,
            decay_s: 3.0,
            ..Settings::default()
        };
        let mut l = vec![0.0f32; bank::BLOCK];
        let mut r = vec![0.0f32; bank::BLOCK];
        for _ in 0..600 {
            proc.configure(&set);
            proc.process(&mut l, &mut r);
        }
        let frame = proc.engine().modes_frame().to_vec();
        for (v, note) in notes.iter().enumerate() {
            let want = 440.0 * 2f32.powf((*note as f32 - 69.0) / 12.0);
            // The voice's own fundamental is its mode 1.
            let got = frame
                .chunks(MODE_FIELDS)
                .find(|row| row[2] > 0.0 && row[1] as usize == v && row[0] == 1.0)
                .map(|row| row[2]);
            let got = got.unwrap_or_else(|| {
                panic!("at root {root}, voice {v} has no fundamental in the published table")
            });
            let cents = 1200.0 * (got / want).log2();
            assert!(
                cents.abs() < 1.0,
                "at root {root} Hz, note {note} sounds at {got:.2} Hz and should be {want:.2} \
                 ({cents:+.2} cents)"
            );
        }
    }
}

#[test]
fn a_held_note_keeps_its_voice_when_another_arrives() {
    // Assignment has to be **stable**, because a per-mode override is keyed
    // to a voice: reshuffling under a held chord would move a user's edits
    // onto partials they never touched, which is the same fault as keying an
    // edit by its row in the display. So holding a chord and adding a note
    // must not move the notes already down.
    let mut v = Voicing::default();
    v.note_on(60);
    v.note_on(67);
    let mut before = [0.0f32; CHORD_VOICES];
    v.semis(220.0, &mut before);
    v.note_on(64);
    let mut after = [0.0f32; CHORD_VOICES];
    v.semis(220.0, &mut after);
    assert_eq!(
        before[0], after[0],
        "the first held note moved when a third arrived"
    );
    assert_eq!(
        before[1], after[1],
        "the second held note moved when a third arrived"
    );
    assert_eq!(v.count(), 3);

    // Releasing frees exactly that voice and leaves the others where they are.
    v.note_off(60);
    assert!(!v.is_held(0), "the released note still holds its voice");
    assert!(v.is_held(1) && v.is_held(2), "a release moved the others");

    // A seventh note with every voice taken is ignored rather than stealing,
    // because stealing is the reshuffle this type exists to prevent.
    let mut full = Voicing::default();
    for n in 60..66u8 {
        full.note_on(n);
    }
    assert_eq!(full.count(), CHORD_VOICES);
    full.note_on(72);
    assert_eq!(full.count(), CHORD_VOICES, "a seventh note took a voice");
    assert!(full.is_held(0), "a seventh note stole the first voice");
}

#[test]
fn midi_overrides_the_voice_pitches_and_never_writes_them() {
    // A parameter the audio thread wrote behind the host's back is a gesture
    // nothing recorded and an automation lane that fights the player. So
    // held notes override, and the manual pitches are exactly where the user
    // left them when the keys come up.
    let manual = [0.0f32, 3.0, 8.0, 0.0, 0.0, 0.0];
    let set = Settings {
        object: 2,
        tune_hz: 220.0,
        voices: 3,
        midi_voices: true,
        disperse: 0.31,
        voice_semis: manual,
        ..Settings::default()
    };
    let mut proc = Processor::new(SR);
    let mut l = vec![0.0f32; bank::BLOCK];
    let mut r = vec![0.0f32; bank::BLOCK];
    proc.note_on(72);
    proc.note_on(79);
    for _ in 0..300 {
        proc.configure(&set);
        proc.process(&mut l, &mut r);
    }
    // The settings the caller holds are untouched: the override lives on the
    // copy the engine was configured with.
    assert_eq!(set.voice_semis, manual, "MIDI wrote the voice parameters");

    // While held, the published source says so; the free voices report
    // manual, and a voice that is not sounding reports NaN.
    let f = proc.engine().info_frame();
    assert_eq!(f[19], 1.0, "voice 0 is held and does not say so");
    assert_eq!(f[20], 1.0, "voice 1 is held and does not say so");
    for v in 2..CHORD_VOICES {
        assert!(
            f[19 + v].is_nan(),
            "voice {v} is not sounding and reports {}",
            f[19 + v]
        );
    }

    // Keys up: the object returns to the manual chord rather than to silence
    // or to whatever was last played.
    proc.notes_off();
    for _ in 0..300 {
        proc.configure(&set);
        proc.process(&mut l, &mut r);
    }
    let f = proc.engine().info_frame();
    for v in 0..3 {
        assert_eq!(
            f[19 + v],
            0.0,
            "voice {v} still reads as held after the keys came up"
        );
    }
    let frame = proc.engine().modes_frame().to_vec();
    let third = frame
        .chunks(MODE_FIELDS)
        .find(|row| row[2] > 0.0 && row[1] as usize == 1 && row[0] == 1.0)
        .map(|row| row[2])
        .expect("voice 1 has a fundamental");
    let want = 220.0 * 2f32.powf(3.0 / 12.0);
    assert!(
        (1200.0 * (third / want).log2()).abs() < 1.0,
        "after the keys came up voice 1 is at {third:.2} Hz, not the manual {want:.2}"
    );
}

#[test]
fn a_slot_note_recalls_a_stored_chord_and_a_played_note_replaces_it() {
    // **Both recall paths the research documents**, and the division the lead
    // ruled: the page stores and names the six chords, the engine recalls
    // them from the six notes, because only the engine sees MIDI and only the
    // editor can move a parameter in a way the host records.
    let slots = Arc::new(SlotTable::new());
    slots.load_json(&json!({
        "slots": [
            { "semis": [0.0, 7.0, 16.0, 0.0, 0.0, 0.0], "voices": 3 },
            { "semis": [0.0, 5.0, 10.0, 15.0, 0.0, 0.0], "voices": 4 }
        ]
    }));
    assert!(slots.get(0).is_some() && slots.get(1).is_some());
    // A slot nobody stored recalls nothing rather than a chord of zeros.
    for k in 2..CHORD_VOICES {
        assert!(slots.get(k).is_none(), "slot {k} was never stored");
    }

    let mut v = Voicing::default();
    v.recall(slots.get(0).unwrap());
    assert!(v.from_slot());
    assert_eq!(v.count(), 3);
    let mut semis = [0.0f32; CHORD_VOICES];
    assert_eq!(v.semis(220.0, &mut semis), 3);
    assert_eq!(semis[1], 7.0, "the recalled fifth is not a fifth");

    // A played note is the more recent instruction and replaces the recall,
    // rather than the two fighting over the pitches.
    v.note_on(72);
    assert!(!v.from_slot(), "a played note left the slot in charge");
    assert_eq!(v.count(), 1);

    // And through the processor, which is where the notes actually arrive:
    // a slot note recalls, and releasing it does not un-recall, because a
    // recall is an instruction rather than a key held down.
    let mut proc = Processor::new(SR);
    proc.set_slots(slots.clone());
    proc.note_on(SLOT_NOTES[1]);
    proc.note_off(SLOT_NOTES[1]);
    let set = Settings {
        object: 2,
        tune_hz: 110.0,
        midi_voices: true,
        ..Settings::default()
    };
    let mut l = vec![0.0f32; bank::BLOCK];
    let mut r = vec![0.0f32; bank::BLOCK];
    for _ in 0..600 {
        proc.configure(&set);
        proc.process(&mut l, &mut r);
    }
    let f = proc.engine().info_frame();
    for v in 0..4 {
        assert!(
            f[13 + v].is_finite(),
            "slot 2 asked for four voices and voice {v} is not sounding"
        );
    }
    assert!(
        f[17].is_nan(),
        "slot 2 asked for four voices and five sound"
    );
    // Its second voice is a fourth above the root, which is what was stored.
    let frame = proc.engine().modes_frame().to_vec();
    let got = frame
        .chunks(MODE_FIELDS)
        .find(|row| row[2] > 0.0 && row[1] as usize == 1 && row[0] == 1.0)
        .map(|row| row[2])
        .expect("voice 1 has a fundamental");
    let want = 110.0 * 2f32.powf(5.0 / 12.0);
    assert!(
        (1200.0 * (got / want).log2()).abs() < 1.0,
        "the recalled fourth sounds at {got:.2} Hz and should be {want:.2}"
    );
}

#[test]
fn a_voiced_air_column_claims_no_single_length() {
    // A rank of six columns has six lengths, and `column_m` is one field.
    // Publishing voice one's and labelling it would put "air column 85.0 cm
    // (voice 1 of 3)" on the face, which is a number in the right place
    // describing something other than what a reader expects — the failure
    // this contract's NaN rule exists to prevent. So above one voice these
    // three go blank, and at one voice they are exactly what they always
    // were.
    for object in [Object::Pipe, Object::Tube] {
        let ix = Object::ALL.iter().position(|o| *o == object).unwrap();
        let one = Settings {
            object: ix,
            tune_hz: 110.0,
            voices: 1,
            ..Settings::default()
        };
        let many = Settings { voices: 3, ..one };
        let settle = |set: &Settings| {
            let mut e = Resonator::new(SR);
            e.configure(set);
            let mut l = vec![0.0f32; bank::BLOCK];
            let mut r = vec![0.0f32; bank::BLOCK];
            for _ in 0..400 {
                e.process(&mut l, &mut r);
            }
            e.info_frame()
        };
        let a = settle(&one);
        for (k, name) in [(6usize, "column_m"), (7, "loop_ms")] {
            assert!(
                a[k].is_finite() && a[k] > 0.0,
                "{object:?} at one voice publishes {} for {name}",
                a[k]
            );
        }
        // `open_hz` is finite rather than positive: a stopped pipe's far end
        // has no opening, and **that zero is a measurement** rather than an
        // uncomputed field. It is the distinction the NaN rule turns on, and
        // asserting "> 0" here would have been the test misreading it.
        assert!(
            a[8].is_finite(),
            "{object:?} at one voice publishes {} for open_hz",
            a[8]
        );
        let b = settle(&many);
        for (k, name) in [(6usize, "column_m"), (7, "loop_ms"), (8, "open_hz")] {
            assert!(
                b[k].is_nan(),
                "{object:?} at three voices publishes {} for {name}, which is one rank's length \
                 standing for six",
                b[k]
            );
        }
        // And the fields that *are* per-voice still say something, so this is
        // a refusal to answer one question rather than a loss of the readout.
        assert!(
            b[13].is_finite() && b[14].is_finite() && b[15].is_finite(),
            "{object:?}: the per-voice counts went blank too"
        );
    }
}

#[test]
fn no_setting_publishes_a_partial_count_that_cannot_be_one() {
    // Found live by the panel agent, driving every control to both ends: a
    // string at negative Inharm published 18,446,744,073,709,551,615
    // partials, which their page printed faithfully as "18446744073709552.0
    // k partials". It was an unsigned cast of an infinity — `uninharm`
    // returns one above the compression's asymptote, meaning "every partial
    // fits", which is true and is not a number a count can be. A membrane in
    // the same state overflowed the addition outright, and the mode search
    // never settled at all because it was walking a set with no end.
    //
    // So this drives the fields that are counts of things, over every object
    // and both ends of everything that can change how many partials there
    // are, and asserts they can be what they claim: finite, whole, not
    // negative, and inside what the engine will consider. The panel now
    // refuses a value that cannot be a count and says it refused, which is
    // the right guard on their side and is not a fix on mine.
    for object in 0..OBJECT_NAMES.len() {
        for tune in [20.0f32, 220.0, 4000.0] {
            for transpose in [-48.0f32, 0.0, 48.0] {
                for inharm in [-1.0f32, -0.5, 0.0, 1.0] {
                    for aspect in [0.05f32, 1.0, 20.0] {
                        let set = Settings {
                            object,
                            tune_hz: tune,
                            transpose,
                            inharm,
                            aspect,
                            ..Settings::default()
                        };
                        let where_ = format!(
                            "{} at {tune} Hz, {transpose:+} st, inharm {inharm}, ratio {aspect}",
                            OBJECT_NAMES[object]
                        );
                        let shape = set.shape();
                        let counted = shape.available((20_000.0 / set.base_hz()) as f64);
                        assert!(
                            counted <= object::MAX_CANDIDATES,
                            "{where_}: {counted} partials available"
                        );
                        let mut e = Resonator::new(SR);
                        e.configure(&set);
                        let mut a = vec![0.0f32; bank::BLOCK];
                        let mut b = vec![0.0f32; bank::BLOCK];
                        for _ in 0..8 {
                            e.process(&mut a, &mut b);
                        }
                        let f = e.info_frame();
                        for (k, field) in [(0usize, "modes_used"), (1, "modes_available")] {
                            let v = f[k];
                            assert!(
                                v.is_finite()
                                    && v >= 0.0
                                    && v.fract() == 0.0
                                    && v <= object::MAX_CANDIDATES as f32,
                                "{where_}: {field} published {v:e}, which cannot be a count"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn the_mode_search_settles_even_when_the_object_has_no_end() {
    // The other half of the same fault, and the one a user would have felt:
    // the panel sat at "still building the mode table" forever, because the
    // incremental search walks the object's whole mode set and the set had
    // no end. Now it is bounded, so every state settles — and the bound is
    // set so that the worst of them is still a fraction of a second.
    for (object, inharm, tune, transpose) in [
        (2usize, -1.0f32, 220.0f32, 0.0f32),
        (3, -1.0, 220.0, 0.0),
        (3, 0.0, 20.0, -48.0),
        (7, -1.0, 220.0, 0.0),
        (9, -1.0, 220.0, 0.0),
    ] {
        let set = Settings {
            object,
            tune_hz: tune,
            transpose,
            inharm,
            ..Settings::default()
        };
        let mut e = Resonator::new(SR);
        e.configure(&set);
        let mut a = vec![0.0f32; bank::BLOCK];
        let mut b = vec![0.0f32; bank::BLOCK];
        let mut blocks = 0;
        while e.info_frame()[10] < 1.0 && blocks < 4_000 {
            e.process(&mut a, &mut b);
            blocks += 1;
        }
        let seconds = (blocks * bank::BLOCK) as f32 / SR;
        assert!(
            blocks < 4_000,
            "{} at inharm {inharm} never finished building",
            OBJECT_NAMES[object]
        );
        assert!(
            seconds < 1.0,
            "{} at inharm {inharm} took {seconds:.2} s to settle",
            OBJECT_NAMES[object]
        );
        // And what it settled on is a real reading, not an absence: the bank
        // does not reach the top of the band here, so there is a wall to draw.
        assert!(
            e.info_frame()[12].is_finite(),
            "{} at inharm {inharm} truncates and published no ceiling",
            OBJECT_NAMES[object]
        );
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
    attach_mode_table(&bridge, table.clone(), Arc::new(SlotTable::new()));
    let before = table.generation();

    bridge
        .store_set(
            MODES_KEY,
            json!({ "edits": [{ "i": 7, "j": 3, "cents": -50.0, "db": -3.0, "decay": 2.0 }] }),
        )
        .expect("the store took the table");
    assert_ne!(table.generation(), before, "the hook did not fire");

    let mut edits = [ModeEdit::default(); MAX_EDITS];
    table.read(&mut edits);
    // Slots are filled in the order the page listed them; what identifies an
    // override is `(i, j)`, never where it sits in this array.
    assert_eq!(edits[0].i, 7);
    assert_eq!(edits[0].j, 3);
    assert!((edits[0].cents + 50.0).abs() < 1e-3);
    assert!((edits[0].db + 3.0).abs() < 1e-3);
    assert!((edits[0].decay - 2.0).abs() < 1e-3);
    assert!(edits[0].matches(7, 3));
    assert!(!edits[0].matches(7, 0), "j is part of the identity");
    assert!(!edits[1].is_set(), "a sparse table stays sparse");

    // Nonsense from a future version of the page must not be able to silence
    // the plug-in: an unknown key, an index out of range and a wild value are
    // all ignored rather than rejected.
    table.load_json(&json!({
        "edits": [
            { "j": 4, "cents": 1.0 },
            { "i": 1, "cents": 1e9, "who": "knows" },
        ],
        "future": true
    }));
    table.read(&mut edits);
    assert!(
        !edits[0].matches(0, 4),
        "an entry with no `i` addresses nothing"
    );
    assert_eq!(edits[0].i, 1, "the entry that named a partial survived");
    // Two octaves either way, widened from one so a partial can be moved onto
    // another object's series — a string's third partial reaches a bell's
    // tierce only 1,586 cents down.
    assert!(edits[0].cents <= 2400.0, "an absurd offset was not clamped");
    assert!(
        edits[0].cents >= 2399.0,
        "and it was clamped to the new limit"
    );

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

#[test]
fn the_beam_ratio_everyone_quotes_is_a_truncation_and_not_a_rounding() {
    // The free bar's second partial is exactly 2.756538507. Every reference
    // prints the series as "1 : 2.756 : 5.404 : 8.933", and the third and
    // fourth of those are correctly rounded while **2.756 is a truncation** —
    // rounded properly it is 2.757.
    //
    // This test exists so that nobody seeing 2.757 come out of the solver
    // "fixes" it toward the quotation. The solver is right and the quotation
    // is short by half a thousandth. Found by the panel agent, solving the
    // equation rather than copying the figure.
    let shape = Shape {
        object: Object::Beam,
        ..Shape::default()
    };
    let exact = shape.ratio(2, 0);
    assert!(
        (exact - 2.756_538_507).abs() < 1e-9,
        "the second partial is {exact}, and the roots of cos x cosh x = 1 give 2.756538507"
    );
    assert_eq!(format!("{exact:.3}"), "2.757");
    assert_ne!(format!("{exact:.3}"), "2.756");
    // The two the literature does round correctly, for contrast.
    assert_eq!(format!("{:.3}", shape.ratio(3, 0)), "5.404");
    assert_eq!(format!("{:.3}", shape.ratio(4, 0)), "8.933");
}

#[test]
fn the_marimba_is_the_one_object_with_no_equation_to_check_it_against() {
    // Every other object's series is the solution of an eigenvalue problem, so
    // a second implementation can disagree with it and the out-of-tree probe
    // does exactly that. **An arch-cut bar has no such solution.** Its ratios
    // are a target a maker works toward by removing material until partials
    // two and three land, so there is nothing for a solver to solve and
    // nothing for the probe to check.
    //
    // What can be asserted is what the literature states — the targets
    // themselves — and that the engine hits them exactly rather than
    // approximately. Anything more would be a fabricated tolerance around a
    // number nobody solved for, which is the failure this file exists to
    // avoid.
    let marimba = Shape {
        object: Object::Marimba,
        ..Shape::default()
    };
    assert_eq!(marimba.ratio(2, 0), 4.0, "two octaves, exactly, not fitted");
    assert_eq!(marimba.ratio(3, 0), 9.2, "Woodhouse's figure, exactly");
    let xylophone = Shape {
        object: Object::Marimba,
        bar_tuning: 1,
        ..Shape::default()
    };
    assert_eq!(xylophone.ratio(2, 0), 3.0, "a twelfth, exactly");

    // The two sources for the third partial differ by close to a whole tone
    // at that partial. It is a builder's choice — how deep the arch is cut —
    // and **averaging them would describe a bar nobody has made**, so both are
    // exposed and neither is picked for the user.
    let rossing = Shape {
        object: Object::Marimba,
        bar_third: 1,
        ..Shape::default()
    };
    assert_eq!(rossing.ratio(3, 0), 10.0);
    let apart = cents(10.0, 9.2);
    assert!(
        apart > 120.0 && apart < 160.0,
        "the two sources are {apart:.0} cents apart at the third partial"
    );

    // And the caveat that is easy to lose: the **mode shapes** are still the
    // uniform bar's. The arch moves those too and nothing published describes
    // the cut, so a marimba's node positions are the beam's and are marked as
    // modelling wherever they are used.
    let beam = Shape {
        object: Object::Beam,
        ..Shape::default()
    };
    let c_bar = Contacts::new(
        marimba,
        Point::new(0.3, 0.0),
        Point::new(0.3, 0.0),
        Point::new(0.3, 0.0),
    );
    let c_beam = Contacts::new(
        beam,
        Point::new(0.3, 0.0),
        Point::new(0.3, 0.0),
        Point::new(0.3, 0.0),
    );
    assert_eq!(c_bar.psi(2, 0).0, c_beam.psi(2, 0).0);
}

#[test]
fn a_drum_head_is_pinned_at_its_rim_and_free_at_its_centre() {
    // Two physical checks on the round membrane's mode shapes that its
    // frequencies cannot give: the rim is a node for **every** mode, because
    // the head is clamped there, and the centre is an antinode only for the
    // circularly symmetric ones — which is why striking a drum dead centre
    // thins it to those and is the reason the disc's contact controls are a
    // radius and an angle rather than an x and a y.
    //
    // Handed over by the panel agent, whose own Bessel solver reproduced them
    // independently before this engine existed.
    for (m, n) in [(0u16, 1u16), (0, 2), (1, 1), (2, 1), (3, 2), (5, 3)] {
        let shape = Shape {
            object: Object::MembraneRound,
            ..Shape::default()
        };
        let rim = Contacts::new(
            shape,
            Point::new(1.0, 0.0),
            Point::new(1.0, 0.0),
            Point::new(1.0, 0.0),
        );
        assert!(
            rim.psi(m, n).0.abs() < 1e-5,
            "mode ({m},{n}) is {} at the rim and a clamped head cannot move there",
            rim.psi(m, n).0
        );
        let centre = Contacts::new(
            shape,
            Point::new(0.0, 0.0),
            Point::new(0.0, 0.0),
            Point::new(0.0, 0.0),
        );
        let at_centre = centre.psi(m, n).0.abs();
        if m == 0 {
            assert!(
                at_centre > 0.5,
                "the circularly symmetric mode ({m},{n}) should be an antinode at the centre and is {at_centre}"
            );
        } else {
            assert!(
                at_centre < 1e-5,
                "mode ({m},{n}) has {m} nodal diameters and cannot move the centre, but reads {at_centre}"
            );
        }
    }
    // And two more of the published zeros, from the same hand-over.
    assert!((bessel_zero(0, 3) - 8.653_727_913).abs() < 1e-6);
    assert!((bessel_zero(1, 2) - 7.015_586_670).abs() < 1e-6);
}

#[test]
fn the_tine_is_a_cantilever_and_its_series_is_leissa_table_4_39() {
    // One sign away from the free bar's frequency equation — cos B cosh B = -1
    // rather than +1 — and a different instrument. Leissa, NASA SP-160,
    // Table 4.39 gives the clamped-free roots; MODAL.md §2.3 prints the same
    // five and the ratios they imply.
    let published = [1.875_104, 4.694_091, 7.854_757, 10.995_541, 14.137_168];
    for (k, want) in published.iter().enumerate() {
        let got = tine_eigenvalue(k + 1);
        assert!(
            (got - want).abs() < 5e-6,
            "beta_{} is {got} and Leissa Table 4.39 says {want}",
            k + 1
        );
    }
    // 1 : 6.267 : 17.55 : 34.39 : 56.84, from CORPUS.md §4.2 and MODAL.md §2.3.
    let ratios = [1.0f64, 6.267, 17.55, 34.39, 56.84];
    let shape = Shape {
        object: Object::Tine,
        ..Shape::default()
    };
    for (k, want) in ratios.iter().enumerate() {
        let got = shape.ratio(k as u16 + 1, 0);
        assert!(
            (got - want).abs() < 0.01,
            "tine partial {} is {got} and the published series says {want}",
            k + 1
        );
    }
    // And it is a different object rather than a beam with a knob turned: the
    // free bar's first overtone is at 2.76 and this one's is at 6.27, which is
    // two and a half octaves apart.
    let beam = Shape {
        object: Object::Beam,
        ..Shape::default()
    };
    assert!(shape.ratio(2, 0) / beam.ratio(2, 0) > 2.2);
}

#[test]
fn a_tine_is_clamped_at_one_end_and_free_at_the_other() {
    // The two boundary conditions are what make it a tine, and neither is
    // imposed by hand — both fall out of the rearranged mode shape, so if the
    // rearrangement were wrong they would fail rather than pass quietly.
    //
    // Clamped: the shape and its slope are both zero. Free: the shape is at
    // its largest, which is why a tine is struck and picked up near its tip.
    for n in [1usize, 2, 3, 6] {
        let at_zero = tine_shape(n, 0.0);
        assert!(
            at_zero.abs() < 1e-9,
            "mode {n} is {at_zero} at the clamped end and must be zero"
        );
        let h = 1e-6;
        let slope = (tine_shape(n, h) - tine_shape(n, 0.0)) / h;
        assert!(
            slope.abs() < 1e-3,
            "mode {n} has slope {slope} at the clamped end and a clamp holds the angle too"
        );
        let tip = tine_shape(n, 1.0).abs();
        assert!(tip > 1.0, "mode {n} is only {tip} at the free tip");
    }
    // Mass-normalised like every other family here, checked by integrating.
    for n in [1usize, 2, 4] {
        let steps = 20_001;
        let mut acc = 0.0f64;
        for i in 0..steps {
            let x = i as f64 / (steps - 1) as f64;
            let w = if i == 0 || i == steps - 1 { 0.5 } else { 1.0 };
            acc += w * tine_shape(n, x).powi(2);
        }
        let mean = acc / (steps - 1) as f64;
        assert!(
            (mean - 1.0).abs() < 1e-4,
            "tine mode {n} integrates to {mean}"
        );
    }
}

/// Write a preset's values into a real bridge, parameter by parameter and by
/// id, then read them back the way the audio thread does.
///
/// This is deliberately the **whole path the page uses** — id to parameter,
/// parameter to atomic, atomic to `Settings` — rather than a function compared
/// with its own inverse. A preset that cannot survive it is a preset that will
/// not survive a user loading it.
fn round_trip(values: &serde_json::Map<String, serde_json::Value>) -> Settings {
    let (bridge, ix) = build_bridge("noob-resonator-preset-test", SR);
    let audio = bridge.take_audio().expect("audio handle");
    for (id, v) in values {
        let i = bridge
            .index_of(id)
            .unwrap_or_else(|| panic!("a preset names `{id}`, which is not a parameter"));
        bridge.set_param(i, v.as_f64().expect("a plain number") as f32);
    }
    read_settings(&audio, &ix)
}

#[test]
fn every_setting_survives_a_preset_round_trip() {
    // A settings snapshot with **every field away from its default**, so that a
    // field missing from `settings_values` reads back as its default and fails
    // here. A preset system that silently drops one control is worse than
    // none: it looks like it worked.
    let s = Settings {
        object: 8,
        tune_hz: 313.0,
        transpose: -7.0,
        fine_cents: 21.0,
        modes: 333,
        order: 2,
        aspect: 2.75,
        bar_tuning: 1,
        bar_third: 1,
        voices: 5,
        midi_voices: true,
        disperse: 0.31,
        voice_semis: [-5.0, 2.0, 9.0, 14.0, 21.0, 33.0],
        radius_mm: 47.0,
        opening: 0.37,
        decay_s: 7.5,
        material: 0.42,
        damp_corner_hz: 3300.0,
        damp_hi: -1.7,
        tail: false,
        bright_db_oct: 2.5,
        inharm: -0.44,
        hit: Point::new(0.11, 0.62),
        pos_l: Point::new(0.27, 0.71),
        pos_r: Point::new(0.83, 0.19),
        spread: 0.66,
        width: 0.23,
        filter_on: true,
        filter_hz: 2200.0,
        filter_oct: 1.75,
        filter_post: true,
        lfo_on: true,
        lfo_shape: 5,
        lfo_rate_hz: 3.5,
        lfo_depth_st: 4.5,
        lfo_phase_deg: 90.0,
        bleed: 0.29,
        mix: 0.61,
        gain_db: -8.5,
        limiter: false,
        limit_ceil_db: -9.0,
        // Never in a preset; see `preset.rs`.
        bypass: false,
    };
    let d = Settings::default();
    let back = round_trip(&preset::settings_values(&s));

    // Every field, named, so a failure says which one was dropped.
    let close = |a: f32, b: f32| (a - b).abs() <= 1e-3 * a.abs().max(1.0);
    assert_eq!(back.object, s.object, "type");
    assert!(close(back.tune_hz, s.tune_hz), "tune");
    assert!(close(back.transpose, s.transpose), "transpose");
    assert!(close(back.fine_cents, s.fine_cents), "fine");
    assert!(
        (back.modes as i32 - s.modes as i32).abs() <= 2,
        "mode_budget: {} against {}",
        back.modes,
        s.modes
    );
    assert_eq!(back.order, s.order, "select");
    assert!(close(back.aspect, s.aspect), "ratio");
    assert_eq!(back.bar_tuning, s.bar_tuning, "bar_tuning");
    assert_eq!(back.bar_third, s.bar_third, "bar_third");
    assert!(close(back.radius_mm, s.radius_mm), "radius");
    assert!(close(back.opening, s.opening), "opening");
    assert!(close(back.decay_s, s.decay_s), "decay");
    assert!(close(back.material, s.material), "material");
    assert!(close(back.damp_corner_hz, s.damp_corner_hz), "damp_corner");
    assert!(close(back.damp_hi, s.damp_hi), "damp_hi");
    assert_eq!(back.tail, s.tail, "tail");
    assert!(close(back.bright_db_oct, s.bright_db_oct), "bright");
    assert!(close(back.inharm, s.inharm), "inharm");
    assert!(
        close(back.hit.x, s.hit.x) && close(back.hit.y, s.hit.y),
        "hit"
    );
    assert!(
        close(back.pos_l.x, s.pos_l.x) && close(back.pos_l.y, s.pos_l.y),
        "pos_l"
    );
    assert!(
        close(back.pos_r.x, s.pos_r.x) && close(back.pos_r.y, s.pos_r.y),
        "pos_r"
    );
    assert!(close(back.spread, s.spread), "spread");
    assert!(close(back.width, s.width), "width");
    assert_eq!(back.filter_on, s.filter_on, "filter_on");
    assert!(close(back.filter_hz, s.filter_hz), "filter_freq");
    assert!(close(back.filter_oct, s.filter_oct), "filter_width");
    assert_eq!(back.filter_post, s.filter_post, "filter_place");
    assert_eq!(back.lfo_on, s.lfo_on, "lfo_on");
    assert_eq!(back.lfo_shape, s.lfo_shape, "lfo_shape");
    assert!(close(back.lfo_rate_hz, s.lfo_rate_hz), "lfo_rate");
    assert!(close(back.lfo_depth_st, s.lfo_depth_st), "lfo_depth");
    assert!(close(back.lfo_phase_deg, s.lfo_phase_deg), "lfo_phase");
    assert!(close(back.bleed, s.bleed), "bleed");
    assert!(close(back.mix, s.mix), "mix");
    assert!(close(back.gain_db, s.gain_db), "gain");
    assert_eq!(back.limiter, s.limiter, "limiter");
    assert!(close(back.limit_ceil_db, s.limit_ceil_db), "limit_ceil");

    // And every one of those really was away from its default, or the block
    // above would pass on a field that is never written at all.
    assert_ne!(s.object, d.object);
    assert_ne!(s.tail, d.tail);
    assert_ne!(s.limiter, d.limiter);
    assert_ne!(s.filter_on, d.filter_on);
    assert_ne!(s.lfo_on, d.lfo_on);
    assert_ne!(s.filter_post, d.filter_post);
}

#[test]
fn every_factory_preset_is_one_a_page_could_load() {
    let specs = param_specs(false);
    let factory = preset::factory();
    assert!(
        factory.len() >= 15,
        "only {} factory presets",
        factory.len()
    );

    let mut names: Vec<&str> = factory.iter().map(|p| p.name).collect();
    names.sort_unstable();
    let before = names.len();
    names.dedup();
    assert_eq!(before, names.len(), "two factory presets share a name");

    for p in &factory {
        let json = p.to_json();
        assert_eq!(json["v"], preset::PRESET_VERSION);
        assert!(!p.description.is_empty(), "{} has no description", p.name);
        // `modes` is mandatory even when empty: an absent key would have to
        // mean something, and every meaning it could have is a trap.
        assert!(json["modes"].is_array(), "{} has no modes list", p.name);

        let values = json["values"].as_object().expect("values is an object");
        assert!(
            !values.contains_key("bypass"),
            "{} carries bypass, which is a transport control and not a sound",
            p.name
        );
        for (id, v) in values {
            let spec = specs
                .iter()
                .find(|s| s.id == *id)
                .unwrap_or_else(|| panic!("{} names `{id}`, which is not a parameter", p.name));
            let x = v.as_f64().unwrap() as f32;
            let (lo, hi) = (spec.min.min(spec.max), spec.min.max(spec.max));
            assert!(
                x >= lo - 1e-4 && x <= hi + 1e-4,
                "{}: `{id}` is {x}, outside {lo}..{hi}",
                p.name
            );
        }
        // And the whole thing survives the path a page would take.
        let back = round_trip(values);
        assert_eq!(
            back.object, p.settings.object,
            "{} loads as a different object",
            p.name
        );
        // The group has to name the object, or the browser files it wrongly.
        assert_eq!(
            p.group, OBJECT_NAMES[p.settings.object],
            "{} is grouped under the wrong object",
            p.name
        );
    }
}

#[test]
fn every_pair_differs_by_exactly_the_one_control_it_argues_about() {
    // A pair exists so a user meets an argument by accident rather than by
    // reading about it, and that only works if the two presets are otherwise
    // identical: one changed control is the experiment, two is an anecdote.
    // The browser finds them structurally, by looking for the two whose values
    // differ in exactly one id, so a stray edit does not break the label — it
    // stops the pair being found at all, quietly. This says so instead.
    let factory = preset::factory();
    let by_name = |n: &str| {
        factory
            .iter()
            .find(|p| p.name == n)
            .unwrap_or_else(|| panic!("the factory set has no preset `{n}`"))
    };
    let pairs = [
        ("A · Loudest Partials", "B · Lowest Partials", "select"),
        ("Piano Wire", "Harp Wire", "inharm"),
        ("Hammer at the Middle", "Hammer at a Seventh", "hit"),
        ("Sloped Strike", "Flat Strike", "bright"),
        ("Wood", "Bronze", "material"),
        ("Stopped Pipe", "Open Pipe", "opening"),
        ("Timpani", "Timpani, Bank Only", "tail"),
        ("Struck Triad", "Struck Six", "voices"),
    ];
    for (a, b, id) in pairs {
        let (x, y) = (by_name(a), by_name(b));
        let (vx, vy) = (
            preset::settings_values(&x.settings),
            preset::settings_values(&y.settings),
        );
        let differ: Vec<&String> = vx
            .iter()
            .filter(|(k, v)| vy.get(*k) != Some(*v))
            .map(|(k, _)| k)
            .collect();
        assert_eq!(
            differ,
            vec![id],
            "`{a}` and `{b}` should differ in `{id}` alone and differ in {differ:?}"
        );
        assert_eq!(x.group, y.group, "`{a}` and `{b}` are on different objects");
        assert!(
            x.modes.is_empty() && y.modes.is_empty(),
            "`{a}` and `{b}` argue about a control, so neither may also move a partial"
        );
    }

    // And no two presets share a name, because the browser addresses them by
    // one and a duplicate would make one of them unreachable.
    let mut names: Vec<&str> = factory.iter().map(|p| p.name).collect();
    let total = names.len();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), total, "two factory presets share a name");

    // **Now the other direction, which is the half that found a real bug.**
    // Everything above checks that each pair I meant is one control apart. It
    // cannot see a pair I did not mean, and the browser will label those too:
    // running this scan the first time turned up three, one of which was two
    // presets with identical values under different names — a duplicate, not
    // a pair. A preset that turns up in two pairs also makes the label
    // ambiguous, since the browser has no way to know which comparison was
    // the intended one.
    //
    // This is the panel's own detection, run here: every unordered pair of
    // presets whose values differ in exactly one id. The answer has to be the
    // list above and nothing else.
    let values: Vec<_> = factory
        .iter()
        .map(|p| (p.name, preset::settings_values(&p.settings)))
        .collect();
    let mut found: Vec<(&str, &str, String)> = Vec::new();
    for (i, (na, va)) in values.iter().enumerate() {
        for (nb, vb) in values.iter().skip(i + 1) {
            let differ: Vec<&String> = va
                .iter()
                .filter(|(k, v)| vb.get(*k) != Some(*v))
                .map(|(k, _)| k)
                .collect();
            assert!(
                !differ.is_empty(),
                "`{na}` and `{nb}` are the same preset under two names"
            );
            if let [only] = differ[..] {
                found.push((na, nb, only.clone()));
            }
        }
    }
    let mut want: Vec<(&str, &str, String)> = pairs
        .iter()
        .map(|(a, b, id)| (*a, *b, (*id).to_string()))
        .collect();
    found.sort_unstable();
    want.sort_unstable();
    assert_eq!(
        found, want,
        "the structural scan finds a different set of pairs than the ones written down"
    );
}

#[test]
fn the_figures_in_the_preset_prose_are_ones_this_engine_still_produces() {
    // Every number a preset quotes was measured on this engine at that
    // preset's own settings, and this holds the two together: the description
    // has to contain the figure and the engine has to still produce it, so a
    // change to either fails here rather than leaving a plausible sentence
    // that stopped being true.
    //
    // **This does not prove any figure correct.** It is the engine checked
    // against itself, which proves nothing about the physics; the physics is
    // checked elsewhere in this file against Leissa, Lehtonen and Abramowitz
    // and Stegun, and out of tree against a probe that has never seen this
    // code. What this catches is prose going stale.
    let factory = preset::factory();
    let by_name = |n: &str| {
        factory
            .iter()
            .find(|p| p.name == n)
            .unwrap_or_else(|| panic!("the factory set has no preset `{n}`"))
    };
    let says = |name: &str, figure: &str| {
        let p = by_name(name);
        assert!(
            p.description.contains(figure),
            "`{name}` no longer says `{figure}`: {}",
            p.description
        );
    };
    let settle = |set: &Settings| {
        let mut e = Resonator::new(SR);
        e.configure(set);
        let mut a = vec![0.0f32; bank::BLOCK];
        let mut b = vec![0.0f32; bank::BLOCK];
        for _ in 0..600 {
            e.process(&mut a, &mut b);
        }
        e
    };

    // The tilt pair, which is the whole reason the default tilt is not zero.
    for (name, mid, high) in [("Sloped Strike", 286usize, 0usize), ("Flat Strike", 0, 292)] {
        let e = settle(&by_name(name).settings);
        let info = e.bank().info();
        let got_mid = info
            .iter()
            .filter(|i| i.hz > 1500.0 && i.hz < 10_000.0)
            .count();
        let got_high = info.iter().filter(|i| i.hz > 10_000.0).count();
        assert_eq!(
            got_mid, mid,
            "`{name}` puts {got_mid} partials between 1.5 and 10 kHz"
        );
        assert_eq!(
            got_high, high,
            "`{name}` puts {got_high} partials above 10 kHz"
        );
    }
    says("Sloped Strike", "286");
    says("Flat Strike", "292");

    // Lehtonen's stiff string: where the sixteenth partial lands.
    //
    // **B comes from `Settings::inharm_b()` and must not be inlined here.**
    // The control is quadratic in `B`, deliberately, so that a real piano's
    // 3e-4 sits where a knob can be put on it rather than in the first pixel.
    // Reproducing that mapping in a test is how I put this preset three
    // orders of magnitude out while "correcting" it toward the published
    // figure: a second copy of a conversion is a second thing to get wrong,
    // and the copy is the one nobody re-derives.
    let wire = by_name("Piano Wire");
    let shape = Shape {
        object: Object::String,
        inharm_b: wire.settings.inharm_b(),
        ..Shape::default()
    };
    let stretch = 1200.0 * (shape.ratio(16, 0) / 16.0).log2();
    assert!(
        (stretch - 64.0).abs() < 0.5,
        "the sixteenth partial is {stretch:.1} cents sharp and the preset says 64"
    );
    says("Piano Wire", "64 cents");

    // The two air columns at the same note: one exactly twice the other.
    let mut lengths = Vec::new();
    for name in ["Stopped Pipe", "Open Pipe"] {
        lengths.push(settle(&by_name(name).settings).info_frame()[6]);
    }
    assert!(
        (lengths[0] - 0.57).abs() < 0.005,
        "the stopped column is {:.4} m and the preset says 0.57",
        lengths[0]
    );
    assert!(
        (lengths[1] - 1.14).abs() < 0.005,
        "the open column is {:.4} m and the preset says 1.14",
        lengths[1]
    );
    assert!(
        (lengths[1] / lengths[0] - 2.0).abs() < 0.01,
        "the open column should be twice the stopped one and is {:.4} times",
        lengths[1] / lengths[0]
    );
    says("Stopped Pipe", "0.57 m");
    says("Open Pipe", "1.14 m");

    // The tail pair: where the head stops being resolvable, and how much
    // energy the truncation left behind for the network to carry.
    let f = settle(&by_name("Timpani").settings).info_frame();
    assert!(
        (f[2] - 1040.0).abs() < 20.0,
        "the crossover is at {:.0} Hz and the pair says 1,040",
        f[2]
    );
    assert!(
        (f[3] + 26.6).abs() < 0.2,
        "the tail sits at {:.1} dB and the pair says -26.6",
        f[3]
    );
    says("Timpani, Bank Only", "1,040 Hz");
    says("Timpani", "1,040 Hz");
    says("Timpani", "-26.6 dB");
}

#[test]
fn the_hand_bell_puts_its_partials_on_a_bells_own_series() {
    // The preset that exists to prove the mode table earns its place. A bell's
    // partials are the standard minor-third set — hum 0.5, prime 1.0, tierce
    // 1.2, quint 1.5, nominal 2.0 — and a string's are 1, 2, 3, 4, 5, so each
    // override is the interval between them. Checked as ratios rather than as
    // the cent figures, so the arithmetic is verified and not merely copied.
    let bell = [0.5f32, 1.0, 1.2, 1.5, 2.0];
    let p = preset::factory()
        .into_iter()
        .find(|p| p.name == "Hand Bell")
        .expect("the Hand Bell preset");
    assert_eq!(p.modes.len(), bell.len());
    for (k, want) in bell.iter().enumerate() {
        let e = p.modes[k];
        let natural = (k + 1) as f32;
        let moved = natural * 2f32.powf(e.cents / 1200.0);
        assert!(
            (moved - want).abs() < 1e-3,
            "partial {} lands at {moved} and a bell's is at {want}",
            k + 1
        );
        assert_eq!(e.i, (k + 1) as u16, "keyed by the partial's own index");
        assert!(
            e.cents.abs() <= 2400.0,
            "the override is outside what the table accepts"
        );
    }
}

#[test]
fn a_partials_two_indices_are_unique_across_every_object() {
    // An override is keyed by `(i, j)`, so the moment two partials share a
    // pair the key silently stops being a key and one edit lands on several
    // resonances. Suggested by the panel agent, who asserts the same thing on
    // their side; it costs nothing and it guards the whole editing model.
    for object in Object::ALL {
        if object.engine() == object::Engine::Guide {
            continue;
        }
        for aspect in [1.0f32, 1.41, 0.6] {
            let shape = Shape {
                object,
                aspect,
                ..Shape::default()
            };
            let mut seen: Vec<(u16, u16)> = Walk::new(shape, 120.0).map(|p| (p.i, p.j)).collect();
            let total = seen.len();
            assert!(total > 3, "{object:?} yielded only {total} partials");
            seen.sort_unstable();
            seen.dedup();
            assert_eq!(
                seen.len(),
                total,
                "{object:?} at aspect {aspect} has two partials sharing an (i, j)"
            );
        }
    }
}

#[test]
fn a_field_that_does_not_apply_publishes_nan_and_never_zero() {
    // A real zero and an uncomputed zero are indistinguishable to a panel, and
    // a plausible zero is worse than a blank because it reads as a measurement
    // nothing made. The panel agent hit exactly this in their own stand-in,
    // where a zero-filled frame published "0.0 dB GR" for a limiter that had
    // never run.
    let settle = |set: &Settings| -> [f32; INFO_LEN] {
        let mut e = Resonator::new(SR);
        e.configure(set);
        let mut l = vec![0.0f32; bank::BLOCK];
        let mut r = vec![0.0f32; bank::BLOCK];
        let mut guard = 0;
        while e.info_frame()[10] < 1.0 && guard < 40_000 {
            e.process(&mut l, &mut r);
            guard += 1;
        }
        for _ in 0..8 {
            e.process(&mut l, &mut r);
        }
        e.info_frame()
    };

    // A mode bank has no bore and no far end.
    let bank_frame = settle(&Settings {
        object: 2,
        tune_hz: 220.0,
        modes: 512,
        limiter: false,
        ..Settings::default()
    });
    for (k, what) in [(6usize, "column_m"), (7, "loop_ms"), (8, "open_hz")] {
        assert!(
            bank_frame[k].is_nan(),
            "a mode bank published {} for {what}",
            bank_frame[k]
        );
    }
    assert!(
        bank_frame[4].is_nan(),
        "the limiter is off and its reduction still read {}",
        bank_frame[4]
    );
    // A string at 220 Hz has 90 partials and the bank holds all of them, so
    // there is no wall — and no wall is NaN, not a wall at zero hertz.
    assert!(
        bank_frame[12].is_nan(),
        "the bank holds every partial and still published a ceiling at {}",
        bank_frame[12]
    );
    // The ones that do apply are real numbers.
    for (k, what) in [
        (0usize, "modes_used"),
        (1, "modes_available"),
        (2, "crossover_hz"),
        (10, "build"),
        (11, "f0_hz"),
    ] {
        assert!(
            bank_frame[k].is_finite(),
            "{what} should be a number on a mode bank"
        );
    }

    // An air column has no mode list to truncate and no inharmonicity.
    let guide_frame = settle(&Settings {
        object: 6,
        tune_hz: 220.0,
        ..Settings::default()
    });
    for (k, what) in [
        (2usize, "crossover_hz"),
        (3, "tail_db"),
        (5, "inharm_b"),
        (12, "ceiling_hz"),
    ] {
        assert!(
            guide_frame[k].is_nan(),
            "an air column published {} for {what}",
            guide_frame[k]
        );
    }
    for (k, what) in [(6usize, "column_m"), (7, "loop_ms"), (11, "f0_hz")] {
        assert!(
            guide_frame[k].is_finite(),
            "{what} should be a number on an air column"
        );
    }
    assert_eq!(guide_frame[9], 1.0, "engine says waveguide");

    // And where a bank really is truncated, the wall is a frequency.
    let truncated = settle(&Settings {
        object: 3,
        tune_hz: 110.0,
        modes: 64,
        order: 1,
        ..Settings::default()
    });
    assert!(
        truncated[12].is_finite() && truncated[12] > 100.0,
        "a truncated bank should publish where it stops and published {}",
        truncated[12]
    );
}

#[test]
fn the_scaled_modified_bessel_matches_abramowitz_and_stegun() {
    // `e^-x I_m(x)`, Abramowitz and Stegun Table 9.8. Scaled because the
    // clamped plate needs it at arguments in the tens, where `I_m` itself is
    // astronomically large and the quotient it appears in is perfectly
    // ordinary.
    for (x, want) in [
        (1.0f64, 0.465_759_6),
        (2.0, 0.308_508_3),
        (5.0, 0.183_540_8),
    ] {
        let got = bessel_i_scaled(0, x);
        assert!(
            (got - want).abs() < 1e-6,
            "e^-x I0({x}) is {got} and A&S Table 9.8 gives {want}"
        );
    }
    // I_1(2) = 1.590637 by its own series, times e^-2. Checked against the
    // series rather than against a table, because the figure I first wrote
    // here from memory was wrong in the fifth place and the series is not.
    // I_1(2) = sum over k of 1/(k!(k+1)!), summed as running terms because
    // the factorials themselves overflow long before the series does.
    let mut term = 1.0f64;
    let mut series = term;
    for k in 1..40u32 {
        term /= (k as f64) * (k as f64 + 1.0);
        series += term;
    }
    let want = series * (-2.0f64).exp();
    let got = bessel_i_scaled(1, 2.0);
    assert!(
        (got - want).abs() < 1e-9,
        "e^-x I1(2) is {got} and its own series gives {want}"
    );
}

#[test]
fn the_clamped_disc_is_leissas_circular_plate() {
    // A circular plate clamped at its rim. The frequency parameter is the
    // published one, `lambda^2 = omega a^2 sqrt(rho h / D)`, and its first
    // four values are the standard tabulated set for this plate.
    let published = [10.2158f64, 21.26, 34.88, 39.771];
    let modes = [(0usize, 1usize), (1, 1), (2, 1), (0, 2)];
    for ((m, n), want) in modes.iter().zip(published.iter()) {
        let l = disc_root(*m, *n);
        let l2 = l * l;
        assert!(
            (l2 - want).abs() < 0.01,
            "({m},{n}) gives lambda^2 = {l2:.4} and the published value is {want}"
        );
    }
    // And the ratios that follow: 1, 2.08, 3.41, 3.89.
    let shape = Shape {
        object: Object::PlateRound,
        ..Shape::default()
    };
    for ((m, n), want) in modes.iter().zip([1.0f64, 2.081, 3.414, 3.893].iter()) {
        let got = shape.ratio(*m as u16, *n as u16);
        assert!(
            (got - want).abs() < 0.002,
            "({m},{n}) is at {got} and the published ratio is {want}"
        );
    }
}

#[test]
fn a_clamped_plate_spreads_where_a_membrane_crowds() {
    // The one square that separates the two discs. A membrane's frequency
    // goes as its eigenvalue and a plate's as the **square** of it, so their
    // partials go opposite ways as they rise: this is why a drum has a pitch
    // and a cymbal has a wash, on the same shape of object.
    let membrane = Shape {
        object: Object::MembraneRound,
        ..Shape::default()
    };
    let plate = Shape {
        object: Object::PlateRound,
        ..Shape::default()
    };
    // Both start at 1 by construction; by the fourth partial they are far
    // apart and the plate is above.
    let mut m_ratios: Vec<f64> = Walk::new(membrane, 40.0).map(|p| p.ratio as f64).collect();
    let mut p_ratios: Vec<f64> = Walk::new(plate, 40.0).map(|p| p.ratio as f64).collect();
    m_ratios.sort_by(|a, b| a.partial_cmp(b).unwrap());
    p_ratios.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert!(m_ratios.len() > 8 && p_ratios.len() > 8);
    assert!(
        p_ratios[3] > m_ratios[3] * 1.4,
        "the plate's fourth partial is {} and the membrane's {}",
        p_ratios[3],
        m_ratios[3]
    );
    // And the counting law differs: constant density against rising.
    assert_eq!(Object::PlateRound.density_exponent(), 1.0);
    assert_eq!(Object::MembraneRound.density_exponent(), 2.0);
}

#[test]
fn a_clamped_plate_is_held_flat_at_its_rim() {
    // "Clamped" is two conditions rather than a membrane's one: the plate
    // cannot move at the rim **and** cannot tilt there. The first falls out of
    // how the shape is written; the second is what the eigenvalue was solved
    // for, so it is the one that would expose a wrong root.
    for (m, n) in [(0usize, 1usize), (0, 2), (1, 1), (2, 1), (3, 2)] {
        let at_rim = disc_shape(m, n, 1.0);
        assert!(
            at_rim.abs() < 1e-6,
            "mode ({m},{n}) is {at_rim} at the rim and a clamp holds it at zero"
        );
        let h = 1e-5;
        let slope = (disc_shape(m, n, 1.0) - disc_shape(m, n, 1.0 - h)) / h;
        assert!(
            slope.abs() < 2e-3,
            "mode ({m},{n}) has slope {slope} at the rim and a clamp holds the angle too"
        );
    }
    // Mass-normalised like every other family, checked by integrating rather
    // than by trusting the constant that was stored.
    for (m, n) in [(0usize, 1usize), (1, 1), (2, 2)] {
        let steps = 20_001;
        let mut acc = 0.0f64;
        for i in 0..steps {
            let r = i as f64 / (steps - 1) as f64;
            let w = if i == 0 || i == steps - 1 { 0.5 } else { 1.0 };
            acc += w * disc_shape(m, n, r).powi(2) * r;
        }
        let mean = acc / (steps - 1) as f64 * if m == 0 { 2.0 } else { 1.0 };
        assert!(
            (mean - 1.0).abs() < 1e-3,
            "clamped plate mode ({m},{n}) integrates to {mean}"
        );
    }
}

#[test]
fn scratch_disp() {
    for disperse in [0.0f32, 0.25, 0.5, 1.0] {
        let mut g = guide::Guide::new(SR);
        g.configure(&guide::Settings {
            f0: 220.0,
            opening: 1.0,
            radius_mm: 20.0,
            decay: 4.0,
            tilt_db_oct: 0.0,
            disperse,
            hit: 0.107,
            pos_l: 0.213,
            pos_r: 0.379,
        });
        let r = g.resonances();
        let f1 = r.first().map(|x| x.hz).unwrap_or(0.0);
        let show: Vec<String> = [1usize, 3, 7, 15]
            .iter()
            .filter_map(|k| r.get(*k))
            .map(|x| {
                format!(
                    "{:.1}",
                    1200.0
                        * (x.hz
                            / (f1 * (r.iter().position(|y| y.hz == x.hz).unwrap() as f32 + 1.0)))
                            .log2()
                )
            })
            .collect();
        println!(
            "disperse {disperse}: f1={f1:.2} n={} stretch(cents at 2,4,8,16)={:?}",
            r.len(),
            show
        );
    }
}
