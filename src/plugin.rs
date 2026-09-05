//! The nih-plug plug-in: VST3 and CLAP, stereo in and stereo out. Its editor
//! is the operating system's web view showing the Vue page from `web/dist`,
//! embedded in the binary.
//!
//! How the pieces connect:
//!
//! * The parameters are nih-plug parameters with the same ids as the
//!   standalone's specs ([`crate::dsp::param_specs`]), mirrored into the
//!   bridge by [`NoobVstWebguiFrameworkEditor::with_builder`], so the same
//!   page drives both. The mirroring samples nih-plug's own mapping into a
//!   table, so the page's knob is exactly this plug-in's knob rather than a
//!   second guess at it.
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

use std::collections::BTreeMap;
use std::num::NonZeroU32;
use std::sync::Arc;

use include_dir::{Dir, include_dir};
use nih_plug::prelude::*;
use noob_vst_webgui_framework::{Assets, AudioHandle, NoobVstWebguiFramework};
use noob_vst_webgui_framework_nih::{EditorConfig, NoobVstWebguiFrameworkEditor, StoreSlot};

use crate::dsp::{self, ModeTable, Point, Processor, Settings, bank};

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
            (g("modes"), self.modes.as_ptr(), g("body")),
            (g("select"), self.select.as_ptr(), g("body")),
            (g("ratio"), self.ratio.as_ptr(), g("body")),
            (g("bar_tuning"), self.bar_tuning.as_ptr(), g("body")),
            (g("bar_third"), self.bar_third.as_ptr(), g("body")),
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

    fn serialize_fields(&self) -> BTreeMap<String, String> {
        let mut m = BTreeMap::new();
        self.ui_store.serialize_into(&mut m);
        m
    }

    fn deserialize_fields(&self, serialized: &BTreeMap<String, String>) {
        self.ui_store.deserialize_from(serialized);
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
    editor: Arc<NoobVstWebguiFrameworkEditor>,
    bridge: NoobVstWebguiFramework,
    audio: Option<AudioHandle>,
    processor: Processor,
    table: Arc<ModeTable>,
}

impl Default for NoobResonator {
    fn default() -> Self {
        let params = Arc::new(NoobResonatorParams::default());
        let (editor, bridge) = NoobVstWebguiFrameworkEditor::with_builder(
            "noob-resonator",
            params.as_ref(),
            dsp::streams(48_000.0),
            EditorConfig::new(1100, 700)
                .size_limits((900, 560), (7680, 4320))
                .devtools(cfg!(feature = "devtools") || cfg!(debug_assertions))
                .assets(Assets::Lookup(ui_lookup)),
            |b| b.meta(dsp::bridge_meta(48_000.0, false)),
        );
        let audio = bridge.take_audio();
        params.ui_store.attach(&bridge);
        let table = Arc::new(ModeTable::new());
        dsp::attach_mode_table(&bridge, table.clone());
        NoobResonator {
            params,
            editor,
            bridge,
            audio,
            processor: Processor::with_table(48_000.0, table.clone()),
            table,
        }
    }
}

impl Plugin for NoobResonator {
    const NAME: &'static str = "Noob Resonator";
    const VENDOR: &'static str = "Noob Audio Engineering";
    const URL: &'static str = env!("CARGO_PKG_HOMEPAGE");
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(1),
            ..AudioIOLayout::const_default()
        },
    ];

    const SAMPLE_ACCURATE_AUTOMATION: bool = false;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        Some(Box::new(self.editor.handle()))
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
        if let Some(v) = self.bridge.store_get(dsp::MODES_KEY) {
            self.table.load_json(&v);
        }
        self.processor.configure(&self.params.settings());
        // Zero, unconditionally, at whatever rate the host runs.
        context.set_latency_samples(0);
        self.bridge.send_json(
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
            // Mono: run the one channel against a copy of itself and keep the
            // left result. The two pickup positions still differ, so what
            // comes back is the left one rather than a mono sum of both.
            let l = &mut *slices[0];
            let mut r = [0.0f32; MONO_SCRATCH];
            let n = l.len().min(r.len());
            r[..n].copy_from_slice(&l[..n]);
            self.processor.process(&mut l[..n], &mut r[..n]);
        }
        if let Some(audio) = self.audio.as_mut() {
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
