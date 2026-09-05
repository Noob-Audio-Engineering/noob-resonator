//! The nih-plug plug-in: VST3 and CLAP, stereo in and stereo out. Its editor
//! is the operating system's web view showing the Vue page from `web/dist`,
//! embedded in the binary.
//!
//! How the pieces connect:
//!
//! **Everything this file is not** now lives in the framework's `PluginHost`:
//! the editor, the bridge and the audio handle, the four-step construction
//! whose order is not interchangeable, the two persistence methods, the
//! input and output layouts and the vendor constants. What is left is what
//! only this plug-in can say — its parameters, their order, how they fold
//! into a settings snapshot, and what `process` does with them.
//!
//! * The parameters are nih-plug parameters with the same ids as the
//!   standalone's specs ([`crate::dsp::param_specs`]), mirrored into the
//!   bridge, so the same page drives both. The mirroring samples nih-plug's
//!   own mapping into a table, so the page's knob is exactly this plug-in's
//!   knob rather than a second guess at it.
//!
//! * **Their order is append-only from the first release.** A host saves
//!   automation by index, so inserting a parameter renumbers every one after
//!   it and silently reassigns a saved project's automation lanes.
//! * `process` reads a [`Settings`] snapshot from the nih-plug values,
//!   configures the [`Processor`], runs the block and publishes the streams
//!   through the audio handle.
//! * **The latency is zero and is reported as zero, at every sample rate.**
//!   There is no lookahead anywhere in the device: the limiter applies its
//!   gain instantly on the way down and releases slowly, which can distort a
//!   fast transient and cannot exceed its ceiling. Lookahead is what costs
//!   samples, and a resonator's output is a sum of decaying sinusoids rather
//!   than a drum hit, so it is the right side of that trade.
//! * The per-mode override table rides in the page's own store, which
//!   [`NoobResonatorParams::ui_store`] persists inside the plug-in state. It
//!   is re-applied in `initialize`, **before the first block**, because the
//!   host restores that state by replacing the store wholesale and a
//!   wholesale replacement does not run the hook a client write does. Without
//!   that line a project would load sounding subtly wrong until somebody
//!   opened the editor.

use std::sync::Arc;

use include_dir::{Dir, include_dir};
use nih_plug::prelude::*;
use noob_vst_webgui_framework::Assets;
use noob_vst_webgui_framework_nih::{
    EditorConfig, PluginHost, StoreSlot, UiStoreParams, noob_identity, stereo_or_mono_io,
    ui_store_fields,
};

use crate::dsp::{self, CHORD_VOICES, ModeTable, Point, Processor, Settings, bank};

/// Longest mono block the scratch buffer covers. Hosts that hand out more
/// than this in one call are vanishingly rare, and the device chunks
/// internally anyway.
const MONO_SCRATCH: usize = 8192;

static UI: Dir = include_dir!("$CARGO_MANIFEST_DIR/web/dist");

fn ui_lookup(path: &str) -> Option<&'static [u8]> {
    UI.get_file(path).map(|f| f.contents())
}

/// The skew that puts a range's **geometric** middle in the middle of the
/// travel.
///
/// nih-plug has no logarithmic range, and a frequency control with a linear
/// one spends nine tenths of its travel above a kilohertz. This is the
/// exponent that makes the halfway point `√(min·max)`, which is what a
/// logarithmic control means in practice.
fn log_skew(min: f32, max: f32) -> f32 {
    let mid = (min * max).sqrt();
    let t = ((mid - min) / (max - min)).clamp(1e-6, 1.0 - 1e-6);
    0.5f32.ln() / t.ln()
}

fn log_range(min: f32, max: f32) -> FloatRange {
    FloatRange::Skewed {
        min,
        max,
        factor: log_skew(min, max),
    }
}

/// Which object is ringing. The first seven are the order the device this one
/// answers lists them in, so an index never moves under a saved project.
#[derive(Enum, Clone, Copy, PartialEq, Eq, Debug)]
pub enum ObjectParam {
    Beam,
    Marimba,
    String,
    Membrane,
    Plate,
    Pipe,
    Tube,
    #[name = "Membrane Round"]
    MembraneRound,
}

/// How the mode budget is spent. The decision this plug-in is about, made
/// audible.
#[derive(Enum, Clone, Copy, PartialEq, Eq, Debug)]
pub enum SelectParam {
    Loudest,
    Lowest,
    #[name = "Log Spread"]
    LogSpread,
}

/// What the maker tunes an arch-cut bar's first overtone to.
#[derive(Enum, Clone, Copy, PartialEq, Eq, Debug)]
pub enum BarTuningParam {
    #[name = "Marimba 4:1"]
    Marimba,
    #[name = "Xylophone 3:1"]
    Xylophone,
}

/// Where the second tuned overtone lands. Two sources disagree and this is
/// the disagreement.
#[derive(Enum, Clone, Copy, PartialEq, Eq, Debug)]
pub enum BarThirdParam {
    #[name = "9.2x"]
    Woodhouse,
    #[name = "10x"]
    Rossing,
}

/// Where the exciter's band-pass sits.
#[derive(Enum, Clone, Copy, PartialEq, Eq, Debug)]
pub enum FilterPlaceParam {
    Pre,
    Post,
}

/// The oscillator's shape, in the ordinal order the device this one answers
/// stores them.
#[derive(Enum, Clone, Copy, PartialEq, Eq, Debug)]
pub enum LfoShapeParam {
    Sine,
    Square,
    Triangle,
    #[name = "Ramp Up"]
    RampUp,
    #[name = "Ramp Down"]
    RampDown,
    #[name = "S&H"]
    SampleHold,
    #[name = "Random Ramp"]
    RandomRamp,
}

/// Every host parameter. The ids match the standalone's specs and the page.
///
/// The [`Params`] implementation is written out rather than derived, so that
/// the ids and the group names are the ones [`crate::dsp::param_specs`]
/// declares rather than whatever the field names happen to spell, and so that
/// the page's store rides along in the plug-in state.
pub struct NoobResonatorParams {
    pub object: EnumParam<ObjectParam>,
    pub tune: FloatParam,
    pub transpose: IntParam,
    pub fine: FloatParam,
    /// The mode budget, as a **number**, where the device this one answers
    /// offers a four-position quality menu and publishes no count at all.
    pub modes: FloatParam,
    pub select: EnumParam<SelectParam>,
    pub ratio: FloatParam,
    pub bar_tuning: EnumParam<BarTuningParam>,
    pub bar_third: EnumParam<BarThirdParam>,
    /// How many of the chord's voices sound, and each voice's pitch in
    /// semitones from the root. Six pitches and no chord: see `dsp::mod`.
    pub voices: IntParam,
    pub voice: [IntParam; CHORD_VOICES],
    pub radius: FloatParam,
    pub opening: FloatParam,
    /// T60 at the fundamental, in **seconds**.
    pub decay: FloatParam,
    /// The exponent in `T60(f) = T60(f₁)·(f/f₁)^m`.
    pub material: FloatParam,
    pub damp_corner: FloatParam,
    pub damp_hi: FloatParam,
    pub tail: BoolParam,
    /// A spectral tilt in **decibels per octave**, which is the unit the
    /// engine vendor behind that device calibrates the same control in.
    pub bright: FloatParam,
    pub inharm: FloatParam,
    pub hit: FloatParam,
    pub hit_y: FloatParam,
    pub pos_l: FloatParam,
    pub pos_l_y: FloatParam,
    pub pos_r: FloatParam,
    pub pos_r_y: FloatParam,
    pub spread: FloatParam,
    pub width: FloatParam,
    pub filter_on: BoolParam,
    pub filter_freq: FloatParam,
    pub filter_width: FloatParam,
    pub filter_place: EnumParam<FilterPlaceParam>,
    pub lfo_on: BoolParam,
    pub lfo_shape: EnumParam<LfoShapeParam>,
    pub lfo_rate: FloatParam,
    pub lfo_depth: FloatParam,
    pub lfo_phase: FloatParam,
    pub bleed: FloatParam,
    pub mix: FloatParam,
    pub gain: FloatParam,
    pub limiter: BoolParam,
    pub limit_ceil: FloatParam,
    pub bypass: BoolParam,
    /// The page's own state, including the per-mode override table; not
    /// parameters, but saved with the plug-in state.
    pub ui_store: StoreSlot,
}

impl Default for NoobResonatorParams {
    fn default() -> Self {
        let d = Settings::default();
        let pct = |name: &str, default: f32| {
            FloatParam::new(
                name,
                default,
                FloatRange::Linear {
                    min: 0.0,
                    max: 100.0,
                },
            )
            .with_unit(" %")
            .with_step_size(0.1)
        };
        NoobResonatorParams {
            object: EnumParam::new("Object", ObjectParam::String),
            tune: FloatParam::new("Tune", d.tune_hz, log_range(20.0, 4000.0))
                .with_unit(" Hz")
                .with_value_to_string(formatters::v2s_f32_rounded(1)),
            transpose: IntParam::new("Transpose", 0, IntRange::Linear { min: -48, max: 48 })
                .with_unit(" st"),
            fine: FloatParam::new(
                "Fine",
                d.fine_cents,
                FloatRange::Linear {
                    min: -50.0,
                    max: 50.0,
                },
            )
            .with_unit(" ct")
            .with_step_size(0.1),
            modes: FloatParam::new(
                "Modes",
                d.modes as f32,
                log_range(4.0, bank::MAX_MODES as f32),
            )
            .with_value_to_string(formatters::v2s_f32_rounded(0)),
            select: EnumParam::new("Selection", SelectParam::Loudest),
            ratio: FloatParam::new("Ratio", d.aspect, log_range(0.2, 5.0))
                .with_value_to_string(formatters::v2s_f32_rounded(2)),
            bar_tuning: EnumParam::new("Bar Tuning", BarTuningParam::Marimba),
            bar_third: EnumParam::new("Third Partial", BarThirdParam::Woodhouse),
            voices: IntParam::new(
                "Voices",
                d.voices as i32,
                IntRange::Linear {
                    min: 1,
                    max: CHORD_VOICES as i32,
                },
            ),
            voice: std::array::from_fn(|k| {
                IntParam::new(
                    format!("Voice {}", k + 1),
                    d.voice_semis[k] as i32,
                    IntRange::Linear { min: -24, max: 36 },
                )
                .with_unit(" st")
            }),
            radius: FloatParam::new("Radius", d.radius_mm, log_range(1.0, 100.0))
                .with_unit(" mm")
                .with_value_to_string(formatters::v2s_f32_rounded(1)),
            opening: pct("Opening", d.opening * 100.0),
            decay: FloatParam::new("Decay", d.decay_s, log_range(0.02, 60.0))
                .with_unit(" s")
                .with_value_to_string(formatters::v2s_f32_rounded(3)),
            material: FloatParam::new(
                "Material",
                d.material,
                FloatRange::Linear {
                    min: -1.0,
                    max: 1.0,
                },
            )
            .with_step_size(0.01),
            damp_corner: FloatParam::new(
                "Damp Corner",
                d.damp_corner_hz,
                log_range(100.0, 20_000.0),
            )
            .with_unit(" Hz")
            .with_value_to_string(formatters::v2s_f32_rounded(0)),
            damp_hi: FloatParam::new(
                "HF Slope",
                d.damp_hi,
                FloatRange::Linear {
                    min: -2.0,
                    max: 1.0,
                },
            )
            .with_step_size(0.01),
            tail: BoolParam::new("Tail", d.tail),
            bright: FloatParam::new(
                "Bright",
                d.bright_db_oct,
                FloatRange::Linear {
                    min: -6.0,
                    max: 6.0,
                },
            )
            .with_unit(" dB/oct")
            .with_step_size(0.1),
            inharm: FloatParam::new(
                "Inharm",
                d.inharm * 100.0,
                FloatRange::Linear {
                    min: -100.0,
                    max: 100.0,
                },
            )
            .with_unit(" %")
            .with_step_size(0.1),
            hit: pct("Hit X", d.hit.x * 100.0),
            hit_y: pct("Hit Y", d.hit.y * 100.0),
            pos_l: pct("Pos L X", d.pos_l.x * 100.0),
            pos_l_y: pct("Pos L Y", d.pos_l.y * 100.0),
            pos_r: pct("Pos R X", d.pos_r.x * 100.0),
            pos_r_y: pct("Pos R Y", d.pos_r.y * 100.0),
            spread: pct("Spread", d.spread * 100.0),
            width: pct("Width", d.width * 100.0),
            filter_on: BoolParam::new("Exciter Filter", d.filter_on),
            filter_freq: FloatParam::new("Filter Freq", d.filter_hz, log_range(50.0, 18_000.0))
                .with_unit(" Hz")
                .with_value_to_string(formatters::v2s_f32_rounded(0)),
            filter_width: FloatParam::new(
                "Filter Width",
                d.filter_oct,
                FloatRange::Linear { min: 0.5, max: 9.0 },
            )
            .with_unit(" oct")
            .with_step_size(0.05),
            filter_place: EnumParam::new("Filter Place", FilterPlaceParam::Pre),
            lfo_on: BoolParam::new("LFO", d.lfo_on),
            lfo_shape: EnumParam::new("LFO Shape", LfoShapeParam::Sine),
            lfo_rate: FloatParam::new("LFO Rate", d.lfo_rate_hz, log_range(0.01, 20.0))
                .with_unit(" Hz")
                .with_value_to_string(formatters::v2s_f32_rounded(2)),
            lfo_depth: FloatParam::new(
                "LFO Depth",
                d.lfo_depth_st,
                FloatRange::Linear {
                    min: 0.0,
                    max: 12.0,
                },
            )
            .with_unit(" st")
            .with_step_size(0.01),
            lfo_phase: FloatParam::new(
                "LFO Phase",
                d.lfo_phase_deg,
                FloatRange::Linear {
                    min: 0.0,
                    max: 360.0,
                },
            )
            .with_unit(" \u{b0}")
            .with_step_size(1.0),
            bleed: pct("Bleed", d.bleed * 100.0),
            mix: pct("Dry/Wet", d.mix * 100.0),
            gain: FloatParam::new(
                "Gain",
                d.gain_db,
                FloatRange::Linear {
                    min: -36.0,
                    max: 36.0,
                },
            )
            .with_unit(" dB")
            .with_step_size(0.1),
            limiter: BoolParam::new("Limiter", d.limiter),
            limit_ceil: FloatParam::new(
                "Ceiling",
                d.limit_ceil_db,
                FloatRange::Linear {
                    min: -24.0,
                    max: 0.0,
                },
            )
            .with_unit(" dB")
            .with_step_size(0.1),
            bypass: BoolParam::new("Bypass", false),
            ui_store: StoreSlot::new(),
        }
    }
}

// SAFETY: every pointer comes from a field of `self`, which nih-plug keeps
// alive in an `Arc` for the plug-in's whole life. Written by hand so the ids
// and the groups match the standalone and the page.
unsafe impl Params for NoobResonatorParams {
    fn param_map(&self) -> Vec<(String, ParamPtr, String)> {
        let g = |s: &str| s.to_string();
        vec![
            (g("type"), self.object.as_ptr(), g("body")),
            (g("tune"), self.tune.as_ptr(), g("body")),
            (g("transpose"), self.transpose.as_ptr(), g("body")),
            (g("fine"), self.fine.as_ptr(), g("body")),
            (g("mode_budget"), self.modes.as_ptr(), g("body")),
            (g("select"), self.select.as_ptr(), g("body")),
            (g("ratio"), self.ratio.as_ptr(), g("body")),
            (g("bar_tuning"), self.bar_tuning.as_ptr(), g("body")),
            (g("bar_third"), self.bar_third.as_ptr(), g("body")),
            (g("voices"), self.voices.as_ptr(), g("chord")),
            (g("voice1"), self.voice[0].as_ptr(), g("chord")),
            (g("voice2"), self.voice[1].as_ptr(), g("chord")),
            (g("voice3"), self.voice[2].as_ptr(), g("chord")),
            (g("voice4"), self.voice[3].as_ptr(), g("chord")),
            (g("voice5"), self.voice[4].as_ptr(), g("chord")),
            (g("voice6"), self.voice[5].as_ptr(), g("chord")),
            (g("radius"), self.radius.as_ptr(), g("body")),
            (g("opening"), self.opening.as_ptr(), g("body")),
            (g("decay"), self.decay.as_ptr(), g("damping")),
            (g("material"), self.material.as_ptr(), g("damping")),
            (g("damp_corner"), self.damp_corner.as_ptr(), g("damping")),
            (g("damp_hi"), self.damp_hi.as_ptr(), g("damping")),
            (g("tail"), self.tail.as_ptr(), g("damping")),
            (g("bright"), self.bright.as_ptr(), g("tone")),
            (g("inharm"), self.inharm.as_ptr(), g("tone")),
            (g("hit"), self.hit.as_ptr(), g("contact")),
            (g("hit_y"), self.hit_y.as_ptr(), g("contact")),
            (g("pos_l"), self.pos_l.as_ptr(), g("contact")),
            (g("pos_l_y"), self.pos_l_y.as_ptr(), g("contact")),
            (g("pos_r"), self.pos_r.as_ptr(), g("contact")),
            (g("pos_r_y"), self.pos_r_y.as_ptr(), g("contact")),
            (g("spread"), self.spread.as_ptr(), g("contact")),
            (g("width"), self.width.as_ptr(), g("contact")),
            (g("filter_on"), self.filter_on.as_ptr(), g("exciter")),
            (g("filter_freq"), self.filter_freq.as_ptr(), g("exciter")),
            (g("filter_width"), self.filter_width.as_ptr(), g("exciter")),
            (g("filter_place"), self.filter_place.as_ptr(), g("exciter")),
            (g("lfo_on"), self.lfo_on.as_ptr(), g("lfo")),
            (g("lfo_shape"), self.lfo_shape.as_ptr(), g("lfo")),
            (g("lfo_rate"), self.lfo_rate.as_ptr(), g("lfo")),
            (g("lfo_depth"), self.lfo_depth.as_ptr(), g("lfo")),
            (g("lfo_phase"), self.lfo_phase.as_ptr(), g("lfo")),
            (g("bleed"), self.bleed.as_ptr(), g("output")),
            (g("mix"), self.mix.as_ptr(), g("output")),
            (g("gain"), self.gain.as_ptr(), g("output")),
            (g("limiter"), self.limiter.as_ptr(), g("output")),
            (g("limit_ceil"), self.limit_ceil.as_ptr(), g("output")),
            (g("bypass"), self.bypass.as_ptr(), g("output")),
        ]
    }

    ui_store_fields!(ui_store);
}

impl UiStoreParams for NoobResonatorParams {
    fn ui_store(&self) -> &StoreSlot {
        &self.ui_store
    }
}

impl NoobResonatorParams {
    /// One block's worth of settings, read from the host's values.
    fn settings(&self) -> Settings {
        Settings {
            object: self.object.value() as usize,
            tune_hz: self.tune.value(),
            transpose: self.transpose.value() as f32,
            fine_cents: self.fine.value(),
            modes: self
                .modes
                .value()
                .round()
                .clamp(1.0, bank::MAX_MODES as f32) as usize,
            order: self.select.value() as usize,
            aspect: self.ratio.value(),
            bar_tuning: self.bar_tuning.value() as usize,
            bar_third: self.bar_third.value() as usize,
            voices: self.voices.value() as usize,
            voice_semis: std::array::from_fn(|k| self.voice[k].value() as f32),
            radius_mm: self.radius.value(),
            opening: self.opening.value() / 100.0,
            decay_s: self.decay.value(),
            material: self.material.value(),
            damp_corner_hz: self.damp_corner.value(),
            damp_hi: self.damp_hi.value(),
            tail: self.tail.value(),
            bright_db_oct: self.bright.value(),
            inharm: self.inharm.value() / 100.0,
            hit: Point::new(self.hit.value() / 100.0, self.hit_y.value() / 100.0),
            pos_l: Point::new(self.pos_l.value() / 100.0, self.pos_l_y.value() / 100.0),
            pos_r: Point::new(self.pos_r.value() / 100.0, self.pos_r_y.value() / 100.0),
            spread: self.spread.value() / 100.0,
            width: self.width.value() / 100.0,
            filter_on: self.filter_on.value(),
            filter_hz: self.filter_freq.value(),
            filter_oct: self.filter_width.value(),
            filter_post: self.filter_place.value() == FilterPlaceParam::Post,
            lfo_on: self.lfo_on.value(),
            lfo_shape: self.lfo_shape.value() as usize,
            lfo_rate_hz: self.lfo_rate.value(),
            lfo_depth_st: self.lfo_depth.value(),
            lfo_phase_deg: self.lfo_phase.value(),
            bleed: self.bleed.value() / 100.0,
            mix: self.mix.value() / 100.0,
            gain_db: self.gain.value(),
            limiter: self.limiter.value(),
            limit_ceil_db: self.limit_ceil.value(),
            bypass: self.bypass.value(),
        }
    }
}

/// The plug-in.
pub struct NoobResonator {
    params: Arc<NoobResonatorParams>,
    /// The editor, the bridge and the audio handle, built in the one order
    /// that works. See the framework's `PluginHost`.
    host: PluginHost,
    processor: Processor,
    table: Arc<ModeTable>,
}

impl Default for NoobResonator {
    fn default() -> Self {
        let params = Arc::new(NoobResonatorParams::default());
        let host = PluginHost::new(
            "noob-resonator",
            &params,
            dsp::streams(48_000.0),
            // The floor is the smallest size the panel is designed against.
            // It is not the saturator's: this face carries a ratio axis with
            // sixty-four partials on it, and below about nine hundred wide the
            // partials stop being individually clickable, which is the whole
            // point of the display.
            EditorConfig::new(1100, 700)
                .size_limits((900, 560), (7680, 4320))
                .devtools(cfg!(feature = "devtools") || cfg!(debug_assertions))
                .assets(Assets::Lookup(ui_lookup)),
            |b| b.meta(dsp::bridge_meta(48_000.0, false)),
        );
        let table = Arc::new(ModeTable::new());
        dsp::attach_mode_table(host.bridge(), table.clone());
        NoobResonator {
            params,
            host,
            processor: Processor::with_table(48_000.0, table.clone()),
            table,
        }
    }
}

impl Plugin for NoobResonator {
    const NAME: &'static str = "Noob Resonator";
    noob_identity!();

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = stereo_or_mono_io!();

    const SAMPLE_ACCURATE_AUTOMATION: bool = false;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        Some(self.host.editor())
    }

    fn initialize(
        &mut self,
        _layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        context: &mut impl InitContext<Self>,
    ) -> bool {
        self.processor.set_sample_rate(buffer_config.sample_rate);
        // The host restores the page's store by replacing it wholesale, which
        // deliberately does not run the hook a client write does — so the mode
        // table has to be picked up here, before the first block, or a
        // reloaded project would sound wrong until the editor was opened.
        if let Some(v) = self.host.bridge().store_get(dsp::MODES_KEY) {
            self.table.load_json(&v);
        }
        self.processor.configure(&self.params.settings());
        // Zero, unconditionally, at whatever rate the host runs.
        context.set_latency_samples(0);
        self.host.bridge().send_json(
            "sample_rate",
            serde_json::json!({ "sample_rate": buffer_config.sample_rate }),
        );
        true
    }

    fn reset(&mut self) {
        self.processor.reset();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        self.processor.configure(&self.params.settings());
        let channels = buffer.channels();
        let slices = buffer.as_slice();
        if channels >= 2 {
            let (a, b) = slices.split_at_mut(1);
            self.processor.process(a[0], b[0]);
        } else if channels == 1 {
            // Mono: run the one channel against a copy of itself and return
            // the **sum of the two pickups**, not the left one.
            //
            // Discarding a pickup would throw away half the object: the two
            // listening positions are at different points on it and hear
            // different partials, because striking or listening at 1/k of the
            // length nulls every k-th one. Summing them is not a compromise
            // either — it is exactly what the Width control's own zero end
            // does, where both resonators feed both sides equally. So a mono
            // instance is the same device with Width closed, which is the one
            // answer that is a setting of this plug-in rather than a guess.
            let l = &mut *slices[0];
            let mut r = [0.0f32; MONO_SCRATCH];
            let n = l.len().min(r.len());
            r[..n].copy_from_slice(&l[..n]);
            self.processor.process(&mut l[..n], &mut r[..n]);
            for i in 0..n {
                l[i] = 0.5 * (l[i] + r[i]);
            }
        }
        if let Some(audio) = self.host.audio() {
            self.processor.publish(audio);
        }
        ProcessStatus::Normal
    }
}

impl Vst3Plugin for NoobResonator {
    const VST3_CLASS_ID: [u8; 16] = *b"NoobResonatorV3W";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Reverb];
}

impl ClapPlugin for NoobResonator {
    const CLAP_ID: &'static str = "io.github.noob-audio-engineering.noob-resonator";
    const CLAP_DESCRIPTION: Option<&'static str> = Some(
        "Eight physical objects, a mode bank and a waveguide, with the mode table exposed \u{2014} \
         with a web-view editor over noob-vst-webgui-framework",
    );
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Reverb,
        ClapFeature::Stereo,
    ];
}
