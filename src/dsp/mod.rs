//! The DSP of Noob Resonator, and the bridge description shared by the
//! plug-in and the standalone.
//!
//! ## Layout
//!
//! | module | contents |
//! |---|---|
//! | [`object`] | the eight objects: their partial series and their mode shapes, solved rather than tabulated |
//! | [`damp`] | the damping law, the decrement, and the resolvability crossover |
//! | [`bank`] | the mode bank: the coupled form, mode-major, lane-buffered |
//! | [`select`] | **which** modes to keep, which is the decision this device is about |
//! | [`preset`] | the factory presets, and the shape every preset has |
//! | [`guide`] | the waveguide: the air columns, where the reflection sign is the whole difference |
//! | [`tail`] | the statistical extension above the crossover |
//! | [`filters`] | the exciter's band-pass, the shelves, and the zero-latency limiter |
//! | [`lfo`] | the pitch oscillator |
//! | [`engine`] | the whole chain, the mode table, the readouts |
//! | [`source`] | the standalone's demo signals |
//! | this file | parameter ids and specs, streams, the bridge builder, the [`Processor`] |
//!
//! ## Parameters
//!
//! [`param_specs`] describes every parameter once; the standalone builds its
//! bridge from it directly and the plug-in's nih-plug parameters use the same
//! ids, so the same page drives both. **Ids are stable API.**
//!
//! Where a range came from the device this one answers, it came from that
//! device's own serialised parameter file or from its engine vendor's
//! published calibration, and the table says which. Where we differ, it says
//! that too.
//!
//! | id | range / labels | default | ours or theirs |
//! |---|---|---|---|
//! | `type` | Beam, Marimba, String, Membrane, Plate, Pipe, Tube, **Membrane Round** | String | theirs, plus one: a drum head is a disc and theirs is a rectangle |
//! | `tune` | 20 … 4000 Hz, log | 220 | **ours** — theirs serialises as a bare 0…1 and its range is on no file on disk |
//! | `transpose` | −48 … +48 st | 0 | theirs |
//! | `fine` | −50 … +50 ct | 0 | theirs |
//! | `mode_budget` | 4 … 4096, log | 1024 | **improved** — a stated count, where theirs is a four-position quality menu that publishes no number. Not `modes`, which is the stream, and deliberately not "quality", which is their word for a control that truncates by frequency and then needs a Bleed knob to restore what it threw away |
//! | `select` | Loudest, Lowest, Log Spread | Loudest | **ours** — the decision this plug-in exists for, made audible |
//! | `ratio` | 0.2 … 5, log | 1.0 | theirs |
//! | `bar_tuning` | Marimba 4:1, Xylophone 3:1 | Marimba | **ours** |
//! | `bar_third` | 9.2x, 10x | 9.2x | **ours** — two sources disagree and this is the disagreement |
//! | `radius` | 1 … 100 mm, log | 20 | **improved** — a physical radius in millimetres, in the direction wall loss actually moves |
//! | `opening` | 0 … 100 % | 0 | theirs |
//! | `decay` | 0.02 … 60 s, log | 2.0 | **improved** — seconds, where theirs is a bare 0…1 |
//! | `material` | −1 … +1 | −0.5 | theirs, and their engine vendor's published law |
//! | `damp_corner` | 100 … 20000 Hz, log | 20000 (inert) | **ours** — the second parameter of a two-parameter loss model |
//! | `damp_hi` | −2 … +1 | −1 | **ours** |
//! | `tail` | toggle | on | **ours** |
//! | `bright` | −6 … +6 dB/oct | **−3** | **improved** — their engine vendor's own calibration, printed in the unit they calibrate it in. Not zero: at exactly flat the contribution ordering degenerates, and [`select`] says why |
//! | `inharm` | −100 … +100 % | 0 | **improved** — the positive half is Fletcher's stiff string with `B` published; the negative half is labelled synthetic |
//! | `hit`, `hit_y` | 0 … 100 % | 20, 20 | X theirs, **Y ours** |
//! | `pos_l`, `pos_l_y` | 0 … 100 % | 30, 30 | X theirs, **Y ours** |
//! | `pos_r`, `pos_r_y` | 0 … 100 % | 70, 70 | X theirs, **Y ours** |
//! | `spread` | 0 … 100 % | 0 | theirs |
//! | `width` | 0 … 100 % | 100 | theirs |
//! | `filter_on` | toggle | off | theirs |
//! | `filter_freq` | 50 … 18000 Hz, log | 1000 | theirs, exactly |
//! | `filter_width` | 0.5 … 9 oct | 4.0 | theirs' range, in a stated unit |
//! | `filter_place` | Pre, Post | Pre | **ours** — theirs is documented in one place and placed in another |
//! | `lfo_on` | toggle | off | theirs |
//! | `lfo_shape` | Sine, Square, Triangle, Ramp Up, Ramp Down, S&H, Random Ramp | Sine | theirs, in their own ordinal order |
//! | `lfo_rate` | 0.01 … 20 Hz, log | 1.0 | theirs |
//! | `lfo_depth` | 0 … 12 st | 0 | **improved** — semitones, where theirs is a bare amount |
//! | `lfo_phase` | 0 … 360 ° | 180 | theirs |
//! | `bleed` | 0 … 100 % | 0 | theirs, kept as a blend and not load-bearing |
//! | `mix` | 0 … 100 % | 100 | theirs — an input gate on the wet send, so a tail is never chopped |
//! | `gain` | −36 … +36 dB | 0 | **improved** — bipolar, where theirs only attenuates |
//! | `limiter` | toggle | on | **improved** — optional, and zero-latency |
//! | `limit_ceil` | −24 … 0 dB | −0.3 | **ours** |
//! | `bypass` | toggle | off | |
//! | `src_kind`, `src_level` | standalone only | | |
//! | `src_freq` | 0.5 … 20 Hz, log | 2 | standalone only, and a **strike rate** rather than a pitch: three of the five demo sources are struck |
//!
//! ## Streams
//!
//! | id | kind | values | rate | contents |
//! |---|---|---|---|---|
//! | `meter` | meter | 4 | every block | `[in_l, in_r, out_l, out_r]`, linear peaks, 1.0 = 0 dBFS |
//! | `modes` | raw, sticky | 512 | on change | 64 partials × `[i, j, hz, db_l, db_r, t60_s, db_bare, base_hz]`, ascending in `hz`, terminated by `hz = 0`. For a mode bank these are the **loudest** 64 and `(i, j)` are the mode's own indices; for an air column they are the loop's lowest 64 resonances and `i` is the resonance number. `db_bare` is the level the partial would have had with unit mode shapes at both contacts, so the gap between it and `db_l` is the energy a node **removed**; `base_hz` is where the partial sat before Inharm stretched the series. **A partial with an override is always published**, however quiet the override made it |
//! | `info` | raw | 13 | every block | `[modes_used, modes_available, crossover_hz, tail_db, limit_gr_db, inharm_b, column_m, loop_ms, open_hz, engine, build, f0_hz, ceiling_hz]` — `engine` is 0 for the mode bank and 1 for the waveguide; `build` is 0…1 for how far a pending mode search has got and 1 when it has settled; `f0_hz` is the sounding fundamental after transpose, fine and the oscillator, which is the number every ratio on the display is divided by; `ceiling_hz` is where the bank runs out. `modes_available` is capped at `object::MAX_CANDIDATES`, because an object's mode set is not always finite — a negative `inharm` compresses the whole series under a fixed ratio, and a low enough fundamental puts hundreds of millions of a membrane's partials in the band — so at that value it is a floor on what the object has rather than a total, and `ceiling_hz` is then a real number saying the bank does not reach the top. **Any field that does not apply publishes NaN rather than zero** — the air-column fields on a bank, the bank fields on an air column, the limiter's reduction when it is off, and `ceiling_hz` when the bank holds every partial the object has. A real zero and an uncomputed one are indistinguishable to a panel, and a plausible zero reads as a measurement nothing made |
//! | `response` | curve, sticky | 512 | on change | the engine's own magnitude in dB, 20 Hz … Nyquist log-spaced, normalised to its own peak |
//!
//! ## The mode table
//!
//! The per-partial overrides live in the **UI store**, under the key `modes`,
//! as `{"edits":[{"i":3,"j":0,"cents":-12,"db":-6,"decay":1.5}]}`.
//!
//! **`i` and `j` are the partial's own indices**, which the `modes` stream
//! publishes in each frame's first two floats — not a row number. An edit
//! belongs to *that resonance*: change `Selection` or the mode budget and the
//! frame becomes a different set of partials in a different order, and an
//! override keyed by position would silently move to something the user never
//! touched. `j` is zero for every one-dimensional object.
//!
//! The store is persisted inside the plug-in's own state, so a project
//! reloads sounding as it was saved with no editor open, and a store hook
//! turns each write into a lock-free table the audio thread reads once per
//! block.
//!
//! Exposing that table costs nothing at runtime and is native to this
//! architecture and to no other: a modal bank *is* a list of frequency, gain
//! and decay triples, and every global knob on it exists to generate that
//! list from a formula.
//!
//! ## Presets
//!
//! The **factory** set is generated from [`preset`] into the bridge metadata
//! under `presets`, because a preset here is physics — an object, a damping
//! law, a bore — and written as a `Settings` struct it cannot fall outside a
//! range or contradict the object it names. **User** presets are the page's,
//! in the interface store under `presets`, in the same shape.
//!
//! Applying is the page's either way, and that is a fact about hosts rather
//! than a division of labour: a parameter change has to travel through the
//! host to be recorded and undoable, and only the editor can do that.
//!
//! Every preset is `{v, name, group, description, values, modes}`, with
//! `values` **keyed by parameter id in plain units** — never an index, which
//! breaks the moment a parameter is appended. On load every parameter is set,
//! to `values[id]` where the preset names it and otherwise to its own default,
//! so a preset fully determines the sound; unknown ids are ignored rather than
//! refused. `bypass` is never carried. `modes` is mandatory and **always
//! replaces**, so an empty list clears the user's overrides rather than
//! leaving them.
//!
//! ## Real-time rules
//!
//! Everything reachable from [`Processor::process`] runs without allocation,
//! locks or input and output. Parameters are read from atomics into a
//! [`engine::Settings`] snapshot once per block; the mode search is spread
//! across blocks with a bounded work budget so no block ever pays for all of
//! it.

pub mod bank;
pub mod damp;
pub mod engine;
pub mod filters;
pub mod guide;
pub mod lfo;
pub mod object;
pub mod preset;
pub mod select;
pub mod source;
pub mod tail;

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use noob_vst_webgui_framework::{
    AudioHandle, NoobVstWebguiFramework, ParamSpec, StreamKind, StreamSpec,
};
use serde_json::{Value, json};

pub use engine::{
    INFO_LEN, MAX_EDITS, MODE_FIELDS, ModeEdit, RESPONSE_POINTS, Resonator, SPREAD_MAX_CENTS,
    Settings,
};
pub use lfo::LFO_NAMES;
pub use object::{BAR_THIRD_NAMES, BAR_TUNING_NAMES, OBJECT_NAMES, Object, Point};
pub use select::SELECT_NAMES;
pub use source::{SOURCE_NAMES, Source};

/// Values in one `meter` frame.
pub const METER_LEN: usize = 4;
/// Where the exciter's band-pass can sit.
pub const FILTER_PLACE_NAMES: [&str; 2] = ["Pre", "Post"];

/// Stream indices, in the order [`streams`] declares them.
#[derive(Clone, Copy, Debug)]
pub struct StreamIx {
    pub meter: usize,
    pub modes: usize,
    pub info: usize,
    pub response: usize,
}

/// The fixed stream layout.
pub const STREAM_IX: StreamIx = StreamIx {
    meter: 0,
    modes: 1,
    info: 2,
    response: 3,
};

/// The streams (see the module docs for the layouts).
pub fn streams(sr: f32) -> Vec<StreamSpec> {
    vec![
        StreamSpec::new("meter", METER_LEN)
            .name("Meter")
            .kind(StreamKind::Meter)
            .channels(2)
            .meta(json!({ "layout": "in_l,in_r,out_l,out_r", "sample_rate": sr })),
        StreamSpec::new("modes", MAX_EDITS * MODE_FIELDS)
            .name("Partials")
            .kind(StreamKind::Raw)
            .sticky()
            .meta(json!({
                "layout": "i,j,hz,db_l,db_r,t60_s,db_bare,base_hz",
                "fields": MODE_FIELDS,
                "max_partials": MAX_EDITS,
                "terminator": "hz = 0",
                "note": "the loudest partials for a mode bank, the lowest resonances for an air column; \
                         an edited partial is always here however quiet the edit made it"
            })),
        StreamSpec::new("info", INFO_LEN)
            .name("Readouts")
            .kind(StreamKind::Raw)
            .meta(json!({
                "layout": "modes_used,modes_available,crossover_hz,tail_db,limit_gr_db,inharm_b,column_m,loop_ms,open_hz,engine,build,f0_hz,ceiling_hz"
            })),
        StreamSpec::new("response", RESPONSE_POINTS)
            .name("Response")
            .kind(StreamKind::Curve)
            .sticky()
            .meta(json!({
                "hz_range": [20.0, sr * 0.5],
                "points": RESPONSE_POINTS,
                "unit": "dB",
                "log": true,
                "note": "the engine's own response, normalised to its own peak"
            })),
    ]
}

/// Every parameter (see the module docs). `with_source` adds the standalone's
/// demo-source parameters, which are not automatable.
pub fn param_specs(with_source: bool) -> Vec<ParamSpec> {
    let d = Settings::default();
    let pct = |id: &str, name: &str, default: f32, group: &str| {
        ParamSpec::new(id, name)
            .range(0.0, 100.0)
            .default(default)
            .unit("%")
            .group(group)
    };
    let mut v = vec![
        ParamSpec::new("type", "Object")
            .labels(OBJECT_NAMES)
            .default(d.object as f32)
            .group("body"),
        ParamSpec::new("tune", "Tune")
            .range(20.0, 4000.0)
            .log()
            .default(d.tune_hz)
            .unit("Hz")
            .group("body"),
        ParamSpec::new("transpose", "Transpose")
            .range(-48.0, 48.0)
            .steps(97)
            .default(d.transpose)
            .unit("st")
            .group("body"),
        ParamSpec::new("fine", "Fine")
            .range(-50.0, 50.0)
            .default(d.fine_cents)
            .unit("ct")
            .group("body"),
        // **A count, published as a continuous log taper, and the fraction
        // is real.** The engine rounds it to a whole number of resonators,
        // so a page must round it too or it prints "24.0" for a bank of 24.
        // Neither `steps` nor a table taper can fix that here and both cost
        // fidelity: they snap in the normalized domain, so a preset asking
        // for 1,024 modes loads as 1,021. Measured, not assumed. The manifest
        // has no way to say "this one is a count", which is a gap in the
        // framework rather than in this plug-in.
        ParamSpec::new("mode_budget", "Modes")
            .range(4.0, bank::MAX_MODES as f32)
            .log()
            .default(d.modes as f32)
            .group("body"),
        ParamSpec::new("select", "Selection")
            .labels(SELECT_NAMES)
            .default(d.order as f32)
            .group("body"),
        ParamSpec::new("ratio", "Ratio")
            .range(0.2, 5.0)
            .log()
            .default(d.aspect)
            .group("body"),
        ParamSpec::new("bar_tuning", "Bar Tuning")
            .labels(BAR_TUNING_NAMES)
            .default(d.bar_tuning as f32)
            .group("body"),
        ParamSpec::new("bar_third", "Third Partial")
            .labels(BAR_THIRD_NAMES)
            .default(d.bar_third as f32)
            .group("body"),
        ParamSpec::new("radius", "Radius")
            .range(1.0, 100.0)
            .log()
            .default(d.radius_mm)
            .unit("mm")
            .group("body"),
        pct("opening", "Opening", d.opening * 100.0, "body"),
        ParamSpec::new("decay", "Decay")
            .range(0.02, 60.0)
            .log()
            .default(d.decay_s)
            .unit("s")
            .group("damping"),
        ParamSpec::new("material", "Material")
            .range(-1.0, 1.0)
            .default(d.material)
            .group("damping"),
        ParamSpec::new("damp_corner", "Damp Corner")
            .range(100.0, 20_000.0)
            .log()
            .default(d.damp_corner_hz)
            .unit("Hz")
            .group("damping"),
        ParamSpec::new("damp_hi", "HF Slope")
            .range(-2.0, 1.0)
            .default(d.damp_hi)
            .group("damping"),
        ParamSpec::new("tail", "Tail")
            .toggle()
            .default(if d.tail { 1.0 } else { 0.0 })
            .group("damping"),
        ParamSpec::new("bright", "Bright")
            .range(-6.0, 6.0)
            .default(d.bright_db_oct)
            .unit("dB/oct")
            .group("tone"),
        ParamSpec::new("inharm", "Inharm")
            .range(-100.0, 100.0)
            .default(d.inharm * 100.0)
            .unit("%")
            .group("tone"),
        pct("hit", "Hit X", d.hit.x * 100.0, "contact"),
        pct("hit_y", "Hit Y", d.hit.y * 100.0, "contact"),
        pct("pos_l", "Pos L X", d.pos_l.x * 100.0, "contact"),
        pct("pos_l_y", "Pos L Y", d.pos_l.y * 100.0, "contact"),
        pct("pos_r", "Pos R X", d.pos_r.x * 100.0, "contact"),
        pct("pos_r_y", "Pos R Y", d.pos_r.y * 100.0, "contact"),
        pct("spread", "Spread", d.spread * 100.0, "contact"),
        pct("width", "Width", d.width * 100.0, "contact"),
        ParamSpec::new("filter_on", "Exciter Filter")
            .toggle()
            .default(if d.filter_on { 1.0 } else { 0.0 })
            .group("exciter"),
        ParamSpec::new("filter_freq", "Filter Freq")
            .range(50.0, 18_000.0)
            .log()
            .default(d.filter_hz)
            .unit("Hz")
            .group("exciter"),
        ParamSpec::new("filter_width", "Filter Width")
            .range(0.5, 9.0)
            .default(d.filter_oct)
            .unit("oct")
            .group("exciter"),
        ParamSpec::new("filter_place", "Filter Place")
            .labels(FILTER_PLACE_NAMES)
            .default(if d.filter_post { 1.0 } else { 0.0 })
            .group("exciter"),
        ParamSpec::new("lfo_on", "LFO")
            .toggle()
            .default(if d.lfo_on { 1.0 } else { 0.0 })
            .group("lfo"),
        ParamSpec::new("lfo_shape", "LFO Shape")
            .labels(LFO_NAMES)
            .default(d.lfo_shape as f32)
            .group("lfo"),
        ParamSpec::new("lfo_rate", "LFO Rate")
            .range(0.01, 20.0)
            .log()
            .default(d.lfo_rate_hz)
            .unit("Hz")
            .group("lfo"),
        ParamSpec::new("lfo_depth", "LFO Depth")
            .range(0.0, 12.0)
            .default(d.lfo_depth_st)
            .unit("st")
            .group("lfo"),
        ParamSpec::new("lfo_phase", "LFO Phase")
            .range(0.0, 360.0)
            .default(d.lfo_phase_deg)
            .unit("\u{b0}")
            .group("lfo"),
        pct("bleed", "Bleed", d.bleed * 100.0, "output"),
        pct("mix", "Dry/Wet", d.mix * 100.0, "output"),
        ParamSpec::new("gain", "Gain")
            .range(-36.0, 36.0)
            .default(d.gain_db)
            .unit("dB")
            .group("output"),
        ParamSpec::new("limiter", "Limiter")
            .toggle()
            .default(if d.limiter { 1.0 } else { 0.0 })
            .group("output"),
        ParamSpec::new("limit_ceil", "Ceiling")
            .range(-24.0, 0.0)
            .default(d.limit_ceil_db)
            .unit("dB")
            .group("output"),
        ParamSpec::new("bypass", "Bypass")
            .toggle()
            .default(0.0)
            .group("output"),
    ];
    if with_source {
        v.push(
            ParamSpec::new("src_kind", "Source")
                .labels(SOURCE_NAMES)
                .default(0.0)
                .not_automatable()
                .group("source"),
        );
        v.push(
            ParamSpec::new("src_level", "Source Level")
                .range(0.0, 1.0)
                .default(0.5)
                .not_automatable()
                .group("source"),
        );
        v.push(
            ParamSpec::new("src_freq", "Source Rate")
                .range(0.5, 20.0)
                .log()
                .default(2.0)
                .unit("Hz")
                .not_automatable()
                .group("source"),
        );
    }
    v
}

/// Which controls each object actually uses.
///
/// **The panel greys out from this rather than deriving it again.** The
/// device this one answers publishes the same information in a shipped,
/// machine-readable bank definition, and where two sources for it disagreed
/// that file was the one to trust; this is ours, and it is the engine's own
/// truth rather than a second guess at it.
pub fn object_meta() -> Value {
    let common = [
        "type",
        "tune",
        "transpose",
        "fine",
        "decay",
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
    ];
    let bank_only = [
        "mode_budget",
        "select",
        "material",
        "damp_corner",
        "damp_hi",
        "tail",
        "bright",
        "inharm",
        "hit",
        "pos_l",
        "pos_r",
    ];
    let mut out = Vec::new();
    for (i, object) in Object::ALL.iter().enumerate() {
        let mut uses: Vec<&str> = common.to_vec();
        let guide = object.engine() == object::Engine::Guide;
        if guide {
            // An air column has no surface to strike, no material and no mode
            // list to truncate; what it has instead is a bore and a far end.
            uses.push("radius");
            uses.push("hit");
            uses.push("pos_l");
            uses.push("pos_r");
            uses.push("bright");
            if *object == Object::Pipe {
                uses.push("opening");
            }
        } else {
            uses.extend_from_slice(&bank_only);
            if object.is_2d() {
                uses.push("hit_y");
                uses.push("pos_l_y");
                uses.push("pos_r_y");
            }
            if object.has_aspect() {
                uses.push("ratio");
            }
            if matches!(object, Object::Marimba) {
                uses.push("bar_tuning");
                uses.push("bar_third");
            }
        }
        uses.sort_unstable();
        // Pipe and Tube are one loop at two settings of one termination, and
        // the meta says so rather than implying two engines. **Both keep their
        // own index**: they are the indices the device this one answers uses,
        // a saved project loads by index, and a user who picks "Tube" has
        // asked for a tube rather than for a pipe they must then open by hand.
        // What the engine does is force the far end open for one of them, and
        // `forces` is that, published so the panel can say it in words.
        let forces = if *object == Object::Tube {
            json!({ "opening": 1.0 })
        } else {
            Value::Null
        };
        let note = match object {
            Object::Tube => {
                "a Pipe with its far end fully open: the same loop, one reflection at its extreme"
            }
            Object::Pipe => {
                "an air column with a variable far end, from fully closed to fully open"
            }
            _ => "",
        };
        out.push(json!({
            "id": i,
            "label": OBJECT_NAMES[i],
            "engine": if guide { "waveguide" } else { "bank" },
            "forces": forces,
            "note": note,
            // What the contact controls mean on this object. A line has one
            // coordinate, a rectangle has two, and a disc has a radius and an
            // angle rather than an x and a y — mapping a square into a circle
            // would put the control's corners on the rim, where a clamped
            // membrane's every mode is zero.
            "coords": if matches!(object, Object::MembraneRound | Object::PlateRound) {
                "polar"
            } else if object.is_2d() {
                "xy"
            } else {
                "line"
            },
            "uses": uses,
        }));
    }
    Value::Array(out)
}

/// Parameter indices, resolved once so the audio thread never looks an id up
/// by string.
#[derive(Clone, Copy, Debug)]
pub struct ParamIx {
    pub object: usize,
    pub tune: usize,
    pub transpose: usize,
    pub fine: usize,
    pub modes: usize,
    pub select: usize,
    pub ratio: usize,
    pub bar_tuning: usize,
    pub bar_third: usize,
    pub radius: usize,
    pub opening: usize,
    pub decay: usize,
    pub material: usize,
    pub damp_corner: usize,
    pub damp_hi: usize,
    pub tail: usize,
    pub bright: usize,
    pub inharm: usize,
    pub hit: usize,
    pub hit_y: usize,
    pub pos_l: usize,
    pub pos_l_y: usize,
    pub pos_r: usize,
    pub pos_r_y: usize,
    pub spread: usize,
    pub width: usize,
    pub filter_on: usize,
    pub filter_freq: usize,
    pub filter_width: usize,
    pub filter_place: usize,
    pub lfo_on: usize,
    pub lfo_shape: usize,
    pub lfo_rate: usize,
    pub lfo_depth: usize,
    pub lfo_phase: usize,
    pub bleed: usize,
    pub mix: usize,
    pub gain: usize,
    pub limiter: usize,
    pub limit_ceil: usize,
    pub bypass: usize,
    pub src_kind: Option<usize>,
    pub src_level: Option<usize>,
    pub src_freq: Option<usize>,
}

/// Resolve the parameter indices by id. Works for the plug-in's mirror, which
/// has no source parameters, as well as for the standalone.
pub fn param_index(s: &NoobVstWebguiFramework) -> ParamIx {
    let ix = |id: &str| s.index_of(id).expect(id);
    ParamIx {
        object: ix("type"),
        tune: ix("tune"),
        transpose: ix("transpose"),
        fine: ix("fine"),
        modes: ix("mode_budget"),
        select: ix("select"),
        ratio: ix("ratio"),
        bar_tuning: ix("bar_tuning"),
        bar_third: ix("bar_third"),
        radius: ix("radius"),
        opening: ix("opening"),
        decay: ix("decay"),
        material: ix("material"),
        damp_corner: ix("damp_corner"),
        damp_hi: ix("damp_hi"),
        tail: ix("tail"),
        bright: ix("bright"),
        inharm: ix("inharm"),
        hit: ix("hit"),
        hit_y: ix("hit_y"),
        pos_l: ix("pos_l"),
        pos_l_y: ix("pos_l_y"),
        pos_r: ix("pos_r"),
        pos_r_y: ix("pos_r_y"),
        spread: ix("spread"),
        width: ix("width"),
        filter_on: ix("filter_on"),
        filter_freq: ix("filter_freq"),
        filter_width: ix("filter_width"),
        filter_place: ix("filter_place"),
        lfo_on: ix("lfo_on"),
        lfo_shape: ix("lfo_shape"),
        lfo_rate: ix("lfo_rate"),
        lfo_depth: ix("lfo_depth"),
        lfo_phase: ix("lfo_phase"),
        bleed: ix("bleed"),
        mix: ix("mix"),
        gain: ix("gain"),
        limiter: ix("limiter"),
        limit_ceil: ix("limit_ceil"),
        bypass: ix("bypass"),
        src_kind: s.index_of("src_kind"),
        src_level: s.index_of("src_level"),
        src_freq: s.index_of("src_freq"),
    }
}

/// One block's worth of parameter values, read from the atomics.
pub fn read_settings(audio: &AudioHandle, ix: &ParamIx) -> Settings {
    let p = |i: usize| audio.param(i);
    let on = |i: usize| audio.param(i) >= 0.5;
    Settings {
        object: p(ix.object)
            .round()
            .clamp(0.0, (OBJECT_NAMES.len() - 1) as f32) as usize,
        tune_hz: p(ix.tune),
        transpose: p(ix.transpose).round(),
        fine_cents: p(ix.fine),
        modes: p(ix.modes).round().clamp(1.0, bank::MAX_MODES as f32) as usize,
        order: p(ix.select).round().clamp(0.0, 2.0) as usize,
        aspect: p(ix.ratio),
        bar_tuning: p(ix.bar_tuning).round().clamp(0.0, 1.0) as usize,
        bar_third: p(ix.bar_third).round().clamp(0.0, 1.0) as usize,
        radius_mm: p(ix.radius),
        opening: p(ix.opening) / 100.0,
        decay_s: p(ix.decay),
        material: p(ix.material),
        damp_corner_hz: p(ix.damp_corner),
        damp_hi: p(ix.damp_hi),
        tail: on(ix.tail),
        bright_db_oct: p(ix.bright),
        inharm: p(ix.inharm) / 100.0,
        hit: Point::new(p(ix.hit) / 100.0, p(ix.hit_y) / 100.0),
        pos_l: Point::new(p(ix.pos_l) / 100.0, p(ix.pos_l_y) / 100.0),
        pos_r: Point::new(p(ix.pos_r) / 100.0, p(ix.pos_r_y) / 100.0),
        spread: p(ix.spread) / 100.0,
        width: p(ix.width) / 100.0,
        filter_on: on(ix.filter_on),
        filter_hz: p(ix.filter_freq),
        filter_oct: p(ix.filter_width),
        filter_post: on(ix.filter_place),
        lfo_on: on(ix.lfo_on),
        lfo_shape: p(ix.lfo_shape).round().clamp(0.0, 6.0) as usize,
        lfo_rate_hz: p(ix.lfo_rate),
        lfo_depth_st: p(ix.lfo_depth),
        lfo_phase_deg: p(ix.lfo_phase),
        bleed: p(ix.bleed) / 100.0,
        mix: p(ix.mix) / 100.0,
        gain_db: p(ix.gain),
        limiter: on(ix.limiter),
        limit_ceil_db: p(ix.limit_ceil),
        bypass: on(ix.bypass),
    }
}

/// The UI store key the mode table lives under.
pub const MODES_KEY: &str = "modes";

/// The UI store key the **user's** presets live under. The factory set is in
/// the bridge metadata instead, because it is generated from the DSP and
/// cannot be edited; see [`preset`].
pub const PRESETS_KEY: &str = "presets";

/// A partial's two indices in one word, so an override's identity is read and
/// written atomically rather than in two halves the audio thread could catch
/// between.
fn pack(i: u16, j: u16) -> u32 {
    ((i as u32) << 16) | j as u32
}

/// The per-partial override table, shared between whichever thread the page
/// writes on and the audio thread.
///
/// Three floats per partial in atomics, plus a generation counter. The audio
/// thread compares the counter once per block and only re-reads the cells
/// when it has moved, so the common case costs one relaxed load and the
/// uncommon one costs 192 — and neither costs a lock or an allocation.
pub struct ModeTable {
    generation: AtomicU32,
    /// Four words per slot: the partial's identity packed as
    /// `(i << 16) | j`, then the three values.
    cells: Vec<AtomicU32>,
}

/// Words per override slot.
const CELLS: usize = 4;

impl Default for ModeTable {
    fn default() -> Self {
        ModeTable::new()
    }
}

impl ModeTable {
    pub fn new() -> ModeTable {
        let d = ModeEdit::default();
        ModeTable {
            generation: AtomicU32::new(0),
            cells: (0..MAX_EDITS * CELLS)
                .map(|i| {
                    AtomicU32::new(match i % CELLS {
                        0 => pack(d.i, d.j),
                        1 => d.cents.to_bits(),
                        2 => d.db.to_bits(),
                        _ => d.decay.to_bits(),
                    })
                })
                .collect(),
        }
    }

    pub fn generation(&self) -> u32 {
        self.generation.load(Ordering::Acquire)
    }

    /// Replace the table from the JSON the page stored.
    ///
    /// **`i` and `j` are the partial's own indices**, as the `modes` stream
    /// publishes them in each frame's first two floats — not a row number. An
    /// entry with no `i` is ignored; `j` defaults to zero, which is what every
    /// one-dimensional object uses.
    ///
    /// Unknown keys, missing fields and absurd values are ignored or clamped
    /// rather than rejected: a page from a future version must not be able to
    /// silence the plug-in.
    pub fn load_json(&self, v: &Value) {
        let mut edits = [ModeEdit::default(); MAX_EDITS];
        let mut n = 0usize;
        if let Some(list) = v.get("edits").and_then(|e| e.as_array()) {
            for e in list {
                if n >= MAX_EDITS {
                    break;
                }
                let Some(i) = e.get("i").and_then(|x| x.as_i64()) else {
                    continue;
                };
                if !(0..ModeEdit::NONE as i64).contains(&i) {
                    continue;
                }
                let j = e.get("j").and_then(|x| x.as_i64()).unwrap_or(0);
                if !(0..u16::MAX as i64).contains(&j) {
                    continue;
                }
                let slot = &mut edits[n];
                slot.i = i as u16;
                slot.j = j as u16;
                if let Some(c) = e.get("cents").and_then(|x| x.as_f64()) {
                    // Two octaves either way. A mode table exists to build
                    // objects nobody shipped, and an octave is not enough
                    // room for that: putting a string's third partial onto a
                    // bell's tierce is 1,586 cents down on its own.
                    slot.cents = (c as f32).clamp(-2400.0, 2400.0);
                }
                if let Some(g) = e.get("db").and_then(|x| x.as_f64()) {
                    slot.db = (g as f32).clamp(-60.0, 60.0);
                }
                if let Some(t) = e.get("decay").and_then(|x| x.as_f64()) {
                    slot.decay = (t as f32).clamp(0.1, 10.0);
                }
                n += 1;
            }
        }
        self.store(&edits);
    }

    /// The table as the page's store holds it, sparse: unused slots are left
    /// out.
    pub fn to_json(&self) -> Value {
        let mut edits = [ModeEdit::default(); MAX_EDITS];
        self.read(&mut edits);
        let list: Vec<Value> = edits
            .iter()
            .filter(|e| e.is_set())
            .map(|e| {
                json!({
                    "i": e.i,
                    "j": e.j,
                    "cents": e.cents,
                    "db": e.db,
                    "decay": e.decay
                })
            })
            .collect();
        json!({ "edits": list })
    }

    pub fn store(&self, edits: &[ModeEdit; MAX_EDITS]) {
        for (k, e) in edits.iter().enumerate() {
            self.cells[k * CELLS].store(pack(e.i, e.j), Ordering::Relaxed);
            self.cells[k * CELLS + 1].store(e.cents.to_bits(), Ordering::Relaxed);
            self.cells[k * CELLS + 2].store(e.db.to_bits(), Ordering::Relaxed);
            self.cells[k * CELLS + 3].store(e.decay.to_bits(), Ordering::Relaxed);
        }
        self.generation.fetch_add(1, Ordering::Release);
    }

    pub fn read(&self, out: &mut [ModeEdit; MAX_EDITS]) {
        for (k, e) in out.iter_mut().enumerate() {
            let id = self.cells[k * CELLS].load(Ordering::Relaxed);
            e.i = (id >> 16) as u16;
            e.j = (id & 0xFFFF) as u16;
            e.cents = f32::from_bits(self.cells[k * CELLS + 1].load(Ordering::Relaxed));
            e.db = f32::from_bits(self.cells[k * CELLS + 2].load(Ordering::Relaxed));
            e.decay = f32::from_bits(self.cells[k * CELLS + 3].load(Ordering::Relaxed));
        }
    }
}

/// Build the standalone's bridge and resolve its parameter indices.
pub fn build_bridge(name: &str, sr: f32) -> (NoobVstWebguiFramework, ParamIx) {
    let mut b = NoobVstWebguiFramework::builder(name)
        .meta(bridge_meta(sr, true))
        .params(param_specs(true));
    for s in streams(sr) {
        b = b.stream(s);
    }
    let s = b.build();
    let ix = param_index(&s);
    (s, ix)
}

/// The bridge metadata the page reads its layout from.
pub fn bridge_meta(sr: f32, standalone: bool) -> Value {
    json!({
        "vendor": "Noob Audio Engineering",
        "version": env!("CARGO_PKG_VERSION"),
        "sample_rate": sr,
        "standalone": standalone,
        "max_partials": MAX_EDITS,
        "max_modes": bank::MAX_MODES,
        "response_points": RESPONSE_POINTS,
        "spread_max_cents": SPREAD_MAX_CENTS,
        "c_air": guide::C_AIR,
        "modes_key": MODES_KEY,
        "objects": object_meta(),
        "presets_key": PRESETS_KEY,
        "preset_version": preset::PRESET_VERSION,
        "presets": preset::factory_json(),
    })
}

/// Attach a mode table to a bridge, so that every page write reaches the
/// audio thread and whatever the host restored is applied before the first
/// block.
pub fn attach_mode_table(bridge: &NoobVstWebguiFramework, table: Arc<ModeTable>) {
    if let Some(v) = bridge.store_get(MODES_KEY) {
        table.load_json(&v);
    }
    let t = table.clone();
    bridge.set_store_hook(Some(Arc::new(move |key: &str, value: &Value| {
        if key == MODES_KEY {
            t.load_json(value);
        }
    })));
}

/// The engine plus the block-rate telemetry. The plug-in and the standalone
/// drive it the same way: [`configure`](Processor::configure) with a fresh
/// snapshot, [`process`](Processor::process) the block,
/// [`publish`](Processor::publish) the streams.
pub struct Processor {
    engine: Resonator,
    table: Arc<ModeTable>,
    table_gen: u32,
    edits: [ModeEdit; MAX_EDITS],
    blocks: u64,
    last_modes: Vec<f32>,
    last_response: Vec<f32>,
}

impl Processor {
    pub fn new(sr: f32) -> Processor {
        Processor::with_table(sr, Arc::new(ModeTable::new()))
    }

    pub fn with_table(sr: f32, table: Arc<ModeTable>) -> Processor {
        Processor {
            engine: Resonator::new(sr),
            table,
            table_gen: u32::MAX,
            edits: [ModeEdit::default(); MAX_EDITS],
            blocks: 0,
            last_modes: vec![f32::NAN; MAX_EDITS * MODE_FIELDS],
            last_response: vec![f32::NAN; RESPONSE_POINTS],
        }
    }

    pub fn table(&self) -> &Arc<ModeTable> {
        &self.table
    }

    pub fn engine(&self) -> &Resonator {
        &self.engine
    }

    pub fn set_sample_rate(&mut self, sr: f32) {
        self.engine.set_sample_rate(sr);
    }

    pub fn reset(&mut self) {
        self.engine.reset();
    }

    pub fn latency(&self) -> usize {
        self.engine.latency()
    }

    pub fn configure(&mut self, s: &Settings) {
        let g = self.table.generation();
        if g != self.table_gen {
            self.table_gen = g;
            self.table.read(&mut self.edits);
            self.engine.set_edits(&self.edits);
        }
        self.engine.configure(s);
    }

    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        self.engine.process(l, r);
    }

    /// Publish the streams after [`process`](Processor::process). Real-time
    /// safe.
    ///
    /// The two sticky streams are only sent when what they hold has actually
    /// changed, and then only on every fourth block, so a swept knob does not
    /// flood the wire.
    pub fn publish(&mut self, audio: &mut AudioHandle) {
        audio.publish_slice(STREAM_IX.meter, &self.engine.meter());
        audio.publish_slice(STREAM_IX.info, &self.engine.info_frame());
        self.blocks += 1;
        if !self.blocks.is_multiple_of(4) {
            return;
        }
        if self.last_modes != self.engine.modes_frame() {
            self.last_modes.copy_from_slice(self.engine.modes_frame());
            audio.publish_slice(STREAM_IX.modes, &self.last_modes);
        }
        if self.last_response != self.engine.response_curve() {
            self.last_response
                .copy_from_slice(self.engine.response_curve());
            audio.publish_slice(STREAM_IX.response, &self.last_response);
        }
    }
}

#[cfg(test)]
mod tests;
