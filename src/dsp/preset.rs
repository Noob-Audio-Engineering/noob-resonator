//! The factory presets, and the shape every preset has.
//!
//! ## Why these live in the engine
//!
//! A preset here is physics: an object, a damping law, a bore, a contact
//! point. Written as a [`Settings`] struct it cannot fall outside a
//! parameter's range or contradict the object it names, because it is the
//! same type the audio thread runs on. Written as JSON on the page it could
//! do both, and nothing would notice until somebody loaded it.
//!
//! So the **factory** set is generated from here into the bridge metadata,
//! and the page reads it like any other metadata. **User** presets are the
//! page's, in the interface store, in the same shape. Applying is the page's
//! either way, and that is not a division of labour but a fact about hosts: a
//! parameter change has to travel through the host to be recorded and
//! undoable, and only the editor can do that.
//!
//! ## The shape
//!
//! ```json
//! {
//!   "v": 1,
//!   "name": "Struck Slate",
//!   "group": "Plate",
//!   "description": "one line, what it is",
//!   "values": { "type": 4, "tune": 110.0, "decay": 3.2 },
//!   "modes": [ { "i": 3, "j": 1, "cents": -14.0, "db": -2.0, "decay": 1.4 } ]
//! }
//! ```
//!
//! **`values` is keyed by parameter id and carries plain units** — the ones
//! the manifest reports — never an index and never a normalised number. An
//! index breaks the moment a parameter is appended, which is the failure this
//! project has caught more times than any other.
//!
//! **Every parameter is set on load**: to `values[id]` where the preset names
//! it, and otherwise to that parameter's own default. So a preset fully
//! determines the sound and cannot leave a stray control behind from whatever
//! was loaded before it. Ids the engine does not know are ignored rather than
//! refused, so a preset saved by a later version still loads what it can.
//!
//! **`bypass` is never in a preset.** It is a transport control rather than a
//! sound, and a preset that silently bypasses the plug-in is a support
//! ticket.
//!
//! ## The mode table travels, and it always replaces
//!
//! `modes` is **mandatory**, and an empty array means "this preset has no
//! overrides" — which *clears* whatever the user had. One rule, no ambiguity:
//! after loading a preset the mode table is exactly what the preset says.
//!
//! Carrying it rather than excluding it is deliberate. The per-partial table
//! is the thing this architecture can do that no other can, so a preset
//! system that could not capture it would leave the one distinctive feature
//! unsaveable — and `Hand Bell` below exists to prove the point, since it is
//! a plain string with five partials moved onto a bell's own series.
//!
//! ## Two field names, and why they are these ones
//!
//! `description` and `modes` rather than `notes` and `edits`. Both halves of
//! the contract were written to these independently, which is weak evidence
//! for them and no evidence at all about the alternative, so here is the
//! actual argument. **`notes` is a collision in this plug-in specifically**:
//! it is a resonator, a note is the thing you play it with, and a field of
//! that name beside `tune` and `transpose` will be read as one at least once.
//! And `modes` is the name this table already has everywhere else it appears
//! — the store key, the stream, the mode editor — where `edits` would be a
//! fourth name for the same object. The shapes are otherwise identical, so
//! either set works; these two are the ones that do not have to be explained.
//!
//! ## Room for a table that was generated rather than chosen
//!
//! A chord generator — a root and a set of intervals filling the same bank
//! that an object's series fills — would be a different way to answer "what
//! is it tuned to" rather than "what is it made of", and the format already
//! carries one without a version bump.
//!
//! Its controls would be parameters like any other, so they travel in
//! `values` by id, and unknown ids are ignored on load, which is what lets an
//! older build open a preset that names them. The **result** travels in
//! `modes`, because the table is already the thing a generator writes into.
//! So a preset can carry the recipe, or the result, or both, and a build that
//! has the generator reproduces the first while a build that does not still
//! loads the second.
//!
//! **The one rule that has to survive** is that a generator writes the table
//! and then leaves: generate *then* edit, never generate *instead of* edit. A
//! preset that carried only a chord and no table would quietly make the
//! per-partial editor unreachable for anything that used it, which would
//! trade the feature this whole architecture exists for against a menu.

use serde_json::{Map, Value, json};

use crate::dsp::engine::{ModeEdit, Settings};
use crate::dsp::object::{OBJECT_NAMES, Object};

/// The format version. One integer now, a migration avoided later.
pub const PRESET_VERSION: u32 = 1;

/// One preset.
pub struct Preset {
    pub name: &'static str,
    /// What the browser groups by; the object's own name.
    pub group: &'static str,
    pub description: &'static str,
    pub settings: Settings,
    /// The per-partial overrides, which always replace on load. Empty means
    /// "no overrides", not "leave the user's alone".
    pub modes: Vec<ModeEdit>,
}

impl Preset {
    pub fn to_json(&self) -> Value {
        json!({
            "v": PRESET_VERSION,
            "name": self.name,
            "group": self.group,
            "description": self.description,
            "values": Value::Object(settings_values(&self.settings)),
            "modes": self.modes.iter().map(|e| json!({
                "i": e.i,
                "j": e.j,
                "cents": e.cents,
                "db": e.db,
                "decay": e.decay,
            })).collect::<Vec<_>>(),
        })
    }
}

/// A settings snapshot as `{ parameter id: plain value }`.
///
/// The inverse of [`crate::dsp::read_settings`], and the two have to agree:
/// `tests.rs` builds a bridge, writes one of these into it parameter by
/// parameter, reads it back through the real path the page uses, and asserts
/// it comes out the same. A field added to `Settings` and forgotten here
/// fails that test rather than silently dropping out of every preset.
pub fn settings_values(s: &Settings) -> Map<String, Value> {
    let mut m = Map::new();
    let mut put = |id: &str, v: f32| {
        m.insert(id.to_string(), json!(v));
    };
    put("type", s.object as f32);
    put("tune", s.tune_hz);
    put("transpose", s.transpose);
    put("fine", s.fine_cents);
    put("mode_budget", s.modes as f32);
    put("select", s.order as f32);
    put("ratio", s.aspect);
    put("bar_tuning", s.bar_tuning as f32);
    put("bar_third", s.bar_third as f32);
    put("voices", s.voices as f32);
    put("midi_voices", if s.midi_voices { 1.0 } else { 0.0 });
    for (k, semis) in s.voice_semis.iter().enumerate() {
        put(&format!("voice{}", k + 1), *semis);
    }
    put("radius", s.radius_mm);
    put("opening", s.opening * 100.0);
    put("disperse", s.disperse * 100.0);
    put("decay", s.decay_s);
    put("material", s.material);
    put("damp_corner", s.damp_corner_hz);
    put("damp_hi", s.damp_hi);
    put("tail", if s.tail { 1.0 } else { 0.0 });
    put("bright", s.bright_db_oct);
    put("inharm", s.inharm * 100.0);
    put("hit", s.hit.x * 100.0);
    put("hit_y", s.hit.y * 100.0);
    put("pos_l", s.pos_l.x * 100.0);
    put("pos_l_y", s.pos_l.y * 100.0);
    put("pos_r", s.pos_r.x * 100.0);
    put("pos_r_y", s.pos_r.y * 100.0);
    put("spread", s.spread * 100.0);
    put("width", s.width * 100.0);
    put("filter_on", if s.filter_on { 1.0 } else { 0.0 });
    put("filter_freq", s.filter_hz);
    put("filter_width", s.filter_oct);
    put("filter_place", if s.filter_post { 1.0 } else { 0.0 });
    put("lfo_on", if s.lfo_on { 1.0 } else { 0.0 });
    put("lfo_shape", s.lfo_shape as f32);
    put("lfo_rate", s.lfo_rate_hz);
    put("lfo_depth", s.lfo_depth_st);
    put("lfo_phase", s.lfo_phase_deg);
    put("bleed", s.bleed * 100.0);
    put("mix", s.mix * 100.0);
    put("gain", s.gain_db);
    put("limiter", if s.limiter { 1.0 } else { 0.0 });
    put("limit_ceil", s.limit_ceil_db);
    // `bypass` is deliberately absent; see the module docs.
    m
}

/// A helper so a preset reads as the handful of things it changes.
fn on(object: Object) -> Settings {
    Settings {
        object: Object::ALL.iter().position(|o| *o == object).unwrap_or(0),
        ..Settings::default()
    }
}

fn edit(i: u16, j: u16, cents: f32) -> ModeEdit {
    ModeEdit {
        i,
        j,
        cents,
        db: 0.0,
        decay: 1.0,
    }
}

/// The factory set.
///
/// A handful per object rather than a long list, because a preset browser is
/// read and a long one is scrolled past. Every one of them is a starting
/// point that says something about the object it is on.
pub fn factory() -> Vec<Preset> {
    let g = |o: Object| OBJECT_NAMES[Object::ALL.iter().position(|x| *x == o).unwrap_or(0)];
    vec![
        // -- Beam ---------------------------------------------------------
        Preset {
            name: "Glockenspiel",
            group: g(Object::Beam),
            description: "A free bar high enough that its 2.76 overtone lands where the clang lives.",
            settings: Settings {
                tune_hz: 880.0,
                decay_s: 1.4,
                material: -0.35,
                bright_db_oct: -2.0,
                hit: crate::dsp::Point::new(0.12, 0.0),
                pos_l: crate::dsp::Point::new(0.22, 0.0),
                pos_r: crate::dsp::Point::new(0.78, 0.0),
                ..on(Object::Beam)
            },
            modes: vec![],
        },
        Preset {
            name: "Sub Bar",
            group: g(Object::Beam),
            description: "The same bar four octaves down, where its 28 partials are all you get.",
            settings: Settings {
                tune_hz: 41.0,
                decay_s: 8.0,
                material: -0.2,
                bright_db_oct: -1.0,
                ..on(Object::Beam)
            },
            modes: vec![],
        },
        // -- Marimba ------------------------------------------------------
        Preset {
            name: "Rosewood Marimba",
            group: g(Object::Marimba),
            description: "Arch-cut to two octaves, with Woodhouse's 9.2 on the second overtone.",
            settings: Settings {
                tune_hz: 220.0,
                decay_s: 1.1,
                material: -0.7,
                bright_db_oct: -4.0,
                ..on(Object::Marimba)
            },
            modes: vec![],
        },
        Preset {
            name: "Xylophone",
            group: g(Object::Marimba),
            description: "A shallower arch: the first overtone at a twelfth rather than two octaves.",
            settings: Settings {
                tune_hz: 523.0,
                decay_s: 0.7,
                material: -0.6,
                bar_tuning: 1,
                ..on(Object::Marimba)
            },
            modes: vec![],
        },
        // -- String -------------------------------------------------------
        Preset {
            name: "Piano Wire",
            group: g(Object::String),
            description: "Real wire is stiff, and stiffness pulls a string's partials sharp as n·sqrt(1 + Bn²). This is B = 3×10⁻⁴, the figure Lehtonen and colleagues measured on a piano C4: the sixteenth partial comes out 64 cents sharp, which is most of a semitone, and it is why a piano is tuned with stretched octaves.",
            settings: Settings {
                tune_hz: 131.0,
                decay_s: 6.0,
                material: -0.6,
                inharm: 0.31623,
                bright_db_oct: -3.5,
                ..on(Object::String)
            },
            modes: vec![],
        },
        Preset {
            name: "Harp Wire",
            group: g(Object::String),
            description: "The same string with the stiffness taken back out, one control apart: a harmonic series exactly, no stretch, and nothing to tune around. That is the whole difference between the two instruments.",
            settings: Settings {
                tune_hz: 131.0,
                decay_s: 6.0,
                material: -0.6,
                inharm: 0.0,
                bright_db_oct: -3.5,
                ..on(Object::String)
            },
            modes: vec![],
        },
        // The strike comb. A pair, because it is a null, and a null is only
        // audible against the thing it takes away.
        Preset {
            name: "Hammer at the Middle",
            group: g(Object::String),
            description: "A contact cannot excite a partial that has a node under it. At the middle of a string that is every even partial, so what is left is the odd half of the series and a hollow, stopped sort of tone.",
            settings: Settings {
                tune_hz: 147.0,
                decay_s: 4.0,
                material: -0.5,
                bright_db_oct: -3.0,
                hit: crate::dsp::Point::new(0.5, 0.0),
                pos_l: crate::dsp::Point::new(0.12, 0.0),
                pos_r: crate::dsp::Point::new(0.31, 0.0),
                ..on(Object::String)
            },
            modes: vec![],
        },
        Preset {
            name: "Hammer at a Seventh",
            group: g(Object::String),
            description: "The same string with the strike moved to a seventh of its length, one control apart. That puts a node under the seventh partial and its multiples and under nothing else, which is the reason a piano's hammer lands about there.",
            settings: Settings {
                tune_hz: 147.0,
                decay_s: 4.0,
                material: -0.5,
                bright_db_oct: -3.0,
                hit: crate::dsp::Point::new(1.0 / 7.0, 0.0),
                pos_l: crate::dsp::Point::new(0.12, 0.0),
                pos_r: crate::dsp::Point::new(0.31, 0.0),
                ..on(Object::String)
            },
            modes: vec![],
        },
        Preset {
            name: "Nylon",
            group: g(Object::String),
            description: "Soft and quick: the highs leave first, which is what makes it sound like gut.",
            settings: Settings {
                tune_hz: 196.0,
                decay_s: 1.6,
                material: -0.9,
                bright_db_oct: -5.0,
                ..on(Object::String)
            },
            modes: vec![],
        },
        Preset {
            name: "Hand Bell",
            group: g(Object::String),
            description: "A plain string with five partials moved by hand onto a minor-third bell's own series — hum, prime, tierce, quint, nominal. Nothing but the mode table does this.",
            settings: Settings {
                tune_hz: 262.0,
                decay_s: 9.0,
                material: -0.3,
                modes: 24,
                bright_db_oct: -4.0,
                ..on(Object::String)
            },
            // Partial n sits at n; these move it to the bell's ratio.
            //   hum 0.5, prime 1.0, tierce 1.2, quint 1.5, nominal 2.0
            modes: vec![
                edit(1, 0, cents_to(1.0, 0.5)),
                edit(2, 0, cents_to(2.0, 1.0)),
                edit(3, 0, cents_to(3.0, 1.2)),
                edit(4, 0, cents_to(4.0, 1.5)),
                edit(5, 0, cents_to(5.0, 2.0)),
            ],
        },
        // -- Membrane -----------------------------------------------------
        Preset {
            name: "Kick Head",
            group: g(Object::Membrane),
            description: "Low, damped and struck off centre, which is where a drum's body comes from.",
            settings: Settings {
                tune_hz: 55.0,
                decay_s: 0.5,
                material: -0.8,
                aspect: 1.0,
                hit: crate::dsp::Point::new(0.32, 0.28),
                ..on(Object::Membrane)
            },
            modes: vec![],
        },
        Preset {
            name: "Snare Head",
            group: g(Object::Membrane),
            description: "An oblong head, so the degenerate pairs split and the rattle has two pitches.",
            settings: Settings {
                tune_hz: 180.0,
                decay_s: 0.35,
                material: -0.5,
                aspect: 1.41,
                bright_db_oct: -1.5,
                ..on(Object::Membrane)
            },
            modes: vec![],
        },
        // The pair. Same object, same budget, one control apart.
        Preset {
            name: "A · Loudest Partials",
            group: g(Object::Membrane),
            description: "Compare with B. The same membrane and the same 128 modes, spent on the partials that carry the most energy.",
            settings: Settings {
                tune_hz: 110.0,
                decay_s: 3.0,
                modes: 128,
                order: 0,
                tail: false,
                ..on(Object::Membrane)
            },
            modes: vec![],
        },
        Preset {
            name: "B · Lowest Partials",
            group: g(Object::Membrane),
            description: "Compare with A. Identical but for one control: the same 128 modes spent on the lowest partials instead, which is where the object goes deaf.",
            settings: Settings {
                tune_hz: 110.0,
                decay_s: 3.0,
                modes: 128,
                order: 1,
                tail: false,
                ..on(Object::Membrane)
            },
            modes: vec![],
        },
        // The other pair, and the one that says why the default tilt is not
        // zero. Measured on this engine at these exact settings.
        Preset {
            name: "Sloped Strike",
            group: g(Object::Membrane),
            description: "A struck object radiates less at the top of its range than at the bottom, and this is the device's own default slope of -3 dB per octave. With it, the 512 modes land across the band: 286 of them between 1.5 and 10 kHz, and none above.",
            settings: Settings {
                tune_hz: 110.0,
                decay_s: 2.5,
                modes: 512,
                order: 0,
                tail: false,
                bright_db_oct: -3.0,
                ..on(Object::Membrane)
            },
            modes: vec![],
        },
        Preset {
            name: "Flat Strike",
            group: g(Object::Membrane),
            description: "One control apart, and it is the trap inside the whole idea of keeping the loudest partials. A mass-normalised mode set has no amplitude trend at all, so with the excitation flat there is nothing left for \"loudest\" to prefer, and the denser high octaves take the entire budget: none of the 512 between 1.5 and 10 kHz, and 292 above 10 kHz.",
            settings: Settings {
                tune_hz: 110.0,
                decay_s: 2.5,
                modes: 512,
                order: 0,
                tail: false,
                bright_db_oct: 0.0,
                ..on(Object::Membrane)
            },
            modes: vec![],
        },
        // -- Plate --------------------------------------------------------
        Preset {
            name: "Struck Slate",
            group: g(Object::Plate),
            description: "Short and dense: a plate's partials are evenly spread, so it reads as a texture.",
            settings: Settings {
                tune_hz: 140.0,
                decay_s: 1.2,
                material: -0.55,
                aspect: 1.6,
                ..on(Object::Plate)
            },
            modes: vec![],
        },
        Preset {
            name: "Plate Reverb",
            group: g(Object::Plate),
            description: "The same object held open, which is what a plate reverb always was.",
            settings: Settings {
                tune_hz: 62.0,
                decay_s: 12.0,
                material: -0.25,
                aspect: 2.2,
                spread: 0.4,
                width: 1.0,
                ..on(Object::Plate)
            },
            modes: vec![],
        },
        // The damping law, which is one number and a slope.
        Preset {
            name: "Wood",
            group: g(Object::Plate),
            description: "Material is the exponent in T60(f) = T60(f1)·(f/f1)^m, which Applied Acoustics publish quantitatively. At -1 a partial an octave up rings for half as long, so the top of the spectrum has gone before the bottom has begun to fade.",
            settings: Settings {
                tune_hz: 180.0,
                decay_s: 3.0,
                material: -1.0,
                aspect: 1.4,
                ..on(Object::Plate)
            },
            modes: vec![],
        },
        Preset {
            name: "Bronze",
            group: g(Object::Plate),
            description: "The same plate with that exponent at +1 instead, one control apart, where the octave up rings twice as long rather than half. Nothing else moved: the difference between a wood block and a bell is a single slope.",
            settings: Settings {
                tune_hz: 180.0,
                decay_s: 3.0,
                material: 1.0,
                aspect: 1.4,
                ..on(Object::Plate)
            },
            modes: vec![],
        },
        // -- Pipe ---------------------------------------------------------
        Preset {
            name: "Half Open",
            group: g(Object::Pipe),
            description: "The far end part way open, where the even partials are fading in out of nothing.",
            settings: Settings {
                tune_hz: 165.0,
                decay_s: 3.0,
                radius_mm: 30.0,
                opening: 0.45,
                ..on(Object::Pipe)
            },
            modes: vec![],
        },
        // The far end, which is the entire difference between the two air
        // columns and is worth meeting as one control rather than two
        // objects. Both lengths are the model's own, read off the info frame.
        Preset {
            name: "Stopped Pipe",
            group: g(Object::Pipe),
            description: "What happens at the far end is the whole of it. Closed, a pressure wave comes back with its sign intact, only the odd harmonics survive, and this note comes out of 0.57 m of air.",
            settings: Settings {
                tune_hz: 147.0,
                decay_s: 2.5,
                radius_mm: 22.0,
                opening: 0.0,
                ..on(Object::Pipe)
            },
            modes: vec![],
        },
        Preset {
            name: "Open Pipe",
            group: g(Object::Pipe),
            description: "One control apart: open, the wave comes back inverted, every harmonic survives, and the same note now needs 1.14 m — exactly twice the column, which is why a stopped organ pipe is half the size of an open one at the same pitch.",
            settings: Settings {
                tune_hz: 147.0,
                decay_s: 2.5,
                radius_mm: 22.0,
                opening: 1.0,
                ..on(Object::Pipe)
            },
            modes: vec![],
        },
        // -- Tube ---------------------------------------------------------
        Preset {
            name: "Scaffold Tube",
            group: g(Object::Tube),
            description: "Open at both ends, narrow bore: all harmonics and a short, bright ring.",
            settings: Settings {
                tune_hz: 330.0,
                decay_s: 1.8,
                radius_mm: 9.0,
                ..on(Object::Tube)
            },
            modes: vec![],
        },
        Preset {
            name: "Drain Pipe",
            group: g(Object::Tube),
            description: "A fat bore, which loses less at the wall and so rings longer and keeps its highs.",
            settings: Settings {
                tune_hz: 73.0,
                decay_s: 3.5,
                radius_mm: 80.0,
                ..on(Object::Tube)
            },
            modes: vec![],
        },
        // -- Membrane Round -----------------------------------------------
        Preset {
            name: "Timpani",
            group: g(Object::MembraneRound),
            description: "A disc struck a quarter of the way in, which is where a timpanist strikes and why it has a pitch. Above 1,040 Hz its partials sit closer together than their own bandwidths and stop being separately audible, and a feedback delay network carries the sound from there up, at -26.6 dB.",
            settings: Settings {
                tune_hz: 73.0,
                decay_s: 3.0,
                material: -0.6,
                tail: true,
                hit: crate::dsp::Point::new(0.62, 0.1),
                pos_l: crate::dsp::Point::new(0.4, 0.05),
                pos_r: crate::dsp::Point::new(0.4, 0.55),
                ..on(Object::MembraneRound)
            },
            modes: vec![],
        },
        Preset {
            name: "Timpani, Bank Only",
            group: g(Object::MembraneRound),
            description: "The same head one control apart, with the tail off. The modes below 1,040 Hz are unchanged and exact — that is not what the tail is for — and above it the object simply stops, which is what a truncated bank does when nothing carries the rest.",
            settings: Settings {
                tune_hz: 73.0,
                decay_s: 3.0,
                material: -0.6,
                tail: false,
                hit: crate::dsp::Point::new(0.62, 0.1),
                pos_l: crate::dsp::Point::new(0.4, 0.05),
                pos_r: crate::dsp::Point::new(0.4, 0.55),
                ..on(Object::MembraneRound)
            },
            modes: vec![],
        },
        Preset {
            name: "Tabla",
            group: g(Object::MembraneRound),
            description: "Struck close to the rim, which starves the circular modes and leaves the diameters.",
            settings: Settings {
                tune_hz: 210.0,
                decay_s: 1.1,
                material: -0.7,
                hit: crate::dsp::Point::new(0.88, 0.0),
                pos_l: crate::dsp::Point::new(0.55, 0.2),
                pos_r: crate::dsp::Point::new(0.55, 0.7),
                ..on(Object::MembraneRound)
            },
            modes: vec![],
        },
        // -- Tine ---------------------------------------------------------
        Preset {
            name: "Electric Piano",
            group: g(Object::Tine),
            description: "A clamped bar: its first overtone is at 6.27, so there is nothing in the range where a bar clangs.",
            settings: Settings {
                tune_hz: 262.0,
                decay_s: 3.2,
                material: -0.55,
                bright_db_oct: -4.5,
                hit: crate::dsp::Point::new(0.88, 0.0),
                pos_l: crate::dsp::Point::new(0.95, 0.0),
                pos_r: crate::dsp::Point::new(0.72, 0.0),
                ..on(Object::Tine)
            },
            modes: vec![],
        },
        Preset {
            name: "Music Box",
            group: g(Object::Tine),
            description: "The same tooth, higher and shorter, plucked near its tip.",
            settings: Settings {
                tune_hz: 1047.0,
                decay_s: 1.0,
                material: -0.5,
                bright_db_oct: -2.0,
                hit: crate::dsp::Point::new(0.95, 0.0),
                pos_l: crate::dsp::Point::new(0.9, 0.0),
                pos_r: crate::dsp::Point::new(0.8, 0.0),
                ..on(Object::Tine)
            },
            modes: vec![],
        },
        Preset {
            name: "Tuning Fork",
            group: g(Object::Tine),
            description: "Held open with the tilt down, so almost nothing but the fundamental survives.",
            settings: Settings {
                tune_hz: 440.0,
                decay_s: 30.0,
                material: -0.1,
                bright_db_oct: -6.0,
                modes: 8,
                ..on(Object::Tine)
            },
            modes: vec![],
        },
        // -- Plate Round --------------------------------------------------
        Preset {
            name: "Crash Cymbal",
            group: g(Object::PlateRound),
            description: "A clamped disc in flexure: its partials spread as they rise, which is a wash rather than a pitch.",
            settings: Settings {
                tune_hz: 180.0,
                decay_s: 4.5,
                material: -0.35,
                bright_db_oct: -1.5,
                hit: crate::dsp::Point::new(0.85, 0.0),
                pos_l: crate::dsp::Point::new(0.5, 0.15),
                pos_r: crate::dsp::Point::new(0.5, 0.6),
                ..on(Object::PlateRound)
            },
            modes: vec![],
        },
        Preset {
            name: "Temple Gong",
            group: g(Object::PlateRound),
            description: "The same disc struck at its centre, which excites only the modes with no nodal diameter.",
            settings: Settings {
                tune_hz: 58.0,
                decay_s: 14.0,
                material: -0.25,
                bright_db_oct: -3.0,
                hit: crate::dsp::Point::new(0.02, 0.0),
                pos_l: crate::dsp::Point::new(0.45, 0.1),
                pos_r: crate::dsp::Point::new(0.7, 0.55),
                ..on(Object::PlateRound)
            },
            modes: vec![],
        },
        // -- Voiced -----------------------------------------------------------
        //
        // **A chord is not an object here; it is a set of roots, and each
        // root gets the object's own series.** That is the thing worth having
        // and the reason a tenth object would have been the weaker feature: a
        // chord of tuned strings is six harmonic ladders, where a chord of
        // beams is six copies of a real bar's inharmonic series, which is not
        // a sound anything else makes.
        Preset {
            name: "Struck Triad",
            group: g(Object::String),
            description: "Three strings a fifth and a major tenth apart, spread rather than closed so each voice keeps its own register instead of beating against the others.",
            settings: Settings {
                tune_hz: 110.0,
                decay_s: 4.0,
                voices: 3,
                voice_semis: [0.0, 7.0, 16.0, 12.0, 19.0, 24.0],
                material: -0.5,
                bright_db_oct: -3.0,
                hit: crate::dsp::Point::new(0.14, 0.0),
                pos_l: crate::dsp::Point::new(0.24, 0.0),
                pos_r: crate::dsp::Point::new(0.68, 0.0),
                ..on(Object::String)
            },
            modes: vec![],
        },
        Preset {
            name: "Struck Six",
            group: g(Object::String),
            description: "The same six pitches with all of them sounding instead of three, one control apart. The extra voices were already tuned; what changes is how much of the chord is there.",
            settings: Settings {
                tune_hz: 110.0,
                decay_s: 4.0,
                voices: 6,
                voice_semis: [0.0, 7.0, 16.0, 12.0, 19.0, 24.0],
                material: -0.5,
                bright_db_oct: -3.0,
                hit: crate::dsp::Point::new(0.14, 0.0),
                pos_l: crate::dsp::Point::new(0.24, 0.0),
                pos_r: crate::dsp::Point::new(0.68, 0.0),
                ..on(Object::String)
            },
            modes: vec![],
        },
        Preset {
            name: "Bell Chord",
            group: g(Object::Beam),
            description: "Four free bars tuned to a minor ninth. A bar's overtone sits at 2.76 rather than 2, so four of them are not four chords stacked but sixteen partials that belong to no key at all — which is what a chord of bars is for.",
            settings: Settings {
                tune_hz: 220.0,
                decay_s: 5.0,
                voices: 4,
                voice_semis: [0.0, 7.0, 15.0, 26.0, 19.0, 24.0],
                material: -0.4,
                bright_db_oct: -2.5,
                hit: crate::dsp::Point::new(0.12, 0.0),
                pos_l: crate::dsp::Point::new(0.22, 0.0),
                pos_r: crate::dsp::Point::new(0.78, 0.0),
                ..on(Object::Beam)
            },
            modes: vec![],
        },
        Preset {
            name: "Organ Stop",
            group: g(Object::Pipe),
            description: "A stopped rank at fifths and octaves: odd harmonics from each of six columns, which is how a mixture stop is built and why it colours a source without asserting a key.",
            settings: Settings {
                tune_hz: 110.0,
                decay_s: 3.0,
                radius_mm: 26.0,
                opening: 0.0,
                voices: 6,
                voice_semis: [0.0, 7.0, 12.0, 19.0, 24.0, 31.0],
                bright_db_oct: -3.0,
                ..on(Object::Pipe)
            },
            modes: vec![],
        },
        Preset {
            name: "Tine Ninth",
            group: g(Object::Tine),
            description: "Five clamped tines, struck a ninth of the way along, which puts a node under every ninth mode of each of them at once.",
            settings: Settings {
                tune_hz: 164.0,
                decay_s: 4.5,
                voices: 5,
                voice_semis: [0.0, 7.0, 15.0, 22.0, 26.0, 24.0],
                material: -0.5,
                bright_db_oct: -4.0,
                hit: crate::dsp::Point::new(1.0 / 9.0, 0.0),
                pos_l: crate::dsp::Point::new(0.92, 0.0),
                pos_r: crate::dsp::Point::new(0.74, 0.0),
                ..on(Object::Tine)
            },
            modes: vec![],
        },
    ]
}

/// The interval that moves a partial from one ratio to another, in cents.
fn cents_to(from: f32, to: f32) -> f32 {
    1200.0 * (to / from).log2()
}

/// The factory set as the bridge publishes it.
pub fn factory_json() -> Value {
    Value::Array(factory().iter().map(|p| p.to_json()).collect())
}
