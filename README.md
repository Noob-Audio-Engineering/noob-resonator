# Noob Resonator

> **About Noob Resonator.** I wrote it as a humorous, affectionate answer to
> Ableton Live's Corpus, whose seven objects and control set inspired it. It is
> a free plug-in from Noob Audio Engineering, built to show what
> [noob-vst-webgui-framework](https://github.com/Noob-Audio-Engineering/noob-vst-webgui-framework)
> can do with a product-sized interface. It is my tribute to work I admire, not
> a parity replacement for the original — and **nothing here claims to beat it**,
> because nobody has measured it. See [What this does not claim](#what-this-does-not-claim).

A free resonator by Noob Audio Engineering. The plug-in window is the operating
system's web view; what it shows is a Vue single-page app served by the plug-in
itself and driven over the framework's local WebSocket bridge. The DSP, the
parameters and the page are all in this crate; everything reusable is in the
framework.

**The incoming audio supplies the strike; this supplies the body that rings.** A
pluck is a broadband click and the object decides what survives and for how
long. The exciter colours the attack; the resonator owns the pitch and the
decay.

| Part | Where | Role |
|---|---|---|
| DSP | `src/dsp/` | The eight objects, both engines, the selection, the tail, and the parameter and stream layout. Host-agnostic. |
| Plug-in | `src/plugin.rs` (feature `plugin`) | nih-plug VST3 / CLAP effect. Embeds `web/dist`. |
| Standalone | `src/bin/standalone.rs` | Fake audio thread on demo signals plus the framework's server: interface development without a DAW. |
| Measurement | `src/bin/benchmark.rs` | Writes [`docs/BENCHMARK.md`](docs/BENCHMARK.md). |
| SPA | `web/` | The interface. |

## Build and run

```sh
# 1. The page (once, and after every interface change)
cd web && npm install && npm run build && cd ..

# 2. Standalone: serves web/dist on http://127.0.0.1:4246/ (or the next free port)
cargo run --bin noob-resonator-standalone --release -- --open

# 3. Tests (the physics, the bank, the waveguide, the selection, the device)
cargo test

# 4. The measurements
cargo run --release --bin benchmark

# 5. The plug-in (needs web/dist; pulls nih-plug)
cargo build --features plugin --release
```

Step 5 produces the plug-in library (`target/release/noob_resonator.dll`, `.so`
or `.dylib`), which needs to go into a `.vst3` or `.clap` bundle. Without the
`plugin` feature the crate is only the DSP and the standalone, so `cargo test`
and `cargo run` need neither nih-plug nor a built page.

**Build the plug-in library last.** Building a binary without `--features
plugin` recompiles the library without it and leaves an artefact of the same
name with no `GetPluginFactory` in it.

## The one decision this plug-in is about

**"More modes" is the wrong axis, and the affordability question is closed.** A
mode costs about a third of a nanosecond per sample in stereo on the machine
that generated `docs/BENCHMARK.md`, four thousand of them are about six per cent
of one core, and five of the eight objects here have their *entire* physical mode
set inside that budget several times over. A bar tuned to 55 Hz has twenty-eight
partials in the whole audible band. You can simply afford them.

**The scarce resource is *which* modes.** Measured on this engine, against a
reference bank containing every partial the object has: at a 64-mode budget on a
membrane, the ordering that keeps the loudest partials is **68 dB** closer to the
full bank, in its worst band, than the ordering that keeps the lowest ones. Same
cost, same object, same everything else.

All three orderings are on the `Selection` control, so the difference can be
heard as well as read.

There is one setting where the criterion stops being an improvement, and it is
documented in `src/dsp/select.rs` rather than hidden: at exactly **0 dB/octave**
of tilt, a mass-normalised mode set has no frequency trend at all, a membrane
has far more partials per octave at the top of the band than at the bottom, and
"the loudest" therefore means "the highest". The tilt defaults to −3 dB/octave
for that reason among others.

## Three architectures, because there are three kinds of object

**Solids are a mode bank.** The object vibrates, its motion decomposes into
normal modes, and each mode is one damped sinusoid. Beam, Marimba, String,
Membrane, Plate, and a round membrane which is ours.

**Air columns are a waveguide.** A pipe has no material, no strike point on a
surface and no mode shapes: the walls are a boundary and what vibrates is the
air inside. Two delay lines and a reflection at each end, costing the same
whatever number of harmonics come out. **The sign of the reflection at the far
end is the entire difference between a Pipe and a Tube**, and `Opening` morphs
it continuously through a real partly-open termination rather than crossfading
two spectra.

**Above the resolvability crossover, a statistical extension.** A mode is worth
computing separately only while it can be told apart from its neighbours. Above
the frequency where the modal overlap factor reaches one they merge into a
continuum, and for a membrane at 110 Hz that is around a kilohertz — sixty-odd
of its fifty thousand partials. Modelling the rest exactly is possible and
pointless; what the ear can hear very easily is the difference between that
region and **silence**.

## The mode table

A modal bank *is* a list of frequency, gain and decay triples, and every global
knob on it exists to generate that list from a formula. Exposing the list itself
costs nothing at runtime and is native to this architecture and to no other.

The page publishes the loudest 64 partials on the `modes` stream and can write
back a per-partial override — a frequency offset in cents, a gain trim in
decibels, a decay multiplier. The table lives in the interface store, which the
plug-in persists inside its own state, so a project reloads sounding exactly as
it was saved with no editor open.

## The objects

Every series is solved from its own eigenvalue problem rather than copied out of
a book, and `src/dsp/tests.rs` checks each one against published values it was
not built from. `scratchpad/resprobe/p1_physics.py` does the same job from
outside the repository, implementing Bessel functions from their integral
representation and beam eigenvalues by bisection; the worst disagreement between
the two is **0.0001 cents**.

| object | series | checked against |
|---|---|---|
| Beam | `(β_n/β_1)²`, roots of `cos β · cosh β = 1` | Leissa, NASA SP-160 Table 4.23 |
| Marimba | the first two overtones are the maker's | Fletcher & Rossing; Woodhouse |
| String | `n·√(1 + B n²)` | Lehtonen et al., DAFx-08 eq. (2) |
| Membrane | `√((m/Lx)² + (n/Ly)²)` | Russell, Penn State |
| Plate | `(m/Lx)² + (n/Ly)²`, simply supported | Leissa §4.1 |
| Pipe | `(2n−1)·c/4ℓ` at Opening 0, morphing to all harmonics | the standard air-column result |
| Tube | `n·c/2ℓ` | the same |
| **Membrane Round** | `j_{m,n}/j_{0,1}` | Abramowitz & Stegun Table 9.5; Russell |

Three of those rows are worth a sentence each.

**The Marimba's third partial is a control, not a decision.** Its first overtone
is tuned to two octaves and that is agreed; the second is quoted at about 9.2×
by Woodhouse's *Euphonics* §3.3 and at 10× by Fletcher and Rossing. That is close
to a whole tone at that partial — a real choice a builder makes rather than a
discrepancy to average away — so it is on the panel. Above the second tuned
overtone the arch profile no longer controls the ratio and I have no source for
what it becomes; the continuation is modelling and the code says so.

**The Plate is the simply supported one, and that is a statement.** A struck
plate is physically free on all four edges, and the free rectangular plate has
*no closed form* — Leissa's §4.3.15 gives Ritz-method tables and nothing else. A
series that has to be tabulated cannot be solved, so this solves the case that
can be and names which case it is.

**The round membrane is ours**, because a drum head is a disc. Its contact
controls are a **radius and an angle** rather than an x and a y: a square mapped
into a circle puts the control's corners on the rim, where a clamped membrane's
every mode is exactly zero.

## Controls

Where a range came from the device this one answers, it came from that device's
own serialised parameter file or from its engine vendor's published calibration.
Where it is ours, the table says so. The full list with ids and tapers is in
`src/dsp/mod.rs`.

### Body

| control | range | notes |
|---|---|---|
| Object | eight | the seven that device has, in its own order, plus the round membrane |
| Tune | 20 … 4000 Hz | ours: theirs serialises as a bare 0…1 and its range is on no file on disk |
| Transpose, Fine | ±48 st, ±50 ct | theirs |
| Modes | 4 … 4096 | **a stated count**, where theirs is a four-position quality menu that publishes no number |
| Selection | Loudest, Lowest, Log Spread | ours |
| Ratio | 0.2 … 5 | the rectangle's aspect; splits the degenerate pairs |
| Bar Tuning, Third Partial | 4:1 or 3:1; 9.2× or 10× | ours |
| Radius | 1 … 100 mm | a physical bore radius, in the direction wall loss actually moves it |
| Opening | 0 … 100 % | the far end's transition frequency, published on the readouts |

### Damping and tone

| control | range | notes |
|---|---|---|
| Decay | 0.02 … 60 s | **seconds**, where theirs is a bare 0…1 |
| Material | −1 … +1 | `T60(f) = T60(f₁)·(f/f₁)^m`, the law their engine vendor publishes |
| Damp Corner, HF Slope | 100 Hz … 20 kHz; −2 … +1 | ours: the second half of a two-parameter loss model. At the default corner it is inert and the law is exactly theirs |
| Tail | toggle | ours |
| Bright | −6 … +6 dB/oct | **printed in the unit their engine vendor calibrates it in** |
| Inharm | ±100 % | the positive half is Fletcher's stiff string with `B` published live; the negative half is labelled synthetic, because a stiff string's partials are stretched and never compressed |

### Contact

Hit and both pickups have an **X and a Y**. The second coordinate is ours: a
two-dimensional object is struck at a point rather than on a line, and a device
that exposes only one of the two permanently silences a whole sublattice of every
membrane and plate it can make.

Spread detunes the two channels' objects against each other; Width mixes them.

### Exciter, oscillator and output

A band-pass on the excitation with **Pre and Post placement** (ours: theirs is
documented in one place and placed in another), the seven-shape pitch
oscillator, Bleed, Dry/Wet, Gain, and a **limiter that is optional and has no
latency**.

Dry/Wet is an **input gate on the wet send**, not a crossfade at the output.
Turning it down stops new signal being processed and leaves whatever is ringing
to ring out. That device gets this right and it is copied deliberately: a modal
bank whose tail is chopped by a fader is a modal bank that clicks.

## Measured

[`docs/BENCHMARK.md`](docs/BENCHMARK.md) is generated by
`cargo run --release --bin benchmark`. The headline rows:

| | |
|---|---|
| worst partial, measured out of the audio, any object | **0.003 cents** against the physics |
| tuning of one mode, 20 Hz to 16 kHz | **0.0002 cents** |
| decay accuracy, 0.2 s to 8 s | **0.24 %** at worst |
| a thousand-second decay | 1000.0001 s, against 1207 s if the pole radius were stored instead |
| air column, first sixteen resonances | 0.53 cents |
| reported latency | **zero, at every sample rate** |
| cost | about 0.3 ns per mode per sample, stereo, at the portable instruction baseline |
| which modes: best ordering against worst, 64-mode budget | **68 dB** |

## What this does not claim

**Nobody has measured Corpus.** Not this project, not the survey behind it, not
any third party I could find, and it cannot be loaded outside its host. So
nothing in this repository — not the README, not the interface, not a comment,
not a commit message — states a margin over it in decibels or in mode count, and
nothing will until somebody runs a bench session and produces the other number.

What I can say is what this engine measures, which is the whole of
`docs/BENCHMARK.md`, and what Ableton themselves publish: that their Resonator
Quality control works "by reducing the number of overtones that are calculated",
and that their `Bleed` control is "useful for restoring high frequencies, which
can often be damped when the tuning or quality are set to low values". Those are
their sentences about their own device. Ours selects by contribution, and the
Bleed here is a blend rather than a repair.

Three limits are structural and are not defects to be fixed later. The tail
cannot match a membrane's modal density law and does not need to, because above
the crossover the requirement is that the density *exceed* what the ear can
resolve rather than take a particular value. The free plate has no closed form.
And **the air columns do not blow**: a real wind instrument is a nonlinear
exciter in a feedback loop with its bore, and this is a passive linear resonator
driven by whatever audio is put into it. It rings like a tapped length of pipe,
because that is what it is.

## Testing

`cargo test` covers the physics against Leissa, Abramowitz and Stegun, Russell,
Fletcher and Lehtonen; the bank's tuning and decay measured out of its own audio;
the waveguide's two harmonic series and its strike comb; the selection against a
full-bank reference; the tail's density; and the device's latency, bypass,
limiter and mode table.

Two rules govern that file. **Never widen an assertion until it passes**, and
**never assert a value the model produced** — a test that compares the engine
with itself is a tautology with a green tick. Where the model cannot meet a
figure, the row stays and the miss is named.

Both rules earned their keep here. The Bessel recurrence had a parity error that
made every round-membrane mode shape wrong by a factor nobody would have
noticed, and an orthonormality test found it. A benchmark row read five hundred
cents of nonsense and the cause was a contact mapping that put the control's
corners where no mode can be excited. And the row that timed a block found the
resolvability crossover being recomputed on every one of them, at several times
what the audio cost.

## Licence

MIT or Apache-2.0, at your option.
