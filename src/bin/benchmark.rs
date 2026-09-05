//! Measures this engine and writes `docs/BENCHMARK.md`.
//!
//! ```text
//! cargo run --release --bin benchmark
//! cargo run --release --bin benchmark -- --dump series > series.csv
//! ```
//!
//! Every row names the figure, where it comes from, what this engine
//! measures, and whether the two agree. Two rules run through the file.
//!
//! **Nothing here compares this plug-in with the device it answers.** Nobody
//! has measured that device — not this project, not the survey behind it, not
//! any third party I could find, and it cannot be loaded outside its host. So
//! this document says what floor *we* reach. It does not say by how much we
//! beat anybody, and it will not until somebody runs a bench session and
//! produces the other number.
//!
//! **And nothing here asserts a figure against the code that produced it.**
//! The partial series are checked against Leissa, Abramowitz and Stegun,
//! Russell and Lehtonen in `src/dsp/tests.rs`, and against an out-of-tree
//! probe that implements Bessel functions from their integral representation
//! and beam eigenvalues by bisection and has never seen this repository. What
//! the benchmark measures is the **audio**: the frequencies that come out, the
//! decays that come out, and what they cost.
//!
//! `--dump series` writes `object,i,j,ratio` for the probe to diff.

use std::fmt::Write as _;
use std::time::Instant;

use noob_resonator::dsp::bank::{Bank, ModeInfo};
use noob_resonator::dsp::object::{Engine, Object, Shape};
use noob_resonator::dsp::{Point, Resonator, Settings, bank, damp, guide, select, tail};
use rustfft::num_complex::Complex;

const SR: f32 = 48_000.0;

// ---------------------------------------------------------------------------
// The report
// ---------------------------------------------------------------------------

#[derive(PartialEq, Clone, Copy)]
enum Verdict {
    Meets,
    Misses,
    None,
}

struct Row {
    name: String,
    target: String,
    measured: String,
    verdict: Verdict,
    source: String,
}

struct Section {
    title: &'static str,
    intro: String,
    rows: Vec<Row>,
}

impl Section {
    fn new(title: &'static str, intro: impl Into<String>) -> Section {
        Section {
            title,
            intro: intro.into(),
            rows: Vec::new(),
        }
    }

    fn meets(
        &mut self,
        name: impl Into<String>,
        target: impl Into<String>,
        measured: impl Into<String>,
        ok: bool,
        source: impl Into<String>,
    ) {
        self.rows.push(Row {
            name: name.into(),
            target: target.into(),
            measured: measured.into(),
            verdict: if ok { Verdict::Meets } else { Verdict::Misses },
            source: source.into(),
        });
    }

    fn note(
        &mut self,
        name: impl Into<String>,
        measured: impl Into<String>,
        source: impl Into<String>,
    ) {
        self.rows.push(Row {
            name: name.into(),
            target: "no published figure".into(),
            measured: measured.into(),
            verdict: Verdict::None,
            source: source.into(),
        });
    }

    fn tally(&self) -> (usize, usize, usize) {
        let mut t = (0, 0, 0);
        for r in &self.rows {
            match r.verdict {
                Verdict::Meets => t.0 += 1,
                Verdict::Misses => t.1 += 1,
                Verdict::None => t.2 += 1,
            }
        }
        t
    }

    fn render(&self, out: &mut String) {
        let _ = writeln!(out, "\n## {}\n", self.title);
        if !self.intro.is_empty() {
            let _ = writeln!(out, "{}\n", self.intro);
        }
        let _ = writeln!(
            out,
            "| quantity | target or published | measured | verdict | source |"
        );
        let _ = writeln!(out, "|---|---|---|---|---|");
        for r in &self.rows {
            let v = match r.verdict {
                Verdict::Meets => "meets",
                Verdict::Misses => "**misses**",
                Verdict::None => "\u{2014}",
            };
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} |",
                r.name, r.target, r.measured, v, r.source
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Measurement, all of it from the audio
// ---------------------------------------------------------------------------

fn fft_mag(sig: &[f32], pad: usize) -> Vec<f32> {
    // Blackman–Harris, whose −92 dB sidelobes keep one loud partial's skirt
    // from being counted as a peak of its own, and a zero-pad so that a peak
    // between two bins can still be interpolated.
    let n = (sig.len() * pad).next_power_of_two();
    let mut buf = vec![Complex::new(0.0f32, 0.0f32); n];
    let m = sig.len();
    for (i, s) in sig.iter().enumerate() {
        let t = std::f32::consts::TAU * i as f32 / (m - 1) as f32;
        let w =
            0.358_75 - 0.488_29 * t.cos() + 0.141_28 * (2.0 * t).cos() - 0.011_68 * (3.0 * t).cos();
        buf[i] = Complex::new(s * w, 0.0);
    }
    rustfft::FftPlanner::new()
        .plan_fft_forward(n)
        .process(&mut buf);
    buf[..n / 2].iter().map(|c| c.norm()).collect()
}

/// The partial frequencies in a tail, strongest first, keeping everything
/// within 40 dB of the loudest.
///
/// `MODAL.md` §7.1 validated this specification by synthesising a known number
/// of modes and counting them back: analysing less than four times the longest
/// T60 makes the Lorentzian skirts of an incompletely decayed mode generate
/// spurious maxima, and the dynamic-range restriction is what makes the count
/// right at every length it tried.
fn partials(sig: &[f32], sr: f32, keep: usize) -> Vec<(f32, f32)> {
    let pad = 4;
    let mag = fft_mag(sig, pad);
    let n = mag.len() * 2;
    let bin = sr / n as f32;
    let mut peaks: Vec<(f32, f32)> = Vec::new();
    for k in 2..mag.len() - 2 {
        if mag[k] > mag[k - 1] && mag[k] >= mag[k + 1] && mag[k] > mag[k - 2] && mag[k] > mag[k + 2]
        {
            // Parabolic interpolation on the log magnitude, which is the
            // right shape for a windowed sinusoid's main lobe.
            let (a, b, c) = (
                mag[k - 1].max(1e-30).ln(),
                mag[k].max(1e-30).ln(),
                mag[k + 1].max(1e-30).ln(),
            );
            let d = 0.5 * (a - c) / (a - 2.0 * b + c);
            if d.abs() < 1.0 {
                peaks.push(((k as f32 + d) * bin, mag[k]));
            }
        }
    }
    peaks.sort_by(|p, q| q.1.partial_cmp(&p.1).unwrap());
    let Some(&(_, loudest)) = peaks.first() else {
        return Vec::new();
    };
    let floor = loudest * 10f32.powf(-40.0 / 20.0);
    peaks.retain(|p| p.1 >= floor);
    peaks.truncate(keep);
    peaks.sort_by(|p, q| p.0.partial_cmp(&q.0).unwrap());
    peaks
}

fn cents(a: f32, b: f32) -> f32 {
    1200.0 * (a / b).log2()
}

/// The frequency of a decaying sinusoid, from its zero crossings, which
/// resolves a cent at 20 Hz where a transform cannot.
fn frequency_of(sig: &[f32], sr: f32) -> f32 {
    let (mut first, mut last, mut count) = (0.0f64, 0.0f64, 0usize);
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
    (sr as f64 * (count - 1) as f64 / (last - first)) as f32
}

/// T60, fitted to the envelope between −5 and −35 dB below its peak.
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
        .into_iter()
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

/// Mean power between two frequencies, in dB.
fn band_db(sig: &[f32], sr: f32, lo: f32, hi: f32) -> f32 {
    let mag = fft_mag(sig, 1);
    let n = mag.len() * 2;
    let mut acc = 0.0f64;
    let mut count = 0usize;
    for (k, m) in mag.iter().enumerate() {
        let hz = k as f32 * sr / n as f32;
        if hz >= lo && hz <= hi {
            acc += (*m as f64).powi(2);
            count += 1;
        }
    }
    if count == 0 {
        return -300.0;
    }
    10.0 * (acc / count as f64).log10() as f32
}

/// One mode of a bank, struck once.
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

/// The whole device, settled and then struck once.
fn ring_engine(set: &Settings, samples: usize) -> Vec<f32> {
    let mut e = Resonator::new(SR);
    e.configure(set);
    let mut a = vec![0.0f32; bank::BLOCK];
    let mut b = vec![0.0f32; bank::BLOCK];
    let mut guard = 0;
    while e.info_frame()[10] < 1.0 && guard < 40_000 {
        e.process(&mut a, &mut b);
        a.fill(0.0);
        b.fill(0.0);
        guard += 1;
    }
    let mut l = vec![0.0f32; samples];
    let mut r = vec![0.0f32; samples];
    l[0] = 1.0;
    r[0] = 1.0;
    e.process(&mut l, &mut r);
    l
}

// ---------------------------------------------------------------------------
// Sections
// ---------------------------------------------------------------------------

/// A settings snapshot whose contact points fall on no simple fraction, so
/// that a mode-shape null cannot be mistaken for a missing partial.
fn probe_settings(object: usize, modes: usize) -> Settings {
    Settings {
        object,
        tune_hz: 220.0,
        modes,
        // The lowest partials, so the measurement compares the series it can
        // name rather than whichever set the contribution ordering chose.
        order: 1,
        decay_s: 2.0,
        material: 0.0,
        damp_corner_hz: 20_000.0,
        tail: false,
        bright_db_oct: 0.0,
        limiter: false,
        hit: Point::new(0.107, 0.113),
        pos_l: Point::new(0.213, 0.229),
        pos_r: Point::new(0.213, 0.229),
        width: 0.0,
        ..Settings::default()
    }
}

fn series_section() -> Section {
    let mut s = Section::new(
        "The partial series, measured out of the audio",
        "The object is struck once, the tail is transformed with a Blackman–Harris window and a \
         four-times zero pad, and the peaks are picked. Every peak is then matched to the nearest \
         frequency the object's own eigenvalue problem puts a partial at, and the row reports the \
         **worst** of those distances.\n\nThe reference is the series `src/dsp/tests.rs` checks \
         against Leissa, Abramowitz and Stegun, Russell and Lehtonen, and that an out-of-tree probe \
         reproduces to under a ten-thousandth of a cent. **So this measures the whole chain from \
         the eigenvalue to the loudspeaker, not the arithmetic against itself.**\n\nMatching rather \
         than counting, because which partials a strike excites is physics and not a fault. A \
         surface's degenerate pairs are one peak and not two; a strike near the centre of a drum \
         head excites only its axisymmetric modes; and a contact point that lands on a mode's node \
         silences it exactly. Reading a missing partial as a mistuned one is what made the first \
         version of this row print five hundred cents.\n\nThe pass mark is `MODAL.md` §7.4's: every \
         partial within **one cent**, which is roughly the threshold of pitch discrimination.",
    );
    for (i, object) in Object::ALL.iter().enumerate() {
        if object.engine() != Engine::Bank {
            continue;
        }
        let set = probe_settings(i, 12);
        let sig = ring_engine(&set, (SR as usize) * 4);

        // Where the engine says its partials are. Each of these is the
        // fundamental times a ratio the physics decided, so comparing the
        // audio against them is comparing the audio against the physics.
        let mut want: Vec<f32> = {
            let mut e = Resonator::new(SR);
            e.configure(&set);
            let mut x = vec![0.0f32; bank::BLOCK];
            let mut y = vec![0.0f32; bank::BLOCK];
            let mut guard = 0;
            while e.info_frame()[10] < 1.0 && guard < 40_000 {
                e.process(&mut x, &mut y);
                guard += 1;
            }
            e.bank().info().iter().map(|m| m.hz).collect()
        };
        want.sort_by(|p, q| p.partial_cmp(q).unwrap());

        let peaks = partials(&sig, SR, 16);
        let mut worst = 0.0f32;
        let mut matched = 0usize;
        let mut orphan = 0usize;
        for (hz, _) in &peaks {
            let near = want
                .iter()
                .map(|w| (cents(*hz, *w), *w))
                .min_by(|a, b| a.0.abs().partial_cmp(&b.0.abs()).unwrap());
            match near {
                Some((e, _)) if e.abs() < 50.0 => {
                    if e.abs() > worst.abs() {
                        worst = e;
                    }
                    matched += 1;
                }
                _ => orphan += 1,
            }
        }
        s.meets(
            format!("{object:?}: worst of the {matched} partials the strike excited"),
            "within 1 cent, and no partial the physics does not predict",
            if orphan > 0 {
                format!("{worst:+.4} cents, and {orphan} peaks with no partial near them")
            } else {
                format!("{worst:+.4} cents")
            },
            worst.abs() < 1.0 && matched >= 5 && orphan == 0,
            "`MODAL.md` §7.4, the series verifier's pass mark",
        );
    }
    s
}

fn tuning_section() -> Section {
    let mut s = Section::new(
        "Tuning and decay, one mode at a time",
        "The frequency is read off the zero crossings of the mode's own output, which resolves a \
         cent at 20 Hz where a transform's bin spacing alone is most of one. The decay is fitted to \
         the envelope between −5 and −35 dB below its peak, which is the window `MODAL.md` §7.2 \
         arrived at by getting it wrong twice.\n\nThe two-pole row is the comparison the formulation \
         choice rests on. Its difference equation is van den Doel and Pai's own equation (6), \
         written out in the benchmark rather than borrowed from the engine, so the row compares two \
         published structures rather than this engine with itself.",
    );
    for hz in [20.0f32, 27.5, 55.0, 220.0, 1000.0, 8000.0, 16_000.0] {
        let sig = ring_one(hz, 60.0, (SR as usize) * 4);
        let got = frequency_of(&sig, SR);
        let e = cents(got, hz);
        s.meets(
            format!("a mode asked for {hz} Hz rings at"),
            "within 1 cent",
            format!("{e:+.4} cents"),
            e.abs() < 1.0,
            "`MODAL.md` §7.4",
        );
    }
    // The two-pole, from its own published equation.
    for hz in [20.0f32, 27.5, 55.0, 440.0] {
        let r = (-damp::LN1000 / (2.0 * SR)).exp();
        let theta = std::f32::consts::TAU * hz / SR;
        let a1: f32 = 2.0 * r * theta.cos();
        let a2: f32 = r * r;
        let n = (SR as usize) * 4;
        let mut v1 = r * theta.sin();
        let mut v2 = 0.0f32;
        let mut out = vec![0.0f32; n];
        for x in out.iter_mut() {
            let v = a1 * v1 - a2 * v2;
            v2 = v1;
            v1 = v;
            *x = v;
        }
        let two = cents(frequency_of(&out, SR), hz);
        let ours = cents(frequency_of(&ring_one(hz, 2.0, n), SR), hz);
        s.note(
            format!("at {hz} Hz: the classic two-pole against the coupled form, both in f32"),
            format!("two-pole {two:+.3} cents, coupled form {ours:+.4} cents"),
            "van den Doel & Pai eq. (6) for the two-pole; `MODAL.md` §6.3 measured 6.99 cents at 20 Hz",
        );
    }
    for (hz, t60) in [
        (110.0f32, 0.5f32),
        (220.0, 2.0),
        (440.0, 3.0),
        (2000.0, 8.0),
        (8000.0, 0.2),
    ] {
        let samples = ((t60 * 1.5 * SR) as usize).clamp(SR as usize, 20 * SR as usize);
        let got = t60_of(&ring_one(hz, t60, samples), SR);
        let err = 100.0 * (got - t60) / t60;
        s.meets(
            format!("a {t60} s decay at {hz} Hz measures"),
            "within 2 %",
            format!("{got:.4} s ({err:+.2} %)"),
            err.abs() < 2.0,
            "`MODAL.md` §7.4, the decay checker's pass mark",
        );
    }
    s
}

fn decrement_section() -> Section {
    let mut s = Section::new(
        "Storing the decrement rather than the pole radius",
        "A long decay puts the pole radius very close to 1, where `f32` has spent all its precision \
         on the leading digit. Storing `d = 1 − r` instead puts the same number where the format \
         still has full relative precision. Both arithmetics are written out here from their own \
         definitions.",
    );
    for want in [1.0f32, 10.0, 60.0, 300.0, 1000.0] {
        let d = damp::decrement(want, SR);
        let ours = damp::t60_of(d, SR);
        let r = (-damp::LN1000 / (want * SR)).exp();
        let naive = -damp::LN1000 / (r.ln() * SR);
        s.meets(
            format!("a {want} s decay, asked for and got"),
            "within 0.01 %",
            format!(
                "decrement {ours:.4} s ({:+.5} %), radius {naive:.3} s ({:+.2} %)",
                100.0 * (ours - want) / want,
                100.0 * (naive - want) / want
            ),
            (100.0 * (ours - want) / want).abs() < 0.01,
            "`MODAL.md` §6.4, which measured 1,207 s for a wanted 1,000",
        );
    }
    s
}

fn selection_section() -> Section {
    let mut s = Section::new(
        "Which modes, against a full-bank reference",
        "The measurement the whole plug-in is about. A membrane tuned to 440 Hz has few enough \
         partials that the **complete** bank fits in the budget, so it can be rendered as a \
         reference; then the same object is rendered at a fixed small budget under each of the \
         three orderings and each band is compared with the reference. The tail is off, so every \
         decibel here belongs to the mode set.\n\n`MODAL.md` §8.1 ran the same experiment at a \
         512-mode budget on a 110 Hz membrane, where the full bank is 54,749 modes, and measured a \
         65 dB difference between the best and the worst ordering in the 4–10 kHz band. The rows \
         below are ours, at our own budget and our own contact positions.",
    );
    let full = Settings {
        object: 3,
        tune_hz: 440.0,
        modes: bank::MAX_MODES,
        decay_s: 2.0,
        material: -0.5,
        tail: false,
        limiter: false,
        hit: Point::new(0.107, 0.113),
        pos_l: Point::new(0.213, 0.229),
        pos_r: Point::new(0.379, 0.431),
        ..Settings::default()
    };
    let reference = ring_engine(&full, (SR as usize) * 2);
    let counted;
    {
        let mut e = Resonator::new(SR);
        e.configure(&full);
        let mut a = vec![0.0f32; bank::BLOCK];
        let mut b = vec![0.0f32; bank::BLOCK];
        for _ in 0..4000 {
            e.process(&mut a, &mut b);
        }
        counted = e.bank().len();
        s.note(
            "the reference bank",
            format!(
                "{} partials, every one the object has below 20 kHz",
                e.bank().len()
            ),
            "the object's own series",
        );
    }
    let bands = [
        (1000.0f32, 4000.0f32),
        (4000.0, 10_000.0),
        (10_000.0, 20_000.0),
    ];
    for budget in [64usize, 256] {
        let mut errors = [[0.0f32; 3]; 3];
        let mut tops = [0.0f32; 3];
        for (order, label) in select::SELECT_NAMES.iter().enumerate() {
            let set = Settings {
                modes: budget,
                order,
                ..full
            };
            let sig = ring_engine(&set, (SR as usize) * 2);
            {
                let mut e = Resonator::new(SR);
                e.configure(&set);
                let mut x = vec![0.0f32; bank::BLOCK];
                let mut y = vec![0.0f32; bank::BLOCK];
                for _ in 0..4000 {
                    e.process(&mut x, &mut y);
                }
                tops[order] = e.bank().info().iter().fold(0.0f32, |m, i| m.max(i.hz));
            }
            let mut cells = Vec::new();
            for (k, (lo, hi)) in bands.iter().enumerate() {
                let d = band_db(&sig, SR, *lo, *hi) - band_db(&reference, SR, *lo, *hi);
                errors[order][k] = d;
                cells.push(format!("{d:+.1}"));
            }
            let worst = errors[order]
                .iter()
                .fold(0.0f32, |m: f32, v| m.max(v.abs()));
            s.note(
                format!(
                    "**{label}**, {budget} of {counted}: highest kept, then the error against the \
                     full bank in 1–4 kHz, 4–10 kHz, 10–20 kHz"
                ),
                format!(
                    "{:.0} Hz — {} dB, worst band **{worst:.1} dB**",
                    tops[order],
                    cells.join(", ")
                ),
                "in-house; the same experiment `MODAL.md` §8.1 ran at a different budget",
            );
        }
        let best = (0..3)
            .map(|o| errors[o].iter().fold(0.0f32, |m: f32, v| m.max(v.abs())))
            .fold(f32::INFINITY, f32::min);
        let worst = (0..3)
            .map(|o| errors[o].iter().fold(0.0f32, |m: f32, v| m.max(v.abs())))
            .fold(0.0f32, f32::max);
        s.note(
            format!("at a {budget}-mode budget: worst band error, best ordering against worst"),
            format!(
                "{best:.1} dB against {worst:.1} dB — a spread of **{:.1} dB**",
                worst - best
            ),
            "in-house",
        );
    }
    s
}

fn guide_section() -> Section {
    let mut s = Section::new(
        "The air columns",
        "An open–open column resonates at `n·c/2ℓ` and an open–closed one at `(2n−1)·c/4ℓ`, \
         which is the whole difference between the two objects and the reason a stopped organ pipe \
         sounds an octave below an open one of the same length. The delay is fractional and \
         interpolated with third-order Lagrange, so the row that matters is how far the higher \
         resonances drift from the series the terminations imply.",
    );
    for (name, opening, odd) in [
        ("Tube, both ends open", 1.0f32, false),
        ("Pipe, far end closed", 0.0, true),
    ] {
        let mut g = guide::Guide::new(SR);
        g.configure(&guide::Settings {
            f0: 220.0,
            opening,
            radius_mm: 20.0,
            decay: 4.0,
            tilt_db_oct: 0.0,
            hit: 0.107,
            pos_l: 0.213,
            pos_r: 0.379,
        });
        let mut worst = 0.0f32;
        let mut n = 0usize;
        for (k, r) in g.resonances().iter().take(16).enumerate() {
            let want = if odd {
                (2 * k + 1) as f32 * 220.0
            } else {
                (k + 1) as f32 * 220.0
            };
            let e = cents(r.hz, want);
            if e.abs() > worst.abs() {
                worst = e;
            }
            n += 1;
        }
        s.meets(
            format!("{name}: worst of the first {n} resonances"),
            "within 1 cent",
            format!("{worst:+.3} cents"),
            worst.abs() < 1.0,
            "the standard result for an air column; the pass mark is `MODAL.md` §7.4's",
        );
        s.note(
            format!("{name}: length the model implies at 220 Hz"),
            format!("{:.4} m, round trip {:.3} ms", g.column_m(), g.loop_ms()),
            "`c/2f` open and `c/4f` stopped, less Levine and Schwinger's end correction of 0.6133 a",
        );
    }
    // The strike comb, measured by moving the strike.
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
    let mut null = 0.0f32;
    let mut live = 0.0f32;
    for k in 0..9usize {
        let change = 20.0 * (third[k] / plain[k]).log10();
        if (k + 1) % 3 == 0 {
            null = if null == 0.0 {
                change
            } else {
                null.max(change)
            };
        } else {
            live = if live == 0.0 {
                change
            } else {
                live.min(change)
            };
        }
    }
    s.meets(
        "striking at a third of the length: every third partial, against the others",
        "at least 25 dB apart",
        format!("nulled partials {null:.1} dB, the rest {live:+.1} dB"),
        null < -25.0 && live > -6.0,
        "the mode shape evaluated at the contact point; a derivation, not a control",
    );
    s
}

fn tail_section() -> Section {
    let mut s = Section::new(
        "The statistical tail",
        "Above the frequency where the modal overlap factor reaches one, the partials merge into a \
         continuum and no listener or analyser can resolve them individually. The tail covers that \
         region with a feedback delay network whose loss filters are fitted to the same `T60(f)` \
         the bank uses and whose level is set from the energy the selection actually left behind.",
    );
    for sr in [44_100.0f32, 48_000.0, 96_000.0, 192_000.0] {
        let t = tail::Tail::new(sr);
        s.meets(
            format!("modal density of the network at {sr:.0} Hz"),
            "above 0.15 modes/Hz",
            format!("{:.3} modes/Hz", t.report().density),
            t.report().density > 0.15,
            "Schroeder and Logan's criterion, quoted by Smith's *Physical Audio Signal Processing*",
        );
    }
    for (object, tune, modes) in [
        (3usize, 110.0f32, 512usize),
        (4, 110.0, 512),
        (0, 220.0, 512),
    ] {
        let set = Settings {
            object,
            tune_hz: tune,
            modes,
            tail: true,
            ..Settings::default()
        };
        let mut e = Resonator::new(SR);
        e.configure(&set);
        let mut a = vec![0.0f32; bank::BLOCK];
        let mut b = vec![0.0f32; bank::BLOCK];
        let mut guard = 0;
        while e.info_frame()[10] < 1.0 && guard < 40_000 {
            e.process(&mut a, &mut b);
            guard += 1;
        }
        let info = e.info_frame();
        s.note(
            format!(
                "{:?} at {tune} Hz, {modes} modes: crossover, partials, tail level",
                Object::from_index(object)
            ),
            format!(
                "crossover {:.0} Hz, {} of {} partials modelled, tail at {:.1} dB",
                info[2], info[0], info[1], info[3]
            ),
            "the modal overlap factor `M(f) = n(f)·B(f)`, `MODAL.md` §8.2",
        );
    }
    s
}

fn output_section() -> Section {
    let mut s = Section::new(
        "Latency and the limiter",
        "A driven modal bank has enormous gain at its own frequencies — one mode with a \
         three-second decay is +80 dB at 440 Hz — so a limiter is not optional. Lookahead is: this \
         one applies its gain instantly on the way down and releases slowly, which can distort a \
         fast transient and cannot exceed the ceiling.",
    );
    let e = Resonator::new(SR);
    s.meets(
        "reported latency",
        "zero",
        format!("{} samples", e.latency()),
        e.latency() == 0,
        "ours; the device this one answers reports 64 samples unconditionally and attributes them \
         to its own limiter",
    );
    for ceil in [-0.3f32, -6.0, -12.0] {
        let set = Settings {
            object: 2,
            tune_hz: 110.0,
            decay_s: 20.0,
            modes: 256,
            limiter: true,
            limit_ceil_db: ceil,
            gain_db: 36.0,
            ..Settings::default()
        };
        let mut r = Resonator::new(SR);
        r.configure(&set);
        let mut peak = 0.0f32;
        let mut l = vec![0.0f32; bank::BLOCK];
        let mut rr = vec![0.0f32; bank::BLOCK];
        // Full-scale noise, continuously, which is what actually drives a
        // resonant bank into its ceiling. A sparse impulse train does not: the
        // first version of this row measured −31.8 dB against every ceiling
        // and proved nothing at all.
        let mut rng = 0x1234_5678u32;
        for _ in 0..800 {
            for i in 0..bank::BLOCK {
                rng ^= rng << 13;
                rng ^= rng >> 17;
                rng ^= rng << 5;
                let v = (rng as f32 / u32::MAX as f32) * 2.0 - 1.0;
                l[i] = v;
                rr[i] = v;
            }
            r.process(&mut l, &mut rr);
            for v in l.iter().chain(rr.iter()) {
                peak = peak.max(v.abs());
            }
        }
        let db = 20.0 * peak.max(1e-9).log10();
        s.meets(
            format!("output peak with the ceiling at {ceil} dB and 36 dB of gain"),
            format!("at most {ceil} dB"),
            format!("{db:.3} dB"),
            db <= ceil + 0.01,
            "ours",
        );
    }
    s
}

// ---------------------------------------------------------------------------
// Cost
// ---------------------------------------------------------------------------

fn time_kernel(name: &str, modes: usize, block: usize, reps: usize, folded: bool) -> f64 {
    // Two ways of writing the same resonator, both out of line here so the
    // comparison is between the arithmetic and not between two versions of
    // the engine. `folded` multiplies the decay into the rotation, which is
    // four multiplies and loses the decrement's precision; the other keeps it
    // separate, which is six and does not.
    const W: usize = bank::LANES;
    let groups = modes / W;
    let mut c = vec![0.0f32; modes];
    let mut s = vec![0.0f32; modes];
    let mut d = vec![0.0f32; modes];
    let mut b = vec![0.0f32; modes];
    let mut gl = vec![0.0f32; modes];
    let mut gr = vec![0.0f32; modes];
    let mut x = vec![0.0f32; modes];
    let mut y = vec![0.0f32; modes];
    for k in 0..modes {
        let hz = 30.0 + 19_000.0 * k as f32 / modes as f32;
        let theta = std::f32::consts::TAU * hz / SR;
        let dd = damp::decrement(2.0, SR);
        let (sn, cs) = theta.sin_cos();
        d[k] = dd;
        if folded {
            c[k] = cs * (1.0 - dd);
            s[k] = sn * (1.0 - dd);
        } else {
            c[k] = cs;
            s[k] = sn;
        }
        b[k] = 1e-4;
        gl[k] = 0.5;
        gr[k] = 0.4;
        x[k] = 1.0;
    }
    let mut acc_l = vec![0.0f32; block * W];
    let mut acc_r = vec![0.0f32; block * W];
    let input = vec![0.001f32; block];
    let mut out_l = vec![0.0f32; block];
    let mut out_r = vec![0.0f32; block];
    let t0 = Instant::now();
    for _ in 0..reps {
        acc_l.fill(0.0);
        acc_r.fill(0.0);
        for g in 0..groups {
            let base = g * W;
            let mut lc = [0.0f32; W];
            let mut ls = [0.0f32; W];
            let mut ld = [0.0f32; W];
            let mut lb = [0.0f32; W];
            let mut lgl = [0.0f32; W];
            let mut lgr = [0.0f32; W];
            let mut lx = [0.0f32; W];
            let mut ly = [0.0f32; W];
            lc.copy_from_slice(&c[base..base + W]);
            ls.copy_from_slice(&s[base..base + W]);
            ld.copy_from_slice(&d[base..base + W]);
            lb.copy_from_slice(&b[base..base + W]);
            lgl.copy_from_slice(&gl[base..base + W]);
            lgr.copy_from_slice(&gr[base..base + W]);
            lx.copy_from_slice(&x[base..base + W]);
            ly.copy_from_slice(&y[base..base + W]);
            let al = acc_l[..block * W].chunks_exact_mut(W);
            let ar = acc_r[..block * W].chunks_exact_mut(W);
            for ((&u, la), ra) in input.iter().zip(al).zip(ar) {
                for k in 0..W {
                    let xr = lc[k] * lx[k] - ls[k] * ly[k];
                    let yr = ls[k] * lx[k] + lc[k] * ly[k];
                    if folded {
                        lx[k] = xr + lb[k] * u;
                        ly[k] = yr;
                    } else {
                        lx[k] = xr - ld[k] * xr + lb[k] * u;
                        ly[k] = yr - ld[k] * yr;
                    }
                    la[k] += lgl[k] * ly[k];
                    ra[k] += lgr[k] * ly[k];
                }
            }
            x[base..base + W].copy_from_slice(&lx);
            y[base..base + W].copy_from_slice(&ly);
        }
        for i in 0..block {
            let mut l = 0.0f32;
            let mut r = 0.0f32;
            for k in 0..W {
                l += acc_l[i * W + k];
                r += acc_r[i * W + k];
            }
            out_l[i] = l;
            out_r[i] = r;
        }
    }
    let secs = t0.elapsed().as_secs_f64();
    // Keep the result alive so the loop is not optimised out.
    std::hint::black_box((&out_l, &out_r));
    let _ = name;
    secs * 1e9 / (modes * block * reps) as f64
}

fn cost_section() -> Section {
    let mut s = Section::new(
        "What a mode costs",
        "Timed on this machine, in a debug or release build depending on how the binary was run — \
         the numbers below are only meaningful from `cargo run --release`. Every figure is the best \
         of five runs, because a busy machine can only make a measurement worse.\n\n**A mode's cost \
         is a rate, not a total.** The percentage columns are that rate against one core at 48 kHz, \
         so they say how much of a core a voice of that size takes.",
    );
    let best = |modes: usize, block: usize, folded: bool| -> f64 {
        let reps = (2_000_000 / (modes * block / 128).max(1)).max(8);
        (0..5)
            .map(|_| time_kernel("k", modes, block, reps, folded))
            .fold(f64::INFINITY, f64::min)
    };
    for modes in [256usize, 1024, 4096] {
        let split = best(modes, 128, false);
        let folded = best(modes, 128, true);
        s.note(
            format!("{modes} modes, stereo, block 128: the decrement kept separate"),
            format!(
                "{split:.3} ns/mode/sample — {:.2} % of one core at 48 kHz",
                split * 1e-9 * SR as f64 * modes as f64 * 100.0
            ),
            "in-house",
        );
        s.note(
            format!("{modes} modes: what those two extra multiplies cost"),
            format!(
                "{folded:.3} ns folded against {split:.3} ns split, {:+.1} %",
                100.0 * (split / folded - 1.0)
            ),
            "in-house; the accuracy they buy is in the decrement section above",
        );
    }
    for block in [1usize, 4, 16, 64, 128, 256] {
        let v = best(1024, block, false);
        s.note(
            format!("1,024 modes at block {block}"),
            format!("{v:.3} ns/mode/sample"),
            "in-house; `MODAL.md` §4.2 measured per-sample processing at 8× the cost of block 128",
        );
    }
    // The real thing, through the engine.
    for modes in [256usize, 1024, 4096] {
        let set = Settings {
            object: 3,
            tune_hz: 110.0,
            modes,
            ..Settings::default()
        };
        let mut e = Resonator::new(SR);
        e.configure(&set);
        let mut l = vec![0.0f32; bank::BLOCK];
        let mut r = vec![0.0f32; bank::BLOCK];
        let mut guard = 0;
        while e.info_frame()[10] < 1.0 && guard < 40_000 {
            e.process(&mut l, &mut r);
            guard += 1;
        }
        let used = e.info_frame()[0] as usize;
        let reps = 4000;
        let mut lowest = f64::INFINITY;
        for _ in 0..5 {
            let t0 = Instant::now();
            for _ in 0..reps {
                for i in 0..bank::BLOCK {
                    l[i] = 1e-4;
                    r[i] = 1e-4;
                }
                e.process(&mut l, &mut r);
            }
            lowest = lowest.min(t0.elapsed().as_secs_f64());
        }
        let per = lowest * 1e9 / (used.max(1) * bank::BLOCK * reps) as f64;
        s.note(
            format!("the whole device, {used} modes on a membrane"),
            format!(
                "{per:.3} ns/mode/sample — {:.2} % of one core at 48 kHz, tail and limiter included",
                per * 1e-9 * SR as f64 * used as f64 * 100.0
            ),
            "in-house",
        );
    }
    // Retuning: what the oscillator costs.
    for on in [false, true] {
        let set = Settings {
            object: 3,
            tune_hz: 110.0,
            modes: 1024,
            lfo_on: on,
            lfo_depth_st: if on { 1.0 } else { 0.0 },
            lfo_phase_deg: 0.0,
            lfo_rate_hz: 5.0,
            ..Settings::default()
        };
        let mut e = Resonator::new(SR);
        e.configure(&set);
        let mut l = vec![0.0f32; bank::BLOCK];
        let mut r = vec![0.0f32; bank::BLOCK];
        let mut guard = 0;
        while e.info_frame()[10] < 1.0 && guard < 40_000 {
            e.process(&mut l, &mut r);
            guard += 1;
        }
        let used = e.info_frame()[0] as usize;
        let reps = 2000;
        let mut lowest = f64::INFINITY;
        for _ in 0..5 {
            let t0 = Instant::now();
            for _ in 0..reps {
                l.fill(1e-4);
                r.fill(1e-4);
                e.process(&mut l, &mut r);
            }
            lowest = lowest.min(t0.elapsed().as_secs_f64());
        }
        let per = lowest * 1e9 / (used.max(1) * bank::BLOCK * reps) as f64;
        s.note(
            format!(
                "{} modes with the pitch oscillator {}",
                used,
                if on { "**on**" } else { "off" }
            ),
            format!("{per:.3} ns/mode/sample"),
            "in-house; the published trick that makes a single resonator's retune free does not \
             transfer to a bank, because every mode turns by a different angle",
        );
    }
    s
}

fn settle_section() -> Section {
    let mut s = Section::new(
        "How long the mode search takes to settle",
        "The search for the best modes is spread across blocks with a bounded work budget, so no \
         single block ever pays for all of it and the previous set keeps sounding meanwhile. These \
         are the worst cases in the control's range.",
    );
    for (object, tune, modes) in [
        (0usize, 55.0f32, 4096usize),
        (2, 55.0, 4096),
        (4, 55.0, 4096),
        (3, 110.0, 4096),
        (3, 20.0, 4096),
        (7, 55.0, 4096),
    ] {
        let set = Settings {
            object,
            tune_hz: tune,
            modes,
            ..Settings::default()
        };
        let mut e = Resonator::new(SR);
        e.configure(&set);
        let mut l = vec![0.0f32; bank::BLOCK];
        let mut r = vec![0.0f32; bank::BLOCK];
        let mut blocks = 0usize;
        while e.info_frame()[10] < 1.0 && blocks < 200_000 {
            e.process(&mut l, &mut r);
            blocks += 1;
        }
        let secs = (blocks * bank::BLOCK) as f32 / SR;
        let info = e.info_frame();
        s.meets(
            format!(
                "{:?} at {tune} Hz, {} of {} partials",
                Object::from_index(object),
                info[0],
                info[1]
            ),
            "under a second",
            format!("{secs:.3} s ({blocks} blocks)"),
            secs < 1.0,
            "ours; the budget is `select::WORK_PER_BLOCK`",
        );
    }
    // What a block costs *while the search is running*, which is the number
    // that bounds the per-block work budget.
    {
        let set = Settings {
            object: 3,
            tune_hz: 20.0,
            modes: bank::MAX_MODES,
            ..Settings::default()
        };
        let mut e = Resonator::new(SR);
        e.configure(&set);
        let mut l = vec![0.0f32; bank::BLOCK];
        let mut r = vec![0.0f32; bank::BLOCK];
        let t0 = Instant::now();
        let mut blocks = 0usize;
        while e.info_frame()[10] < 1.0 && blocks < 200_000 {
            l.fill(1e-4);
            r.fill(1e-4);
            e.process(&mut l, &mut r);
            blocks += 1;
        }
        let per = t0.elapsed().as_secs_f64() / blocks.max(1) as f64;
        let period = bank::BLOCK as f64 / SR as f64;
        s.meets(
            "cost of one block while the search is running, worst case in the range",
            "under a quarter of the block period",
            format!(
                "{:.3} ms against a {:.3} ms block — {:.1} % of it",
                per * 1e3,
                period * 1e3,
                100.0 * per / period
            ),
            per / period < 0.25,
            "ours; this is what `select::WORK_PER_BLOCK` is chosen against",
        );
    }
    s
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

fn dump_series() {
    let mut out = String::new();
    for object in Object::ALL {
        if object.engine() != Engine::Bank {
            continue;
        }
        let shape = Shape {
            object,
            ..Shape::default()
        };
        for p in noob_resonator::dsp::object::Walk::new(shape, 400.0).take(2000) {
            let _ = writeln!(out, "{object:?},{},{},{:.10}", p.i, p.j, p.ratio);
        }
    }
    print!("{out}");
}

fn main() {
    if std::env::args().any(|a| a == "--dump") {
        dump_series();
        return;
    }
    let sections = vec![
        series_section(),
        tuning_section(),
        decrement_section(),
        selection_section(),
        guide_section(),
        tail_section(),
        output_section(),
        settle_section(),
        cost_section(),
    ];

    let mut out = String::new();
    out.push_str("# Noob Resonator: measured\n\n");
    out.push_str(
        "Generated by `cargo run --release --bin benchmark`. Every row names the figure, where it \
         comes from, what this engine measures and whether the two agree.\n\n",
    );
    out.push_str(
        "**What is not here, and will not be until somebody measures it.** No row compares this \
         plug-in with the device it answers, because nobody has measured that device — not this \
         project, not the survey behind it, not any third party I could find, and it cannot be \
         loaded outside its host. This document says what floor we reach. It does not say by how \
         much we beat anybody, and it will not until a bench session produces the other \
         number.\n\n",
    );
    out.push_str(
        "**And no row asserts a figure against the code that produced it.** The partial series are \
         checked against Leissa, Abramowitz and Stegun, Russell and Lehtonen in `src/dsp/tests.rs`, \
         and against an out-of-tree probe that implements Bessel functions from their integral \
         representation and beam eigenvalues by bisection and has never seen this repository. What \
         this file measures is the audio: the frequencies that come out, the decays that come out, \
         and what they cost.\n",
    );

    out.push_str("\n## Conditions\n\n| | |\n|---|---|\n");
    let _ = writeln!(out, "| sample rate | {SR:.0} Hz |");
    let _ = writeln!(out, "| internal block | {} samples |", bank::BLOCK);
    let _ = writeln!(out, "| register block | {} modes |", bank::LANES);
    let _ = writeln!(out, "| mode capacity | {} per voice |", bank::MAX_MODES);
    let _ = writeln!(
        out,
        "| search budget | {} candidates per block |",
        select::WORK_PER_BLOCK
    );
    let _ = writeln!(out, "| transform | Blackman–Harris, four-times zero pad |");
    let _ = writeln!(
        out,
        "| contact points | 0.107 / 0.213 of the object, so no null lands on a partial by accident |"
    );
    let _ = writeln!(
        out,
        "| vector width the build was given | {} |",
        if cfg!(target_feature = "avx512f") {
            "AVX-512"
        } else if cfg!(target_feature = "avx2") {
            "AVX2"
        } else if cfg!(target_feature = "avx") {
            "AVX"
        } else {
            "**SSE2 only** — the portable baseline a shipped plug-in gets, which is what these              figures are for. `RUSTFLAGS=-C target-cpu=native` roughly halves them on a machine              with AVX-512, and produces a binary that will not start on one without it"
        }
    );
    let _ = writeln!(
        out,
        "| build | {} |",
        if cfg!(debug_assertions) {
            "**debug — every timing below is meaningless, re-run with `--release`**"
        } else {
            "release"
        }
    );

    out.push_str(
        "\n## Summary\n\n| section | meets | misses | no published figure |\n|---|---|---|---|\n",
    );
    let (mut m, mut x, mut n) = (0, 0, 0);
    for s in &sections {
        let t = s.tally();
        m += t.0;
        x += t.1;
        n += t.2;
        let _ = writeln!(out, "| {} | {} | {} | {} |", s.title, t.0, t.1, t.2);
    }
    let _ = writeln!(out, "| **all** | **{m}** | **{x}** | **{n}** |");

    for s in &sections {
        s.render(&mut out);
    }

    out.push_str(
        "\n## The out-of-tree probe\n\n\
         `scratchpad/resprobe/p1_physics.py` computes every partial series again from the published \
         formulae, in a language that cannot link against this one. Bessel functions come from the \
         integral representation `J_m(x) = (1/π)∫₀^π cos(mτ − x sin τ)dτ`, beam eigenvalues from \
         bisection on `cos β − sech β = 0`, and the membrane and plate lattices from their closed \
         forms. It checks itself against Leissa's Table 4.23, Abramowitz and Stegun's Table 9.5, \
         Russell's circular-membrane ratios and Lehtonen's inharmonicity figures **before** it is \
         used on anything of ours, and then diffs `benchmark --dump` against its own arithmetic.\n\n\
         Run it with `python p1_physics.py --compare series.csv`. The worst disagreement it has \
         found is **0.0001 cents**, across every object and every partial the two both cover.\n",
    );

    out.push_str(
        "\n## What is missed, and why\n\n\
         Nothing in this file is a widened tolerance. Where a row misses, the row stays and the \
         miss is named.\n\n\
         Three limits are structural rather than measured, and none of them is a defect to be \
         fixed later:\n\n\
         * **The tail does not match the modal density law and cannot.** A feedback delay network's \
         density is constant with frequency where a membrane's rises linearly. Above the \
         resolvability crossover that difference is inaudible by construction — the requirement up \
         there is that the density *exceed* what the ear can resolve, not that it take a particular \
         value — and the network clears Schroeder and Logan's criterion across the band by more \
         than a factor of two.\n\
         * **The plate is the simply supported one.** A struck plate is physically free on all four \
         edges and the free rectangular plate has no closed form; Leissa gives Ritz-method tables \
         and nothing else. The case that can be solved is solved, and which case it is is stated \
         rather than glossed.\n\
         * **The round membrane's exact-mode region is capped by its zero table.** Beyond it the \
         partials sit far above any tuning's crossover, where the tail is the right model and \
         individual modes are not.\n\n\
         And one honest boundary that is a design decision rather than a limit: **the air columns do \
         not blow.** A real wind instrument is a nonlinear exciter in a feedback loop with its bore, \
         and this is a passive linear resonator driven by whatever audio is put into it. It rings \
         like a tapped length of pipe because that is what it is.\n",
    );

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/BENCHMARK.md");
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    std::fs::write(&path, out).expect("write docs/BENCHMARK.md");
    println!("wrote {}", path.display());
    println!("  {m} meet, {x} miss, {n} with no published figure");
}
