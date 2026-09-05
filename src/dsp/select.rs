//! Which modes to keep, which is the decision this whole plug-in is about.
//!
//! **"More modes" is the wrong axis and the affordability question is
//! closed.** Measured here at the portable instruction baseline, a mode costs
//! **about a third of a nanosecond per sample in stereo** and four thousand of
//! them are **about six per cent of one core** at 48 kHz; `docs/BENCHMARK.md`
//! carries the figures to three places and is regenerated rather than
//! transcribed, because a busy machine moves them by a few per cent. Five of
//! the eight objects here have their
//! *entire* physical mode set inside that budget several times over: a bar
//! tuned to 55 Hz has twenty-eight partials in the whole audible band, a
//! string 363, a plate 545. You can simply afford them.
//!
//! **The scarce resource is *which* modes.** `MODAL.md` §8.1 measured the
//! energy recovered in the 4–10 kHz band at a fixed budget of 512 on a 110 Hz
//! rectangular membrane, against the full 54,749-mode bank:
//!
//! | which 512 | highest kept | 1–4 kHz | 4–10 kHz | 10–20 kHz |
//! |---|---|---|---|---|
//! | the lowest by frequency | 1,980 Hz | −4.9 dB | **−82.5 dB** | −100.8 dB |
//! | **the loudest by contribution** | 14,626 Hz | −6.1 dB | **−17.1 dB** | −30.8 dB |
//! | evenly spread in log frequency | 19,999 Hz | −17.5 dB | −38.5 dB | −48.7 dB |
//!
//! **Sixty-five decibels, at identical cost, purely from which modes you
//! keep** — that document's figure, on its own probe.
//!
//! `docs/BENCHMARK.md` runs the experiment again on **this** engine, against a
//! reference bank holding every partial the object has, and reports the worst
//! band error rather than one band's: at a 64-mode budget the best ordering is
//! **68.4 dB** closer to the full bank than the worst, and at 256 it is
//! **66.7 dB**. All three orderings are on the `Selection` control, so the
//! difference can be heard as well as read.
//!
//! ## Where "loudest" stops being an improvement, which is worth saying
//!
//! At **exactly flat** — `Bright` at 0 dB/octave — the criterion degenerates,
//! and it does so on the very object it matters most for. A mass-normalised
//! mode set has no frequency trend in its amplitudes, so every candidate's
//! contribution is bounded by the same number; a membrane has far more
//! candidates per octave at the top of the band than at the bottom; and so
//! far more of them come close to that bound up there. "Keep the loudest"
//! then means "keep the highest", which is the mirror of "keep the lowest"
//! and no better.
//!
//! Measured here on a 110 Hz membrane at a 512-mode budget, the highest
//! partial kept and where the budget went:
//!
//! | Bright | highest kept | in 1.5–10 kHz | above 10 kHz |
//! |---|---|---|---|
//! | 0 dB/oct | 20.0 kHz | **0** | 352 |
//! | −3 dB/oct | 7.9 kHz | **341** | 0 |
//! | −6 dB/oct | 6.1 kHz | 316 | 0 |
//!
//! Two things follow and both are in the build. The search stops at the top
//! of hearing rather than at Nyquist, because a slot spent above 20 kHz is a
//! slot an audible partial wanted. And **the tilt defaults to −3 dB/octave**
//! rather than to flat, which is ours: no real contact is an impulse, a
//! mallet with a finite contact time is a lowpass on the excitation, and the
//! engine vendor whose calibration this control borrows puts −6 dB/octave at
//! "the amplitude of the partials being inversely proportional to their
//! frequency". Flat is available and it is not the default.
//!
//! ## What "loudest" means
//!
//! A mode's contribution is its shape at the strike times its shape at the
//! pickups times the spectral tilt: `a_k = tilt(f_k)·ψ_k(x_e)·rms(ψ_k(x_L),
//! ψ_k(x_R))`. That is the amplitude it would actually reach in the output,
//! so ordering by it orders by what a listener would lose.
//!
//! Every mode **below the resolvability crossover** is kept first whatever
//! its amplitude, because down there the partials stand apart and a missing
//! one is a missing pitch rather than a missing texture. The remaining budget
//! then goes to the loudest of the rest. The other two orderings are left
//! pure, because their job is to show what the choice costs.
//!
//! ## Why the search is spread over blocks
//!
//! A rectangular membrane tuned to 20 Hz has about 1.7 million partials below
//! Nyquist, and the loudest four thousand of them cannot be found without
//! looking at all of them. Doing that inside one audio block would be a
//! millisecond-long spike; doing it on another thread would need a lock or an
//! allocation.
//!
//! So the search is **incremental**: a bounded amount of work per block, into
//! a shadow set, with the previous set still sounding. It settles in tens of
//! milliseconds for anything ordinary and in a fraction of a second for the
//! pathological corner, and until it does, the object keeps ringing as it
//! was. `docs/BENCHMARK.md` prints the settle time per object.

use crate::dsp::damp::{self, Damping};
use crate::dsp::object::{Contacts, Object, Point, Shape, Walk};
use crate::dsp::tail::{BANDS, band_of};

/// The three orderings.
pub const SELECT_NAMES: [&str; 3] = ["Loudest", "Lowest", "Log Spread"];

/// Candidate partials examined per block, in units of the cheapest object's
/// cost.
///
/// It bounds the rebuild's contribution to one block, and it is set from a
/// measurement rather than chosen: `docs/BENCHMARK.md` times a block while
/// the search is running on the heaviest case in the range — a rectangular
/// membrane at the bottom of the Tune control, 1.6 million candidates — and
/// the budget is the largest that keeps that block inside a quarter of its
/// own period. At this rate that case settles in about half a second and
/// everything else in a few tens of milliseconds. The old set sounds
/// throughout.
pub const WORK_PER_BLOCK: usize = 8192;

/// What the selector was asked to find.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Request {
    pub shape: Shape,
    pub exc: Point,
    pub left: Point,
    pub right: Point,
    /// The fundamental the ratios are quoted against.
    pub f0: f32,
    /// The highest partial worth keeping, hertz.
    pub f_max: f32,
    pub budget: usize,
    /// Index into [`SELECT_NAMES`].
    pub order: usize,
    pub tilt_db_oct: f32,
    pub damping: Damping,
    pub crossover_hz: f32,
    pub sr: f32,
}

impl Default for Request {
    fn default() -> Self {
        Request {
            shape: Shape::default(),
            exc: Point::new(0.2, 0.2),
            left: Point::new(0.3, 0.3),
            right: Point::new(0.7, 0.7),
            f0: 220.0,
            f_max: 20_000.0,
            budget: 1024,
            order: 0,
            tilt_db_oct: 0.0,
            damping: Damping::default(),
            crossover_hz: 20_000.0,
            sr: 48_000.0,
        }
    }
}

/// One kept partial.
///
/// The **ratio** is stored rather than the frequency, so that retuning — a
/// knob, a transposition, an oscillator on the pitch — rescales the whole set
/// without asking for the search again.
#[derive(Clone, Copy, Debug, Default)]
pub struct Chosen {
    pub ratio: f32,
    /// The strike's mode shape, times the spectral tilt.
    pub amp_in: f32,
    pub amp_l: f32,
    pub amp_r: f32,
    pub i: u16,
    pub j: u16,
}

/// A bounded search for the best `budget` of an unbounded stream of
/// candidates, run a piece at a time.
pub struct Selector {
    req: Request,
    walk: Option<Walk>,
    contacts: Option<Contacts>,
    /// The min-heap: `keys[0]` is the worst kept candidate, so a new one only
    /// has to beat that to get in.
    keys: Vec<f32>,
    items: Vec<Chosen>,
    heap_len: usize,
    /// Log-spread bins: the best candidate in each, and how close to the
    /// bin's centre it landed.
    bin_item: Vec<Chosen>,
    bin_dist: Vec<f32>,
    bin_used: Vec<bool>,
    /// `Σ a²·B` over every candidate walked past, per band, kept or not.
    seen: [f32; BANDS],
    /// The finished set, ascending in ratio.
    out: Vec<Chosen>,
    out_len: usize,
    /// What the finished set left behind, per band, in the same units.
    residual: [f32; BANDS],
    /// How many partials the object has below the ceiling.
    available: usize,
    walked: usize,
    generation: u64,
    settled: bool,
}

impl Selector {
    pub fn new(max_modes: usize) -> Selector {
        Selector {
            req: Request::default(),
            walk: None,
            contacts: None,
            keys: vec![0.0; max_modes],
            items: vec![Chosen::default(); max_modes],
            heap_len: 0,
            bin_item: vec![Chosen::default(); max_modes],
            bin_dist: vec![0.0; max_modes],
            bin_used: vec![false; max_modes],
            seen: [0.0; BANDS],
            out: vec![Chosen::default(); max_modes],
            out_len: 0,
            residual: [0.0; BANDS],
            available: 0,
            walked: 0,
            generation: 0,
            settled: false,
        }
    }

    /// The finished set, ascending in ratio. Empty until the first search
    /// completes.
    pub fn result(&self) -> &[Chosen] {
        &self.out[..self.out_len]
    }

    /// What the finished set left out, per band, as `Σ a²·B`.
    pub fn residual(&self) -> &[f32; BANDS] {
        &self.residual
    }

    /// How many partials the object has below the ceiling, whether or not
    /// they were kept.
    pub fn available(&self) -> usize {
        self.available
    }

    /// Bumped every time a search completes, so the engine can tell a new set
    /// from the one it already loaded.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Whether a search is in progress.
    pub fn searching(&self) -> bool {
        self.walk.is_some()
    }

    /// How far a search has got, 0…1. One when nothing is pending.
    pub fn progress(&self) -> f32 {
        if !self.settled {
            return 0.0;
        }
        if self.walk.is_none() {
            return 1.0;
        }
        if self.available == 0 {
            return 0.0;
        }
        (self.walked as f32 / self.available as f32).clamp(0.0, 1.0)
    }

    /// The request the current result was found for.
    pub fn request(&self) -> &Request {
        &self.req
    }

    /// Whether a request would need a new search. Retuning does not: the
    /// ratios do not move, only the ceiling they are measured against, and
    /// the ceiling is checked separately.
    pub fn needs_rebuild(&self, req: &Request) -> bool {
        if !self.settled {
            return true;
        }
        let a = &self.req;
        a.shape != req.shape
            || a.exc != req.exc
            || a.left != req.left
            || a.right != req.right
            || a.budget != req.budget
            || a.order != req.order
            || a.tilt_db_oct != req.tilt_db_oct
            || a.sr != req.sr
            // The set of partials under Nyquist is a function of f_max/f0, and
            // the crossover decides which of them are kept unconditionally.
            || (a.f_max / a.f0 - req.f_max / req.f0).abs() > 1e-3 * (a.f_max / a.f0)
            || (a.crossover_hz - req.crossover_hz).abs() > 0.02 * a.crossover_hz.max(1.0)
    }

    /// Begin a search. Any search in progress is abandoned; the finished set
    /// keeps sounding until this one completes.
    pub fn start(&mut self, req: Request) {
        self.req = req;
        let max_ratio = (req.f_max / req.f0.max(1e-3)) as f64;
        self.walk = Some(Walk::new(req.shape, max_ratio));
        self.contacts = Some(Contacts::new(req.shape, req.exc, req.left, req.right));
        self.heap_len = 0;
        self.seen = [0.0; BANDS];
        self.available = req.shape.available(max_ratio);
        self.walked = 0;
        let bins = req.budget.min(self.bin_used.len());
        self.bin_used[..bins].fill(false);
    }

    /// Do a bounded amount of the search. Returns true if it finished on this
    /// call.
    pub fn step(&mut self, work: usize) -> bool {
        let Some(mut walk) = self.walk.take() else {
            return false;
        };
        let Some(contacts) = self.contacts.take() else {
            return false;
        };
        let req = self.req;
        let cost = req.shape.object.candidate_cost().max(1);
        let allow = (work / cost).max(1);
        let budget = req.budget.min(self.items.len()).max(1);
        let bins = budget;
        let log_span = (req.f_max / req.f0.max(1e-3)).max(1.000_1).log2();

        let mut done = true;
        for _ in 0..allow {
            let Some(p) = walk.next() else {
                done = true;
                break;
            };
            done = false;
            self.walked += 1;
            let hz = req.f0 * p.ratio;
            if hz <= 0.0 || hz >= req.f_max {
                continue;
            }
            let (pe, pl, pr) = contacts.psi(p.i, p.j);
            let tilt = crate::dsp::guide::tilt(hz, req.f0, req.tilt_db_oct);
            let amp_in = pe * tilt;
            let amp = amp_in.abs() * (0.5 * (pl * pl + pr * pr)).sqrt();

            // What this partial would contribute to a broadband output: its
            // squared peak times the bandwidth it passes.
            let b = damp::bandwidth_hz(req.damping.t60_at(hz));
            let band = band_of(hz, req.sr);
            self.seen[band] += amp * amp * b.min(req.sr * 0.5);

            let chosen = Chosen {
                ratio: p.ratio,
                amp_in,
                amp_l: pl,
                amp_r: pr,
                i: p.i,
                j: p.j,
            };
            match req.order {
                // Loudest, with everything below the crossover kept first.
                0 => {
                    let key = if hz <= req.crossover_hz {
                        1e18 + amp
                    } else {
                        amp
                    };
                    self.heap_push(key, chosen, budget);
                }
                // Lowest by frequency, kept pure so that what it costs is
                // visible.
                1 => self.heap_push(-hz, chosen, budget),
                // One per logarithmic bin, whichever landed nearest its
                // centre.
                _ => {
                    let t = (hz / req.f0.max(1e-3)).max(1e-6).log2() / log_span;
                    let idx = ((t * bins as f32) as usize).min(bins - 1);
                    let centre = (idx as f32 + 0.5) / bins as f32;
                    let dist = (t - centre).abs();
                    if !self.bin_used[idx] || dist < self.bin_dist[idx] {
                        self.bin_used[idx] = true;
                        self.bin_dist[idx] = dist;
                        self.bin_item[idx] = chosen;
                    }
                }
            }
        }
        if !done {
            self.walk = Some(walk);
            self.contacts = Some(contacts);
            return false;
        }
        self.finish(budget, bins);
        true
    }

    fn heap_push(&mut self, key: f32, item: Chosen, cap: usize) {
        if self.heap_len < cap {
            let i = self.heap_len;
            self.keys[i] = key;
            self.items[i] = item;
            self.heap_len += 1;
            self.sift_up(i);
        } else if cap > 0 && key > self.keys[0] {
            self.keys[0] = key;
            self.items[0] = item;
            self.sift_down(0, cap);
        }
    }

    fn sift_up(&mut self, mut i: usize) {
        while i > 0 {
            let p = (i - 1) / 2;
            if self.keys[p] <= self.keys[i] {
                break;
            }
            self.keys.swap(p, i);
            self.items.swap(p, i);
            i = p;
        }
    }

    fn sift_down(&mut self, mut i: usize, len: usize) {
        loop {
            let (l, r) = (2 * i + 1, 2 * i + 2);
            let mut m = i;
            if l < len && self.keys[l] < self.keys[m] {
                m = l;
            }
            if r < len && self.keys[r] < self.keys[m] {
                m = r;
            }
            if m == i {
                return;
            }
            self.keys.swap(m, i);
            self.items.swap(m, i);
            i = m;
        }
    }

    /// Collect the kept set, order it by frequency, and work out what was
    /// left behind.
    fn finish(&mut self, budget: usize, bins: usize) {
        let req = self.req;
        let n = if req.order >= 2 {
            let mut n = 0usize;
            for k in 0..bins {
                if self.bin_used[k] {
                    self.out[n] = self.bin_item[k];
                    n += 1;
                }
            }
            n
        } else {
            let n = self.heap_len.min(budget);
            self.out[..n].copy_from_slice(&self.items[..n]);
            n
        };
        self.out[..n].sort_unstable_by(|a, b| {
            a.ratio
                .partial_cmp(&b.ratio)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        self.out_len = n;

        // The residual is what the walk saw minus what was kept, band by
        // band, so it is a sum over the partials that were actually left out
        // rather than an estimate of them.
        self.residual = self.seen;
        for c in &self.out[..n] {
            let hz = req.f0 * c.ratio;
            let amp = c.amp_in.abs() * (0.5 * (c.amp_l * c.amp_l + c.amp_r * c.amp_r)).sqrt();
            let b = damp::bandwidth_hz(req.damping.t60_at(hz)).min(req.sr * 0.5);
            let band = band_of(hz, req.sr);
            self.residual[band] -= amp * amp * b;
        }
        for r in self.residual.iter_mut() {
            *r = r.max(0.0);
        }
        self.walk = None;
        self.contacts = None;
        self.settled = true;
        self.generation = self.generation.wrapping_add(1);
    }

    /// A search that will never be started because the object is an air
    /// column: the waveguide has no mode list to truncate.
    pub fn clear(&mut self) {
        self.walk = None;
        self.contacts = None;
        self.out_len = 0;
        self.residual = [0.0; BANDS];
        self.available = 0;
        self.settled = false;
    }
}

/// The modal density of an object at a frequency, in modes per hertz.
///
/// It follows from the count law and nothing else: `N(f) ∝ f^p`, so
/// `n(f) = dN/df = p·N(f)/f`. Which is why a bar's density falls with
/// frequency, a plate's is flat and a membrane's rises — and why the same
/// mode budget means completely different things on the three.
pub fn density_at(shape: &Shape, f0: f32, hz: f32) -> f32 {
    let f = hz.max(f0.max(1e-3));
    let n = shape.available((f / f0.max(1e-3)) as f64) as f32;
    let p = shape.object.density_exponent();
    if n <= 0.0 { 0.0 } else { p * n / f }
}

/// Whether an object's partials are worth searching at all.
pub fn is_bank(object: Object) -> bool {
    object.engine() == crate::dsp::object::Engine::Bank
}
