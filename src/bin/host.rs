//! The plug-in itself, outside a DAW, so the **real editor** can be opened
//! and watched on this machine.
//!
//! Not shipped. It exists because a web view that fails only inside a host
//! is otherwise debuggable one round trip at a time.
//!
//! ```text
//! cargo run --release --features hostapp --bin noob-resonator-host
//! ```

fn main() {
    nih_plug::prelude::nih_export_standalone::<noob_resonator::plugin::NoobResonator>();
}
