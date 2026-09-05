//! Noob Resonator: eight physical objects, two engines, and a mode table you
//! can reach. Built on noob-vst-webgui-framework, with the front panel drawn
//! by the operating system's web view.
//!
//! The incoming audio supplies the strike; this supplies the body that rings.
//! A pluck is a broadband click and the object decides what survives and for
//! how long.
//!
//! | layer | path | role |
//! |---|---|---|
//! | DSP | [`dsp`] | the objects, both engines, the selection, the tail, the parameter and stream layout |
//! | plug-in | `plugin` (feature `plugin`) | nih-plug VST3 / CLAP effect whose editor is the OS web view |
//! | standalone | `src/bin/standalone.rs` | a dev server with a fake audio thread and demo sources |
//! | measurement | `src/bin/benchmark.rs` | writes `docs/BENCHMARK.md` |
//! | page | `web/` | the Vue front panel |
//!
//! Where the framework ends: everything here is specific to this device. The
//! bridge, server, parameter mirroring, host adapter, browser client,
//! gestures and charts come from noob-vst-webgui-framework, which holds
//! generics only and stays headless and uncoloured.

// The kernels index several parallel arrays by the same lane or sample index,
// and an iterator chain over them would hide the arithmetic the comments
// describe.
#![allow(clippy::needless_range_loop)]

pub mod dsp;

#[cfg(feature = "plugin")]
pub mod plugin;

// The VST3 and CLAP entry points. nih-plug generates the C ABI exports from
// the `Plugin` / `Vst3Plugin` / `ClapPlugin` impls in `plugin.rs`.
#[cfg(feature = "plugin")]
nih_plug::nih_export_vst3!(plugin::NoobResonator);
#[cfg(feature = "plugin")]
nih_plug::nih_export_clap!(plugin::NoobResonator);
