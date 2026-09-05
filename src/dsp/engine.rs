//! The whole device: an exciter, one of two resonator architectures, the
//! statistical tail, and an output stage that reports zero latency and means
//! it.
//!
//! ## What this device is
//!
//! **A resonator, not a synthesiser.** The incoming audio supplies the
//! strike; this supplies the body that rings. A pluck is a broadband click
//! and the box decides what survives and for how long. The exciter colours
//! the attack; the resonator owns the pitch and the decay.
//!
//! ## The order of the chain, and why Dry/Wet is where it is
//!
//! ```text
//!   in ─┬─────────────────────────────────────────── dry ────────────┐
//!       │                                                            │
//!       └─ × Dry/Wet ─ [band-pass] ─┬─ mode bank ──┬─ width ─ gain ─ limiter ─┴─ out
//!                                   └─ tail ───────┘
//!                                   or waveguide
//! ```
//!
//! **Dry/Wet is a gate on the input to the wet path, not a crossfade at the
//! output.** Turning it down stops new signal from being processed and leaves
//! whatever is already ringing to ring out. That is a design the device this
//! one answers gets right, and it is copied deliberately: a modal bank whose
//! tail is chopped by a fader is a modal bank that clicks.
//!
//! **The exciter is the mono sum of the input.** One object is struck at one
//! point, so it has one input; the stereo comes from the two pickup
//! positions, from the detune between the pair, and from Width. That is a
//! consequence of the architecture rather than a shortcut, and it is stated
//! here so nobody has to discover it.
//!
//! ## Where the cost goes
//!
//! One bank with two pickup taps is the cheap case and the default. Two
//! things force a second bank, and both are honest doublings rather than
//! oversights: **Spread**, which detunes the two channels' objects against
//! each other, and an **oscillator with a stereo phase offset**, which does
//! the same thing continuously. `modes_used` on the readout stream is the
//! total, so the panel can show what a setting is actually costing.

use crate::dsp::bank::{Bank, ModeInfo};
use crate::dsp::damp::{self, Damping};
use crate::dsp::filters::{Limiter, Svf, q_from_octaves};
use crate::dsp::guide::{self, Guide};
use crate::dsp::lfo::Lfo;
use crate::dsp::object::{CHORD_VOICES, Engine as ObjEngine, Object, Point, Shape};
use crate::dsp::select::{self, Request, Selector};
use crate::dsp::tail::{BANDS, Tail};

/// Modes the per-mode table can address, and partials published on the modes
/// stream.
pub const MAX_EDITS: usize = 64;
/// Values in one `modes` frame.
pub const MODE_FIELDS: usize = 8;
/// Values in one `info` frame: thirteen readouts, then one available count
/// per voice.
///
/// **The per-voice counts are appended rather than given their own stream**,
/// because they are read at the same moment and by the same code as the
/// thirteen, and a second stream would be a second arrival time to reconcile
/// — which is the fault that made a ratio-1 partial draw at 1.2. Appending
/// also leaves every existing index where it was.
pub const INFO_LEN: usize = 13 + 2 * CHORD_VOICES;
/// Points on the response curve.
pub const RESPONSE_POINTS: usize = 512;
/// Points of the response curve refreshed per block, so a redraw never lands
/// in one block.
const RESPONSE_CHUNK: usize = 128;

/// Blocks between readout refreshes while only the pitch is moving.
const READOUT_BLOCKS: u32 = 8;

/// Marks a mode slot that no override addresses.
const NO_EDIT: u8 = u8::MAX;

/// How many blocks running the mode search may be abandoned before one is
/// allowed to finish.
///
/// **Abandoning is the right default and starving is the failure it can
/// become.** A search whose settings have changed is answering a question
/// nobody is asking any more, so it should be dropped — that is the fix for
/// the wedge, where a change arriving mid-search was ignored and, with a
/// search that could not end, ignored for the rest of the session. But
/// dropped *every* block is its own dead end: under host automation on Tune
/// the settings move continuously, no search would ever complete, and the
/// bank would freeze on whatever it last held while appearing to follow.
///
/// So a search that has been restarted this many times in a row without
/// finishing is left alone until it does. At thirty-two blocks that is 85 ms
/// of promptly abandoning, which covers any gesture, followed by one search
/// run to completion, so the bank keeps moving while a knob is swept instead
/// of waiting for the sweep to stop.
const RESTART_LIMIT: u32 = 32;

/// How far Spread pulls the two channels apart at full travel, in cents.
/// A quarter tone, which is the same span the Fine control has.
pub const SPREAD_MAX_CENTS: f32 = 50.0;

/// The highest partial worth a slot in the bank.
///
/// Nyquist is not the right ceiling. A partial at 22 kHz is inaudible, and at
/// a fixed budget every slot it takes is one an audible partial wanted — on a
/// membrane at 48 kHz, where the modal density rises with frequency, the
/// region between 20 kHz and Nyquist holds more candidates than the whole
/// audible band below 4 kHz. So the search stops at the top of hearing and
/// the statistical tail covers whatever is above it.
pub const AUDIBLE_MAX_HZ: f32 = 20_000.0;

/// The largest stiff-string inharmonicity coefficient the Inharm control
/// reaches. Ten times the value Lehtonen and colleagues measured for a piano
/// C4, so the physical region sits in the first third of the travel and the
/// rest is available for the synthetic extension.
pub const INHARM_B_MAX: f32 = 3.0e-3;

/// One partial's override.
///
/// **Keyed by the mode's own identity, `(i, j)`, and not by where it happens
/// to sit in the published frame.** A user who retunes a partial has edited
/// *that resonance*; if the edit followed a frame position, changing
/// `Selection` or the mode budget would reorder the frame underneath and
/// silently reassign every override to a different partial — and the result
/// would look entirely reasonable, which is the failure this project keeps
/// catching late. Resolving identity to a row is the display's job and it
/// happens at draw time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModeEdit {
    /// The partial's first index: `n` on a line, `m` on a surface, the
    /// resonance number in an air column. [`ModeEdit::NONE`] marks an empty
    /// slot.
    pub i: u16,
    /// The second index; zero for anything one-dimensional.
    pub j: u16,
    pub cents: f32,
    pub db: f32,
    /// A multiplier on the partial's T60.
    pub decay: f32,
}

impl Default for ModeEdit {
    fn default() -> Self {
        ModeEdit {
            i: ModeEdit::NONE,
            j: 0,
            cents: 0.0,
            db: 0.0,
            decay: 1.0,
        }
    }
}

impl ModeEdit {
    /// An `i` no partial has, marking an unused slot.
    pub const NONE: u16 = u16::MAX;

    /// Whether the slot addresses a partial at all.
    pub fn is_set(&self) -> bool {
        self.i != ModeEdit::NONE
    }

    /// Whether it addresses one and then does nothing to it.
    pub fn is_neutral(&self) -> bool {
        self.cents == 0.0 && self.db == 0.0 && self.decay == 1.0
    }

    /// Whether it applies to the partial with these indices.
    pub fn matches(&self, i: u16, j: u16) -> bool {
        self.is_set() && self.i == i && self.j == j
    }
}

/// One block's worth of controls.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    pub object: usize,
    pub tune_hz: f32,
    pub transpose: f32,
    pub fine_cents: f32,
    pub modes: usize,
    pub order: usize,
    pub aspect: f32,
    pub bar_tuning: usize,
    pub bar_third: usize,
    /// How many of the chord's voices are sounding.
    pub voices: usize,
    /// Whether held MIDI notes set the voice pitches.
    pub midi_voices: bool,
    /// How far the bore is from an ideal cylinder, 0 … 1. Air columns only.
    pub disperse: f32,
    /// Each voice's pitch in semitones from the root, in the user's order.
    pub voice_semis: [f32; CHORD_VOICES],
    pub radius_mm: f32,
    pub opening: f32,
    pub decay_s: f32,
    pub material: f32,
    pub damp_corner_hz: f32,
    pub damp_hi: f32,
    pub tail: bool,
    pub bright_db_oct: f32,
    /// −1 … +1. Negative compresses the partials, which no string does.
    pub inharm: f32,
    pub hit: Point,
    pub pos_l: Point,
    pub pos_r: Point,
    pub spread: f32,
    pub width: f32,
    pub filter_on: bool,
    pub filter_hz: f32,
    pub filter_oct: f32,
    /// False puts the band-pass before the resonator, which is where it
    /// belongs and where theirs is; true puts it after, which is what their
    /// manual describes and their module order contradicts.
    pub filter_post: bool,
    pub lfo_on: bool,
    pub lfo_shape: usize,
    pub lfo_rate_hz: f32,
    pub lfo_depth_st: f32,
    pub lfo_phase_deg: f32,
    pub bleed: f32,
    pub mix: f32,
    pub gain_db: f32,
    pub limiter: bool,
    pub limit_ceil_db: f32,
    pub bypass: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            object: 2,
            tune_hz: 220.0,
            transpose: 0.0,
            fine_cents: 0.0,
            modes: 1024,
            order: 0,
            aspect: 1.0,
            bar_tuning: 0,
            bar_third: 0,
            voices: 1,
            midi_voices: false,
            disperse: 0.0,
            voice_semis: [0.0, 7.0, 16.0, 12.0, 19.0, 24.0],
            radius_mm: guide::RADIUS_REF_MM,
            opening: 0.0,
            decay_s: 2.0,
            material: -0.5,
            damp_corner_hz: 20_000.0,
            damp_hi: -1.0,
            tail: true,
            bright_db_oct: -3.0,
            inharm: 0.0,
            hit: Point::new(0.2, 0.2),
            pos_l: Point::new(0.3, 0.3),
            pos_r: Point::new(0.7, 0.7),
            spread: 0.0,
            width: 1.0,
            filter_on: false,
            filter_hz: 1000.0,
            filter_oct: 4.0,
            filter_post: false,
            lfo_on: false,
            lfo_shape: 0,
            lfo_rate_hz: 1.0,
            lfo_depth_st: 0.0,
            lfo_phase_deg: 180.0,
            bleed: 0.0,
            mix: 1.0,
            gain_db: 0.0,
            limiter: true,
            limit_ceil_db: -0.3,
            bypass: false,
        }
    }
}

impl Settings {
    pub fn object(&self) -> Object {
        Object::from_index(self.object)
    }

    /// The sounding fundamental before the oscillator and the detune, in
    /// hertz.
    pub fn base_hz(&self) -> f32 {
        let semis = self.transpose + self.fine_cents / 100.0;
        (self.tune_hz * 2f32.powf(semis / 12.0)).clamp(1.0, 20_000.0)
    }

    /// The stiff-string coefficient the Inharm control is asking for, signed.
    ///
    /// Quadratic in the control so that the region a real string lives in —
    /// `B` around `3 × 10⁻⁴` for a piano C4 — sits where a knob can be placed
    /// on it, rather than in the first pixel.
    pub fn inharm_b(&self) -> f32 {
        let x = self.inharm.clamp(-1.0, 1.0);
        INHARM_B_MAX * x * x * x.signum()
    }

    /// Whether this object can carry voices at all.
    pub fn object_can_voice(&self) -> bool {
        self.object().can_voice()
    }

    pub fn shape(&self) -> Shape {
        Shape {
            object: self.object(),
            voices: self.voices.clamp(1, CHORD_VOICES) as u8,
            voice_semis: self.voice_semis,
            aspect: self.aspect,
            inharm_b: self.inharm_b(),
            bar_tuning: self.bar_tuning,
            bar_third: self.bar_third,
        }
    }

    /// The damping law the mode bank runs on.
    pub fn damping(&self) -> Damping {
        Damping {
            f0: self.base_hz(),
            t60: self.decay_s.max(1e-3),
            exponent: self.material.clamp(-1.0, 1.0),
            corner_hz: self.damp_corner_hz,
            exponent_hi: self.damp_hi,
        }
    }
}

/// The device.
pub struct Resonator {
    sr: f32,
    set: Settings,
    banks: [Bank; 2],
    /// One air column per side per voice.
    ///
    /// A waveguide is a loop rather than a bank, so a second pitch is a
    /// second loop: there is no set of modes to add to. Six of them is still
    /// far cheaper than a mode bank of the same reach — a loop costs the same
    /// whatever number of harmonics comes out of it — which is exactly the
    /// property that makes voices affordable here at all.
    guides: [[Guide; CHORD_VOICES]; 2],
    tail: Tail,
    sel: Selector,
    lfo: Lfo,
    pre: Svf,
    post: [Svf; 2],
    limiter: Limiter,
    edits: [ModeEdit; MAX_EDITS],
    edits_dirty: bool,
    /// Where each live partial sat before inharmonicity moved it, so the
    /// panel need not invert a stretch it cannot see. Sized once.
    base_ratios: Vec<f32>,
    /// Which edit slot, if any, owns each mode slot.
    ///
    /// The overrides are keyed by identity, so applying them means asking
    /// "which edit is for *this* partial" once per mode. Doing that as a scan
    /// inside the retune loop would be sixty-four comparisons per mode per
    /// block while the oscillator runs; resolving it once when either side
    /// changes makes it one byte of indirection.
    edit_of: Vec<u8>,

    // Scratch, sized at construction so nothing allocates in `process`.
    exc: Vec<f32>,
    wl: Vec<f32>,
    wr: Vec<f32>,
    bl: Vec<f32>,
    br: Vec<f32>,
    /// A third scratch pair, needed once the air columns sum several voices:
    /// the accumulator and the voice being added cannot be the same buffer.
    gr: Vec<f32>,
    dry_l: Vec<f32>,
    dry_r: Vec<f32>,

    /// The selection generation the banks were loaded from.
    loaded: u64,
    /// Whether a second bank is running.
    stereo: bool,
    /// The fundamentals the two sides are currently tuned to.
    f0: [f32; 2],
    /// The scale that gives the bank unit power gain.
    scale: f32,
    /// The band-by-band residual the tail was configured with.
    residual: [f32; BANDS],
    crossover: f32,
    /// What the cached crossover was computed for. Working it out means
    /// counting an object's partials at ninety-six frequencies, which for a
    /// membrane is half a million operations — several times what the whole
    /// bank costs for the same block — so it is done when something it
    /// depends on moves and not once a block.
    cross_key: Option<(Shape, Damping, f32)>,
    /// The damping law the coefficients were last built with, so that a
    /// retune under the oscillator does not rebuild what has not changed.
    applied: Option<Damping>,
    /// How many times running the mode search has been abandoned and started
    /// again without ever finishing. See [`RESTART_LIMIT`].
    restarts: u32,
    /// The fundamental the published mode table was built at, which is what
    /// `f0_hz` reports — **not** the current one. See [`Self::info_frame`].
    readout_f0: f32,
    /// Which voices MIDI is holding, for the readout only: the engine does
    /// not decide this and does not act on it.
    held: [bool; CHORD_VOICES],

    meter: [f32; 4],
    modes_frame: Vec<f32>,
    response: Vec<f32>,
    response_cursor: usize,
    curves_dirty: bool,
    /// Blocks until the readouts are next refreshed while only the pitch is
    /// moving. Redrawing a 512-point response over four thousand modes on
    /// every block of an oscillator's travel costs several times what the
    /// audio does, and no display needs it: at 48 kHz and a 128-sample block
    /// this is still forty-seven refreshes a second.
    readout_wait: u32,
    limit_db: f32,
}

impl Resonator {
    pub fn new(sr: f32) -> Resonator {
        let max = crate::dsp::bank::MAX_MODES;
        let mut r = Resonator {
            sr,
            set: Settings::default(),
            banks: [Bank::new(sr), Bank::new(sr)],
            guides: std::array::from_fn(|_| std::array::from_fn(|_| Guide::new(sr))),
            tail: Tail::new(sr),
            sel: Selector::new(max),
            lfo: Lfo::default(),
            pre: Svf::default(),
            post: [Svf::default(); 2],
            limiter: Limiter::default(),
            edits: [ModeEdit::default(); MAX_EDITS],
            edits_dirty: false,
            edit_of: vec![NO_EDIT; max],
            base_ratios: vec![1.0; max],
            exc: vec![0.0; crate::dsp::bank::BLOCK],
            wl: vec![0.0; crate::dsp::bank::BLOCK],
            wr: vec![0.0; crate::dsp::bank::BLOCK],
            bl: vec![0.0; crate::dsp::bank::BLOCK],
            br: vec![0.0; crate::dsp::bank::BLOCK],
            gr: vec![0.0; crate::dsp::bank::BLOCK],
            dry_l: vec![0.0; crate::dsp::bank::BLOCK],
            dry_r: vec![0.0; crate::dsp::bank::BLOCK],
            loaded: 0,
            stereo: false,
            f0: [220.0; 2],
            scale: 1.0,
            residual: [0.0; BANDS],
            crossover: 20_000.0,
            cross_key: None,
            applied: None,
            restarts: 0,
            readout_f0: 220.0,
            held: [false; CHORD_VOICES],
            meter: [0.0; 4],
            modes_frame: vec![0.0; MAX_EDITS * MODE_FIELDS],
            response: vec![-120.0; RESPONSE_POINTS],
            response_cursor: 0,
            curves_dirty: true,
            readout_wait: 0,
            limit_db: 0.0,
        };
        let s = r.set;
        r.configure(&s);
        r
    }

    pub fn sample_rate(&self) -> f32 {
        self.sr
    }

    pub fn set_sample_rate(&mut self, sr: f32) {
        self.sr = sr;
        for b in self.banks.iter_mut() {
            b.set_sample_rate(sr);
        }
        for g in self.guides.iter_mut().flatten() {
            g.set_sample_rate(sr);
        }
        self.tail.set_sample_rate(sr);
        self.sel.clear();
        self.loaded = 0;
        let s = self.set;
        self.configure(&s);
    }

    pub fn reset(&mut self) {
        for b in self.banks.iter_mut() {
            b.reset();
        }
        for g in self.guides.iter_mut().flatten() {
            g.reset();
        }
        self.tail.reset();
        self.pre.reset();
        for f in self.post.iter_mut() {
            f.reset();
        }
        self.limiter.reset();
        self.lfo.reset();
        self.meter = [0.0; 4];
    }

    /// This plug-in delays nothing and says so.
    pub fn latency(&self) -> usize {
        0
    }

    pub fn settings(&self) -> &Settings {
        &self.set
    }

    /// The mode bank, for the tests and the measurement binary. Its `info`
    /// is every live partial rather than the 64 the stream publishes.
    pub fn bank(&self) -> &Bank {
        &self.banks[0]
    }

    /// The air column, likewise: the root voice's loop.
    pub fn guide(&self) -> &Guide {
        &self.guides[0][0]
    }

    /// Record which voices MIDI is holding. Readout only.
    pub fn set_held(&mut self, v: crate::dsp::Voicing) {
        for k in 0..CHORD_VOICES {
            self.held[k] = v.is_held(k);
        }
    }

    /// How many voices the current object is actually sounding.
    pub fn voices(&self) -> usize {
        self.set.shape().voice_count() as usize
    }

    /// The selector, so a measurement can ask what it found and how long it
    /// took to find it.
    pub fn selector(&self) -> &Selector {
        &self.sel
    }

    /// Where the partials stop standing apart, hertz.
    pub fn crossover_hz(&self) -> f32 {
        self.crossover
    }

    /// Replace the per-mode override table. Positions are the partial's place
    /// in the published `modes` frame, so an edit follows the bar the user
    /// dragged rather than a mode index they cannot see.
    pub fn set_edits(&mut self, edits: &[ModeEdit; MAX_EDITS]) {
        if self.edits != *edits {
            self.edits = *edits;
            self.edits_dirty = true;
        }
    }

    /// Resolve every live mode to the override that addresses it, if any.
    fn map_edits(&mut self) {
        let n = self.sel.result().len().min(self.edit_of.len());
        self.edit_of[..n].fill(NO_EDIT);
        let any = self.edits.iter().any(|e| e.is_set());
        if !any {
            return;
        }
        for (k, c) in self.sel.result()[..n].iter().enumerate() {
            for (e, edit) in self.edits.iter().enumerate() {
                if edit.matches(c.i, c.j) {
                    self.edit_of[k] = e as u8;
                    break;
                }
            }
        }
    }

    /// The override applying to mode slot `k`, or a neutral one.
    fn edit_at(&self, k: usize) -> ModeEdit {
        match self.edit_of.get(k) {
            Some(&e) if e != NO_EDIT => self.edits[e as usize],
            _ => ModeEdit::default(),
        }
    }

    pub fn edits(&self) -> &[ModeEdit; MAX_EDITS] {
        &self.edits
    }

    /// Take a settings snapshot. Cheap when nothing moved.
    pub fn configure(&mut self, s: &Settings) {
        let changed = self.set != *s;
        self.set = *s;
        if changed {
            self.curves_dirty = true;
        }
        self.pre
            .set(s.filter_hz, q_from_octaves(s.filter_oct), self.sr);
        for f in self.post.iter_mut() {
            f.set(s.filter_hz, q_from_octaves(s.filter_oct), self.sr);
        }
        self.limiter.set(s.limit_ceil_db, self.sr);
    }

    // -- readouts -----------------------------------------------------------

    /// `[in_l, in_r, out_l, out_r]`, linear peaks.
    pub fn meter(&self) -> [f32; 4] {
        self.meter
    }

    /// The readout frame; see `dsp::streams` for the layout.
    pub fn info_frame(&self) -> [f32; INFO_LEN] {
        let object = self.set.object();
        let guide = object.engine() == ObjEngine::Guide;
        let voices = self.voices();
        let used = if guide {
            (0..voices)
                .map(|v| self.guides[0][v].resonances().len())
                .sum::<usize>()
                * if self.stereo { 2 } else { 1 }
        } else {
            self.banks[0].len() * if self.stereo { 2 } else { 1 }
        };
        let available = if guide {
            (0..voices)
                .map(|v| self.guides[0][v].resonances().len())
                .sum::<usize>()
        } else {
            self.sel.available()
        };
        let t = self.tail.report();
        // **A field that does not apply publishes NaN, never zero.** A real
        // zero and an uncomputed one are indistinguishable to a panel, and a
        // plausible zero is worse than a blank: it reads as a measurement
        // nothing made. The page turns any non-finite value into an absent
        // readout, so this is the difference between "no wall to draw" and
        // "a wall at 0 Hz".
        let na = f32::NAN;
        [
            used as f32,
            available as f32,
            if guide { na } else { self.crossover },
            // Silent is a real answer and gets a number; not applicable does
            // not.
            if guide {
                na
            } else if t.level_db.is_finite() {
                t.level_db
            } else {
                -120.0
            },
            if self.set.limiter { self.limit_db } else { na },
            if guide { na } else { self.set.inharm_b() },
            // **NaN above one voice, rather than the root voice's length.**
            // A voiced rank has six lengths and one field cannot be all of
            // them. Publishing voice one's and labelling it was the other
            // option, and the label that would have to be written is "air
            // column 85.0 cm (voice 1 of 3)" — worse than not claiming a
            // length, because a number in the right place describing
            // something other than what a reader expects is the failure this
            // contract's NaN rule exists to prevent. A per-voice length can
            // be appended later if anyone asks for one.
            if guide && voices == 1 {
                self.guides[0][0].column_m()
            } else {
                na
            },
            if guide && voices == 1 {
                self.guides[0][0].loop_ms()
            } else {
                na
            },
            if guide && voices == 1 {
                self.guides[0][0].open_hz()
            } else {
                na
            },
            if guide { 1.0 } else { 0.0 },
            self.sel.progress(),
            self.readout_f0,
            self.ceiling_hz(),
            // **How many partials each voice has, so a page can say how many
            // it is not showing.** Publishing only what is drawn leaves a
            // voice reduced to one bar reading as a voice with one partial;
            // measured at an ordinary six-voice spread, four of six arrive
            // that way. NaN for a voice that is not sounding, which is the
            // same rule as every other field that does not apply.
            self.voice_available(0),
            self.voice_available(1),
            self.voice_available(2),
            self.voice_available(3),
            self.voice_available(4),
            self.voice_available(5),
            // **Where each voice's pitch came from**, so a face can say
            // which are held and which are free rather than inferring it.
            // 0 is a manual pitch, 1 is a note being held; NaN is a voice
            // that is not sounding. A slot recall from the panel writes the
            // parameters through the ordinary path, so it arrives here as
            // manual — which is true, and the page knows it was a slot
            // because the page did the writing.
            self.voice_source(0),
            self.voice_source(1),
            self.voice_source(2),
            self.voice_source(3),
            self.voice_source(4),
            self.voice_source(5),
        ]
    }

    fn voice_source(&self, v: usize) -> f32 {
        if v >= self.set.shape().voice_count() as usize {
            return f32::NAN;
        }
        if self.held[v] { 1.0 } else { 0.0 }
    }

    /// How many partials voice `v` has under the ceiling, or NaN if it is not
    /// sounding.
    fn voice_available(&self, v: usize) -> f32 {
        let shape = self.set.shape();
        if v >= shape.voice_count() as usize {
            return f32::NAN;
        }
        if self.set.object().engine() == ObjEngine::Guide {
            return self.guides[0]
                .get(v)
                .map(|g| g.resonances().len() as f32)
                .unwrap_or(f32::NAN);
        }
        let f_max = (self.sr * 0.49).min(AUDIBLE_MAX_HZ);
        let f0 = self.set.damping().f0.max(1e-3);
        let all = shape.available((f_max / f0) as f64);
        if shape.voice_count() == 1 {
            return all as f32;
        }
        // Per voice, the object's own reach with the ceiling divided by that
        // voice's transposition.
        let one = Shape { voices: 1, ..shape };
        one.available((f_max / f0) as f64 / shape.voice_ratio(v as u16)) as f32
    }

    /// The highest frequency the bank has a partial at, or **NaN when there
    /// is no ceiling** — because it has every partial the object has, or
    /// because the object is an air column with no mode list at all.
    ///
    /// The distinction is the point: a bar has twenty-eight partials and the
    /// bank runs every one, so there is nothing above which the object stops.
    /// A membrane at a low tuning has fifty thousand and the bank runs a few
    /// thousand, and where that runs out is a real edge a listener can hear —
    /// which is what the statistical tail exists to cover.
    ///
    /// Not zero for "none", because zero is a frequency and a panel cannot
    /// tell it from a wall at the bottom of the band.
    pub fn ceiling_hz(&self) -> f32 {
        if self.set.object().engine() == ObjEngine::Guide {
            return f32::NAN;
        }
        let info = self.banks[0].info();
        if info.is_empty() || info.len() >= self.sel.available() {
            return f32::NAN;
        }
        info.iter().fold(0.0f32, |m, i| m.max(i.hz))
    }

    /// The partials frame: up to [`MAX_EDITS`] partials, six floats each,
    /// ascending in frequency and terminated by a zero frequency.
    pub fn modes_frame(&self) -> &[f32] {
        &self.modes_frame
    }

    /// The engine's own magnitude response, dB, 20 Hz … Nyquist log-spaced.
    pub fn response_curve(&self) -> &[f32] {
        &self.response
    }

    /// The frequency of response point `i`.
    pub fn response_hz(i: usize, sr: f32) -> f32 {
        let lo = 20.0f32;
        let hi = (sr * 0.5).max(lo * 2.0);
        lo * (hi / lo).powf(i as f32 / (RESPONSE_POINTS - 1) as f32)
    }

    // -- the audio path -----------------------------------------------------

    /// Process a block of any length, in place.
    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len());
        let mut done = 0usize;
        while done < n {
            let take = (n - done).min(crate::dsp::bank::BLOCK);
            self.block(&mut l[done..done + take], &mut r[done..done + take]);
            done += take;
        }
    }

    fn block(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len();
        let s = self.set;
        let mut in_peak = (0.0f32, 0.0f32);
        for i in 0..n {
            in_peak.0 = in_peak.0.max(l[i].abs());
            in_peak.1 = in_peak.1.max(r[i].abs());
            self.dry_l[i] = l[i];
            self.dry_r[i] = r[i];
        }
        if s.bypass {
            self.meter = [in_peak.0, in_peak.1, in_peak.0, in_peak.1];
            return;
        }

        // The exciter: the mono sum, gated by Dry/Wet so the tail is never
        // chopped, then optionally band-passed before the object.
        let mix = s.mix.clamp(0.0, 1.0);
        for i in 0..n {
            self.exc[i] = 0.5 * (self.dry_l[i] + self.dry_r[i]) * mix;
        }
        if s.filter_on && !s.filter_post {
            for i in 0..n {
                self.exc[i] = self.pre.bp(self.exc[i]);
            }
        }

        self.prepare(n);

        let object = s.object();
        if object.engine() == ObjEngine::Guide {
            let voices = s.shape().voice_count() as usize;
            let (left, right) = self.guides.split_at_mut(1);
            left[0][0].process(&self.exc[..n], &mut self.wl[..n], &mut self.wr[..n]);
            for v in 1..voices {
                left[0][v].process(&self.exc[..n], &mut self.bl[..n], &mut self.br[..n]);
                for i in 0..n {
                    self.wl[i] += self.bl[i];
                    self.wr[i] += self.br[i];
                }
            }
            if self.stereo {
                self.br[..n].fill(0.0);
                for v in 0..voices {
                    right[0][v].process(&self.exc[..n], &mut self.bl[..n], &mut self.gr[..n]);
                    for i in 0..n {
                        self.br[i] += self.gr[i];
                    }
                }
                self.wr[..n].copy_from_slice(&self.br[..n]);
            }
        } else {
            let (a, b) = self.banks.split_at_mut(1);
            a[0].process(&self.exc[..n], &mut self.wl[..n], &mut self.wr[..n]);
            if self.stereo {
                b[0].process(&self.exc[..n], &mut self.bl[..n], &mut self.br[..n]);
                self.wr[..n].copy_from_slice(&self.br[..n]);
            }
            self.tail
                .process(&self.exc[..n], &mut self.wl[..n], &mut self.wr[..n]);
        }

        if s.filter_on && s.filter_post {
            for i in 0..n {
                self.wl[i] = self.post[0].bp(self.wl[i]);
                self.wr[i] = self.post[1].bp(self.wr[i]);
            }
        }

        // Width, the pair's mix: at zero both objects go equally to both
        // sides, at one each goes to its own.
        let width = s.width.clamp(0.0, 1.0);
        let gain = 10f32.powf(s.gain_db / 20.0);
        let bleed = s.bleed.clamp(0.0, 1.0);
        let dry = 1.0 - mix;
        let mut out_peak = (0.0f32, 0.0f32);
        for i in 0..n {
            let m = 0.5 * (self.wl[i] + self.wr[i]);
            let side = 0.5 * (self.wl[i] - self.wr[i]) * width;
            let mut a = (m + side + bleed * self.dry_l[i]) * gain;
            let mut b = (m - side + bleed * self.dry_r[i]) * gain;
            if s.limiter {
                let (ga, gb) = self.limiter.run(a, b);
                a = ga;
                b = gb;
            }
            l[i] = a + dry * self.dry_l[i];
            r[i] = b + dry * self.dry_r[i];
            out_peak.0 = out_peak.0.max(l[i].abs());
            out_peak.1 = out_peak.1.max(r[i].abs());
        }
        if s.limiter {
            let g = self.limiter.take_reduction_db();
            self.limit_db = g.min(0.0);
        } else {
            self.limit_db = 0.0;
        }
        self.meter = [in_peak.0, in_peak.1, out_peak.0, out_peak.1];
        self.refresh_curves();
    }

    /// Everything that happens once per block rather than once per sample:
    /// the oscillator, the mode search, the retuning, and the tail's level.
    fn prepare(&mut self, n: usize) {
        let s = self.set;
        let object = s.object();
        let base = s.base_hz();

        self.lfo.advance(s.lfo_rate_hz, self.sr, n);
        let (mut a, mut b) = (base, base);
        let phase = (s.lfo_phase_deg / 360.0).rem_euclid(1.0);
        let mut stereo = s.spread > 0.0;
        if s.lfo_on && s.lfo_depth_st > 0.0 {
            let va = self.lfo.value(s.lfo_shape, 0.0);
            let vb = self.lfo.value(s.lfo_shape, phase);
            a *= 2f32.powf(s.lfo_depth_st * va / 12.0);
            b *= 2f32.powf(s.lfo_depth_st * vb / 12.0);
            if (va - vb).abs() > 1e-6 {
                stereo = true;
            }
        }
        if s.spread > 0.0 {
            let c = SPREAD_MAX_CENTS * s.spread.clamp(0.0, 1.0) * 0.5;
            a *= 2f32.powf(c / 1200.0);
            b *= 2f32.powf(-c / 1200.0);
        }
        if !stereo {
            b = a;
        }
        self.stereo = stereo;

        if object.engine() == ObjEngine::Guide {
            self.prepare_guide(a, b);
            return;
        }
        self.prepare_bank(a, b);
    }

    fn prepare_guide(&mut self, a: f32, b: f32) {
        let s = self.set;
        let g = guide::Settings {
            f0: a,
            opening: if s.object() == Object::Tube {
                1.0
            } else {
                s.opening.clamp(0.0, 1.0)
            },
            radius_mm: s.radius_mm,
            decay: s.decay_s,
            tilt_db_oct: s.bright_db_oct,
            disperse: s.disperse,
            hit: s.hit.x,
            pos_l: s.pos_l.x,
            pos_r: s.pos_r.x,
        };
        // Each voice is the same column at another pitch, so it is the same
        // settings with `f0` moved. Reconfiguring only what changed keeps a
        // voice's loop from being rebuilt because a neighbour moved.
        let shape = s.shape();
        let voices = shape.voice_count() as usize;
        for side in 0..if self.stereo { 2 } else { 1 } {
            let root = if side == 0 { a } else { b };
            for v in 0..CHORD_VOICES {
                let mut gv = g;
                gv.f0 = root * shape.voice_ratio(v as u16) as f32;
                // A voice past the count is silenced rather than left
                // ringing at its old pitch.
                gv.decay = if v < voices { g.decay } else { 0.0 };
                if *self.guides[side][v].settings() != gv {
                    self.guides[side][v].configure(&gv);
                    if side == 0 {
                        self.curves_dirty = true;
                    }
                }
            }
        }
        self.f0 = [a, b];
        self.banks[0].begin(0);
        self.banks[1].begin(0);
        self.publish_modes();
    }

    fn prepare_bank(&mut self, a: f32, b: f32) {
        let s = self.set;
        let damping = s.damping();
        let shape = s.shape();
        let f_max = (self.sr * 0.49).min(AUDIBLE_MAX_HZ);

        // Where the partials stop standing apart. It moves with the decay
        // setting, which is why the budget is spent by contribution rather
        // than by frequency — and it is cached, because counting an object's
        // partials at ninety-six frequencies is not a per-block cost.
        let key = (shape, damping, f_max);
        if self.cross_key != Some(key) {
            self.cross_key = Some(key);
            self.crossover = damp::crossover_hz(&damping, f_max, |hz| {
                select::density_at(&shape, damping.f0, hz)
            });
        }
        let crossover = self.crossover;

        let req = Request {
            shape,
            exc: s.hit,
            left: s.pos_l,
            right: s.pos_r,
            f0: damping.f0,
            f_max,
            budget: s.modes.clamp(1, crate::dsp::bank::MAX_MODES),
            order: s.order.min(2),
            tilt_db_oct: s.bright_db_oct,
            damping,
            crossover_hz: crossover,
            sr: self.sr,
        };
        // **A search whose premise has changed is abandoned, not finished.**
        // It used to read `&& !self.sel.searching()`, so a settings change
        // arriving during a search was dropped on the floor and the engine
        // went on computing the answer to the previous question. With a
        // search that could not terminate — a membrane at 1.2 Hz has of the
        // order of a hundred million partials under Nyquist — that meant the
        // change was ignored *for the rest of the session*: the object read
        // String while the bank still held the membrane's two-index modes,
        // and no parameter would bring it back. Found by the panel agent,
        // from three controls at their minima and a preset load.
        //
        // Bounding the walk stops the search running away, and that alone
        // ends the wedge. The guard was still wrong on its own terms, so it
        // is gone — up to [`RESTART_LIMIT`], which is what stops the cure
        // becoming the disease: restarting on *every* block would mean no
        // search ever finished under continuous automation, and a bank frozen
        // on its old set looks exactly like one that is following.
        if self.sel.needs_rebuild(&req) && self.restarts < RESTART_LIMIT {
            if self.sel.searching() {
                self.restarts += 1;
            }
            self.sel.start(req);
        }
        if self.sel.searching() && self.sel.step(select::WORK_PER_BLOCK) {
            self.restarts = 0;
        }

        let fresh = self.sel.generation() != self.loaded;
        let moved = (self.f0[0] - a).abs() > 1e-4 * a || (self.f0[1] - b).abs() > 1e-4 * b;
        let rebuilt = fresh || self.edits_dirty || self.applied != Some(damping);
        if !rebuilt && !moved {
            return;
        }
        if !rebuilt {
            // Only the pitch moved, which is the oscillator's case: keep every
            // mode's decay, its gains and its normalisation, and turn its
            // coefficient pair to the new angle.
            self.f0 = [a, b];
            let sides = [a, b];
            let nyq = self.sr * 0.49;
            let count = self.banks[0].len();
            for k in 0..count {
                let ratio = self.sel.result()[k].ratio;
                let cents = 2f32.powf(self.edit_at(k).cents / 1200.0);
                for side in 0..if self.stereo { 2 } else { 1 } {
                    let hz = (sides[side] * ratio * cents).clamp(1.0, nyq);
                    self.banks[side].retune(k, hz);
                }
            }
            if self.readout_wait == 0 {
                self.readout_wait = READOUT_BLOCKS;
                self.curves_dirty = true;
                self.publish_modes();
            } else {
                self.readout_wait -= 1;
            }
            return;
        }
        self.f0 = [a, b];
        self.loaded = self.sel.generation();
        self.edits_dirty = false;
        self.applied = Some(damping);
        self.map_edits();

        // The bank's overall scale: unit power gain, so that turning the mode
        // budget up does not turn the device up. Each mode is already
        // normalised so its own peak cannot exceed what it was asked for,
        // which is what keeps a single long-decaying partial from running
        // away; this is the other half, and between them the limiter is still
        // needed — no static choice removes it.
        let chosen = self.sel.result();
        let mut power = 0.0f64;
        for (k, c) in chosen.iter().enumerate() {
            let g = self.edit_gain(k);
            let ai = c.amp_in * g;
            power += 0.5 * ((ai * c.amp_l) as f64).powi(2) + 0.5 * ((ai * c.amp_r) as f64).powi(2);
        }
        self.scale = if power > 0.0 {
            (1.0 / power.sqrt()) as f32
        } else {
            0.0
        };

        let nyq = self.sr * 0.49;
        let sides: [f32; 2] = [a, b];
        let count = chosen.len();
        // Where each partial sat before inharmonicity moved it. Filled into
        // a buffer that was allocated at construction, because this runs on
        // the audio thread; `Shape::base_ratio` is the only thing that knows
        // it, and the panel would otherwise have to invert a stretch it
        // cannot see.
        for (k, c) in chosen.iter().enumerate().take(self.base_ratios.len()) {
            self.base_ratios[k] = shape.base_ratio(c.i, c.j) as f32;
        }
        for side in 0..if self.stereo { 2 } else { 1 } {
            self.banks[side].begin(count);
        }
        for k in 0..count {
            let c = chosen[k];
            let edit = self.edit_at(k);
            let cents = 2f32.powf(edit.cents / 1200.0);
            let gain = self.scale * 10f32.powf(edit.db / 20.0);
            for side in 0..if self.stereo { 2 } else { 1 } {
                let hz = sides[side] * c.ratio * cents;
                let live = hz > 1.0 && hz < nyq;
                let t60 = (damping.t60_at(hz) * edit.decay.clamp(0.01, 100.0)).max(1e-4);
                let (gl, gr) = if !self.stereo {
                    (c.amp_l, c.amp_r)
                } else if side == 0 {
                    (c.amp_l, 0.0)
                } else {
                    (0.0, c.amp_r)
                };
                let amp_in = if live { c.amp_in * gain } else { 0.0 };
                let info = ModeInfo {
                    hz,
                    base_hz: sides[side] * self.base_ratios[k] * cents,
                    t60,
                    amp_l: (c.amp_in * gain * c.amp_l).abs(),
                    amp_r: (c.amp_in * gain * c.amp_r).abs(),
                    // The same partial with unit mode shapes at both
                    // contacts: what the tilt alone gives it.
                    bare: (guide::tilt(hz, damping.f0, s.bright_db_oct) * gain).abs(),
                    i: c.i,
                    j: c.j,
                };
                // A mode that has been given to a different partial has state
                // that means nothing; one that kept its partial keeps ringing
                // through the rebuild, which is what stops a control change
                // from cutting a tail off.
                let same = {
                    let old = self.banks[side].info();
                    k < old.len() && old[k].i == c.i && old[k].j == c.j
                };
                if !same {
                    self.banks[side].clear_state(k);
                }
                self.banks[side].set_mode(k, hz.clamp(1.0, nyq), t60, amp_in, gl, gr, info, !same);
            }
        }
        if !self.stereo {
            self.banks[1].begin(0);
        }

        // The tail, levelled to what the selection actually left behind.
        self.residual = *self.sel.residual();
        let sq = (self.scale * self.scale) as f64;
        for v in self.residual.iter_mut() {
            *v = (*v as f64 * sq) as f32;
        }
        self.tail
            .configure(&damping, crossover, &self.residual, self.set.tail);
        self.curves_dirty = true;
        self.readout_wait = READOUT_BLOCKS;
        self.publish_modes();
    }

    fn edit_gain(&self, k: usize) -> f32 {
        let e = self.edit_at(k);
        if e.is_set() && !e.is_neutral() {
            10f32.powf(e.db / 20.0)
        } else {
            1.0
        }
    }

    /// Fill the partials frame with the loudest published partials, ascending
    /// in frequency.
    fn publish_modes(&mut self) {
        // **The ruler is part of the picture, so it is taken here.** The mode
        // table is a sticky stream, sent only when it changes; `info` goes out
        // every block. So a page holding the newest `info` and the last table
        // it received was dividing one moment's frequencies by another
        // moment's fundamental, and a partial whose ratio is exactly 1 drew at
        // 1.2. Taking `f0_hz` at the instant the rows are built makes the two
        // one moment by construction, and costs nothing when they agree, which
        // is almost always. Found by the panel agent, who could see it and
        // could not fix it: the lowest *drawn* partial is not the fundamental
        // in general — a strike on a node removes partial 1 outright — so the
        // ruler cannot be inferred from the bars.
        self.readout_f0 = self.f0[0];
        self.modes_frame.fill(0.0);
        let object = self.set.object();
        let mut rows: [Row; MAX_EDITS] = [[0.0; MODE_FIELDS]; MAX_EDITS];
        let mut n = 0usize;
        if object.engine() == ObjEngine::Guide {
            let voices = self.voices();
            for v in 0..voices {
                for res in self.guides[0][v].resonances() {
                    if n == MAX_EDITS {
                        break;
                    }
                    rows[n] = [
                        res.n as f32,
                        v as f32,
                        res.hz,
                        db(res.amp_l),
                        db(res.amp_r),
                        res.t60,
                        db(res.bare),
                        // An air column's series is set by its terminations,
                        // so there is no unstretched frequency to report.
                        res.hz,
                    ];
                    n += 1;
                }
            }
            rows[..n].sort_unstable_by(|p, q| {
                p[2].partial_cmp(&q[2]).unwrap_or(std::cmp::Ordering::Equal)
            });
        } else {
            // The loudest, not the lowest: the display should show what is
            // audible, which is the same decision the selector makes.
            let info = self.banks[0].info();
            let row_of = |m: &ModeInfo| -> Row {
                [
                    m.i as f32,
                    m.j as f32,
                    m.hz,
                    db(m.amp_l),
                    db(m.amp_r),
                    m.t60,
                    db(m.bare),
                    m.base_hz,
                ]
            };
            let mut worst = 0usize;
            for (k, m) in info.iter().enumerate() {
                // An edited partial is always published, however quiet the
                // edit made it. Turning one down forty decibels should not
                // make it leave the picture being used to edit it.
                let keep = self.edit_of.get(k).is_some_and(|&e| e != NO_EDIT);
                let a = if keep {
                    f32::INFINITY
                } else {
                    m.amp_l.max(m.amp_r)
                };
                if n < MAX_EDITS {
                    rows[n] = row_of(m);
                    n += 1;
                    if n == MAX_EDITS {
                        worst = argmin_amp(&rows[..n]);
                    }
                } else {
                    let cur = 10f32
                        .powf(rows[worst][3] / 20.0)
                        .max(10f32.powf(rows[worst][4] / 20.0));
                    if a > cur {
                        rows[worst] = row_of(m);
                        worst = argmin_amp(&rows[..n]);
                    }
                }
            }
            rows[..n].sort_unstable_by(|p, q| {
                p[2].partial_cmp(&q[2]).unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        // **Every sounding voice keeps its loudest partial, however quiet.**
        //
        // The cut is over the whole table, so it distributes by level, and a
        // chord voicing is not level: measured over the real series at an
        // ordinary 6 dB spread, a string published 49/12/3/0/0/0 partials per
        // voice and a membrane 61/3/0/0/0/0. Three sounding voices with no
        // bars at all, and four — the user hears six voices, sees two, and
        // has no way to tell "silent" from "lost the cut".
        //
        // It is the same rule that already keeps an edited mode in the
        // picture however far down the edit took it, for the same reason: a
        // display that claims to show what is sounding must show it. Found
        // and quantified by the panel agent, prototyping against the real
        // tables rather than reasoning about them.
        let voices = self.set.shape().voice_count() as usize;
        if voices > 1 {
            for v in 0..voices {
                if rows[..n].iter().any(|r| r[1] as usize == v && r[2] > 0.0) {
                    continue;
                }
                let Some(best) = self.loudest_of_voice(v) else {
                    continue;
                };
                let slot = if n < MAX_EDITS {
                    n += 1;
                    n - 1
                } else {
                    argmin_amp(&rows[..n])
                };
                rows[slot] = best;
            }
            rows[..n].sort_unstable_by(|p, q| {
                p[2].partial_cmp(&q[2]).unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        for (k, row) in rows[..n].iter().enumerate() {
            let base = k * MODE_FIELDS;
            self.modes_frame[base..base + MODE_FIELDS].copy_from_slice(row);
        }
    }

    /// Redraw a slice of the response curve, so a redraw never lands inside
    /// one block.
    fn refresh_curves(&mut self) {
        if !self.curves_dirty && self.response_cursor == 0 {
            return;
        }
        if self.curves_dirty {
            self.curves_dirty = false;
            self.response_cursor = 0;
        }
        let guide = self.set.object().engine() == ObjEngine::Guide;
        let end = (self.response_cursor + RESPONSE_CHUNK).min(RESPONSE_POINTS);
        for i in self.response_cursor..end {
            let hz = Self::response_hz(i, self.sr);
            let m = if guide {
                (0..self.voices())
                    .map(|v| self.guides[0][v].response(hz))
                    .sum::<f32>()
            } else {
                self.banks[0].response(hz)
            };
            self.response[i] = db(m);
        }
        self.response_cursor = if end >= RESPONSE_POINTS { 0 } else { end };
        if self.response_cursor == 0 {
            normalise(&mut self.response);
        }
    }
}

/// One published partial: the `modes` frame's row layout.
type Row = [f32; MODE_FIELDS];

impl Resonator {
    /// The loudest partial belonging to one voice, as a published row.
    ///
    /// Used only to honour the rule above, so it looks in whichever engine is
    /// running rather than assuming a bank.
    fn loudest_of_voice(&self, v: usize) -> Option<Row> {
        if self.set.object().engine() == ObjEngine::Guide {
            let mut best: Option<&crate::dsp::guide::Resonance> = None;
            for r in self.guides[0].get(v)?.resonances() {
                let a = r.amp_l.abs().max(r.amp_r.abs());
                if best.is_none_or(|b| a > b.amp_l.abs().max(b.amp_r.abs())) {
                    best = Some(r);
                }
            }
            let r = best?;
            return Some([
                r.n as f32,
                v as f32,
                r.hz,
                db(r.amp_l),
                db(r.amp_r),
                r.t60,
                db(r.bare),
                r.hz,
            ]);
        }
        let info = self.banks[0].info();
        let mut best: Option<&ModeInfo> = None;
        for m in info.iter().filter(|m| m.j as usize == v) {
            let a = m.amp_l.abs().max(m.amp_r.abs());
            if best.is_none_or(|b| a > b.amp_l.abs().max(b.amp_r.abs())) {
                best = Some(m);
            }
        }
        let m = best?;
        Some([
            m.i as f32,
            m.j as f32,
            m.hz,
            db(m.amp_l),
            db(m.amp_r),
            m.t60,
            db(m.bare),
            m.base_hz,
        ])
    }
}

fn argmin_amp(rows: &[Row]) -> usize {
    let mut best = 0usize;
    let mut lo = f32::INFINITY;
    for (k, r) in rows.iter().enumerate() {
        let a = r[3].max(r[4]);
        if a < lo {
            lo = a;
            best = k;
        }
    }
    best
}

fn db(x: f32) -> f32 {
    20.0 * x.abs().max(1e-7).log10()
}

/// Put a curve's peak at 0 dB, because its absolute height depends on the
/// bank's normalisation and its shape is what the panel is drawing.
fn normalise(curve: &mut [f32]) {
    let mut peak = f32::NEG_INFINITY;
    for v in curve.iter() {
        if *v > peak {
            peak = *v;
        }
    }
    if !peak.is_finite() {
        return;
    }
    for v in curve.iter_mut() {
        *v = (*v - peak).max(-120.0);
    }
}
