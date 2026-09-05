# Noob Resonator · the page

The front panel of [Noob Resonator](../README.md), a Vue 3 + Tailwind
single-page app rendered inside the plug-in's native web view (or a browser
tab), talking to the Rust DSP over
[noob-vst-webgui-framework](https://github.com/Noob-Audio-Engineering/noob-vst-webgui-framework).

**The device is a resonator, not a synthesiser.** The incoming audio supplies
the strike and this supplies the body that rings. A pluck is a short broadband
click; the wooden box decides which frequencies survive and for how long. Tap
a wine glass with a pen, a knuckle or a spoon and you get the same pitch every
time — the exciter colours the attack, and the resonator owns the pitch and
the decay.

So there is no circuit to draw and no hardware to borrow: an object that rings
is a bar, a drum head, a length of pipe, and none of those has a front panel.
What the case borrows from instead is the bench you would put one on to find
out what it does — a **modal analysis rig**: a dark instrument, a graphite
plate, and one large display that answers the only question worth asking.

Three accents, one meaning each, and they never swap:

| | |
|---|---|
| **teal** | the mode bank — a solid object, vibrating in its own normal modes |
| **amber** | the waveguide — an air column, where the object is only a boundary |
| **rose** | what was lost — energy a node took out, at the strike or at a pickup |

Brass is the things you turn and means nothing beyond that. The warning yellow
appears in three places and all three are honesty markers: the stamp on the
display's level axis, a source line that is a tuning target rather than an
equation, and the wall where the bank runs out of modes.

Every colour, proportion and control surface is in this folder. The framework
supplies behaviour and nothing else: a knob's drag, wheel, fine modifier,
double-click reset and arrow keys all come from `useKnobGesture`, and how a
knob *looks* is `components/ResKnob.vue`. `Segmented` ships unstyled on purpose
and is dressed here as this panel's lit keys.

---

## The panel renders; it does not compute

**All of the mathematics in this plug-in belongs to the Rust engine.** It
computes the partials, the levels and the ring times and publishes them on
four streams; this page draws them and derives none of them. There is no
physics in `src/` outside one quarantined directory, and a production build
does not contain a byte of that.

That was not true for the first few days of this page's life, and the
correction is worth writing down. The panel was built in offline design mode
before an engine existed, which was the right call at the time — and it left
the front end solving a transcendental beam equation, finding nodes by
bisection and deriving levels in JavaScript. Two implementations of the same
physics is one too many, and the one that ships is not this one.

**The second half of that correction went in later, and it went in because the
drift finally happened.** Even quarantined, the page still solved every
eigenvalue problem again in JavaScript to fill its design-mode streams — a
Newton search for the beam's roots, a Bessel solver by Simpson integration for
the round head's. Careful, tested, and still two implementations with nothing
comparing them. Then the engine appended two objects, this page's catalogue
still had eight, `objectAt` clamped, and choosing either of the new ones would
have printed *Membrane Round* on the face over a different object's partials.

So the series come off the engine now. `cargo run --release --bin benchmark --
--dump series` writes `id,object,i,j,ratio`, and `tools/gen-previews.mjs`
turns it into two tables: `src/previews.js` for the browse rows, and
`src/dev/series-table.js` for the design-mode stand-in to walk. **The generator
fails outright** if the dump names an object this catalogue does not have, if
the catalogue has one the dump does not, if an index and a label disagree, or
if the rows are not in frequency order. The line is drawn at **root-finding and
special functions, which are the engine's**, against **closed-form algebra over
a live control, which stays here** — the rectangle's eigenvalues, because Ratio
is a control and the dump is one aspect; the air column's phase, because
Opening sweeps; the stiff-string stretch; and the mode shapes. What left was
`beamEigenvalues`, `besselZeros`, `circleModes` and every Newton loop in the
front end.

The tests changed shape with it, and improved. They used to check the page's
solver against the literature; they now feed **the engine's own table** back
through the equation that defines it — each beam ratio turned into `β₁√r` and
substituted into `cos β · cosh β = 1`, each round-head ratio multiplied by
`j₀₁` and put through the Bessel integral — which owes nothing to the code that
produced them. That is a test of the shipping numbers rather than of a second
copy of them.

**How it is arranged now.** `src/dev/` is the quarantine: the design manifest
and, under it, the equations. Its only job is to fill the same four streams
the engine will, so the page still renders before the plug-in is running and
when somebody clones the repository to look at it. It is loaded from a dynamic
import behind `import.meta.env.DEV`, so it is not in a release build; the panel
never imports it and **cannot tell it apart from the engine**, because a panel
that draws streams cannot tell where a stream came from.

The line is easy to check, which is the point of drawing it there:

```
src/objects.js          the catalogue — names, blurbs, sources. No arithmetic.
src/previews.js         a ratio table generated from the engine. Data, not a computation.
src/composables/        parameter handles and stream readers. No physics.
src/components/         axes, marks and prose. No physics.
src/dev/manifest.js     the contract, and the stand-in that fills the streams   ← dev only
src/dev/physics/        the equations                                           ← dev only
```

**Every stream field is read by the name its stream declares, never by an
offset.** A build that publishes fewer fields loses exactly the readouts that
needed them and each of those says so, instead of the page printing whatever
happened to sit at that index. It also means the engine can add a field
anywhere without breaking a page, and that the panel never has to be told
which version of the contract it is talking to.

**Every value on the face is read by the name its stream declares.** A field
the engine does not publish darkens exactly the readout that wanted it, and
the panel says which field it is missing; a field it adds later lights that
readout up with no change here. `test/layout.test.js` covers the mechanism,
because everything optional on this page rests on it.

**Three fields are still asked for**, each darkening one thing: `db_bare`, a
partial's level before the strike and the pickups took their share — the one
that lets the display draw energy *removed* rather than merely absent;
`base_hz`, where a partial sat before Inharm moved it; and `ceiling_hz`, where
the bank runs out, which is the headline demonstration. None is reconstructed
here.

**`f0_hz` landed**, so the ratio axis no longer assumes how the engine folds
Transpose and Fine into Tune, and the axis says when it is falling back on the
Tune control instead.

**One thing was lost to the move and is worth naming.** Hit and the two
pickups used to print the partial they were currently killing. That needed
each contact point's weight on each mode, which is not on the wire and would
have to be worked out on this side. The display still shows the nulls when
`db_bare` arrives, which is the half that matters.

---

## The thing this page is built around

**There are two engines behind one control, and the panel does not hide it.**

Any vibrating object's motion decomposes into normal modes, and each mode is
one exponentially decaying sinusoid — one two-pole resonant bandpass. So eight
of the ten objects are a **mode bank**: a countable list of resonators, one
per partial, each paid for separately.

An air column is different in kind. The object is only a boundary and what
vibrates is the air inside it, so the two air columns are a **waveguide**: a
pair of delay lines with a reflection filter at each end, giving every
harmonic that fits under Nyquist for the same price as giving four.

That split is on the face four times over. The browse view groups the objects
under the engine that produces them. The display draws a mode bank as bars and
a waveguide as a response. Modes — the resonator count — is live for one
engine and greyed for the other. And only one of the two can run out.

---

## What is on it, and whose idea each part was

### Select — ours, and the headline

**Which partials the bank actually runs.** An object has as many partials as
fit under Nyquist and the bank runs `modes` resonators; when there are more of
the former than the latter, something has to choose, and the obvious choice is
the wrong one.

*Lowest* takes partials 1 to N and throws away everything above, whatever it
was doing. That is what a plain mode count does implicitly and what Ableton's
quality setting does by their own description — Applied Acoustics publish the
ladder as 4 / 16 / 30 / 70 modes — and at a low fundamental it is a wall
inside the audio band rather than a gentle roll-off. A seventy-mode string is
only *complete* above about 286 Hz; at 55 Hz it stops dead at 3.85 kHz and
there is nothing above that at all. **Ableton document the consequence
themselves and offer the dry input signal as the remedy**, which is what Bleed
is: a patch over a hole rather than a creative control, and ours says so on
its face.

*Loudest* keeps the ones you can actually hear, wherever they sit. *Log
Spread* keeps the shape of the whole series at the cost of detail anywhere in
it. All three are here so the difference can be heard rather than argued, and
the strip prints what the setting costs right now: how many partials the
object has, how many the bank is running, and where the survivors stop.

The strip is never behind a tab and never in the deck, for the same reason the
sibling plug-in's alias readout is not.

**Three numbers, three different things, and the panel keeps them apart.**
What the object *has* is physics. What the bank *runs* is `modes` filtered by
Select, and the top of that is a wall you can hear. What the display *draws*
is the sixty-four loudest of those, which is a limit on a picture and nothing
more — so the display's own cut is always the loudest whatever Select says,
and no ceiling is ever drawn at the top of it. Running those last two together
had the panel print "the object is deaf above 7 kHz" for a display feed
running out, which would have been false in exactly the way the rest of this
page exists to avoid.

### The partial display — ours

**Ableton's Corpus has no equivalent of this, and it is the one display the
device needs.** A resonator's whole character is decided by where its partials
sit; damping, brightness and strike position only colour a series those ratios
have already fixed. Switch from String to Beam and the answer to "why does one
sing and the other clang" is on the screen.

**The axis is the ratio to the fundamental, not the frequency.** The ratios
are the whole game — 1, 2, 3, 4 sings and 1, 2.757, 5.404, 8.933 clangs — and
neither statement is about hertz. On a ratio axis the octave gridlines are
fixed furniture, so a string's partials fall on a ruler you can read off, a
marimba's tuned second partial lands exactly on the 4× line where the maker
put it, and switching objects moves the series against a scale that has not
moved. A hertz axis spent a third of its width below the fundamental, where
nothing ever happens.

**The ceiling.** When the bank runs out before the axis does, the display
draws the line in the warning colour and says which limit produced it. Pull
Tune down and watch the wall walk into the audio band; give the bank room for
every partial and it goes away. A wall, not a slope.

**The resolvability crossover.** A listener resolves something like twelve to
thirty partials as separate pitches on a bar or a string, and two to six on a
membrane; above that they fuse into timbre. So above the crossover the display
stops drawing separate lines and draws a band, because separate lines claim a
distinction the ear is not making — and because that is what the engine does
too: an exact bank below the crossover and a statistically matched extension
above it, since an exact membrane is around fifty-two thousand modes and sixty
gigaflops. **"More modes" is the wrong axis**, and the display is drawn so
that the reader can see why.

The band is an envelope between a running maximum and a running minimum, not a
polyline through the tops. Half the partials up there are sitting in a node's
null, so a line joining their tops is a sawtooth, and a sawtooth reads as
detail — which is the opposite of the point.

**The strike and the pickups take their share visibly.** Striking an object at
a mode's node gives that mode no energy — the same reason plucking a string a
fifth of the way along kills the fifth harmonic, and the same reason a marimba
bar hangs from a cord threaded through it at 0.224 of its length. A nulled
partial draws a short bar, and a short bar on its own looks like a modelling
choice rather than a physical fact; so the rose ghost above it is the height
it would have had. On a waveguide the same physics is a comb notch and it
carves into the response.

Sweeping **Opening** is the display's best moment. A stopped pipe reflects +1
at the closed end and −1 at the mouth, so the round trip comes back inverted
and only the odd harmonics survive; open the far end and it reflects −1 twice
and the whole series is there. That single sign is the entire difference
between Pipe and Tube — and because it is a phase rather than a switch, the
partials *slide* between the two while the printed air column halves from
75.6 cm to 37.8 cm at the same pitch.

**The engine's own response is drawn for both engines, and on a mode bank it
goes behind the bars.** It says the one thing the bars cannot: *how wide each
resonance is*. Two objects with identical partials and different ring times
draw identical bars and sound nothing alike — at a two-second decay the curve
is a row of sharp spikes exactly on the bars, and at thirty milliseconds the
same partials have smeared into one continuum. `engine.rs` takes the curve
from `banks[0]` or `guides[0]`, so it exists for both; the panel was
discarding half of it.

**The comb is drawn as a per-column envelope, not a sampled curve.** By a few
kilohertz the response has more peaks than the plot has columns, and a curve
sampled once per column there is not a coarse drawing of the response — it is
a drawing of a different one, inventing structure out of where the samples
happen to land.

### The mode table — ours, and the feature no competitor has

**Nine global knobs generate up to four thousand modes and, in every device of
this kind, not one of them is reachable.** Exposing the table costs nothing at
runtime, it is native to a modal bank and to no other architecture, and it is
the natural end of a display that already draws where every partial sits.
Click a partial and set its pitch, its level and how long it rings.

Meanwhile Ableton's own shipped binary contains a four-partial manual mode, a
low-cut, a strike randomiser and a second spatial coordinate, none of which
appear on their panel.

Two things about how it works:

- **An override addresses a partial by its physical index**, which is the
  first float of each `modes` frame — not by where it sits in a list Select
  reorders. Change Select and the same edit follows the same partial.
- **It is written to the UI store, not sent as a message.** A plug-in has no
  main loop — there is the audio thread and the editor's thread and nothing
  else — so a message channel has nothing to pump it: the message route works
  perfectly against the standalone dev server this page was built on and does
  nothing at all inside a VST3. That is the worst shape of bug there is,
  because it passes every test that can be run from this side. The store also
  carries the table inside the plug-in's saved state, so a project reloads
  sounding exactly as it was saved with no editor ever opened.

The key is `modes` and the shape is
`{"edits":[{"i":1,"j":3,"cents":-300}]}`, sparse; `j` absent or zero means an
object with one index, and clearing every override removes the key rather than
leaving an empty array.

**The whole series can be shaped by drawing across it.** The per-partial
editor is right for a correction and hopeless for sixty-four of them, and the
surface already existed: every partial is drawn against a level lane and a
ring lane, each with its own scale, so a drag across a lane sets that quantity
for every partial it passes. The stroke shows as it goes and commits once on
release — one write, and one thing to undo.

**It generates into the table and the table stays editable.** A drag writes
ordinary per-mode overrides, exactly the ones a click writes, so afterwards
you pick the two that are wrong and fix them by hand. Nothing is locked while
drawing is on. Draw the shape, then edit it — never generate *instead* of
edit, which is the mistake of replacing a set of tunable resonators with a
menu of fixed ones.

**Level and ring only, because those are the lanes that exist.** Pitch is a
horizontal quantity here — a partial's frequency *is* its position — so a
vertical drag cannot honestly mean it, and detuning stays a per-partial edit
rather than getting a gesture that would have to lie about its own axis.

**It is reachable from the keyboard.** Sixty-four tab stops would be worse
than none, so the plot is a single stop and the arrows walk the series, Home
and End go to its ends, and Escape lets go.

**A mode's identity is the pair, everywhere, including in the markup.** Every
list the display draws — the bars, the handles, the ghosts, the ring dots — was
keyed on `i` alone, and a surface's modes routinely share a first index: (1,5)
and (1,6) are two partials at two frequencies with one key between them, so the
framework patched one element where two were meant. It warned on both discs and
every rectangle. The same pair that addresses an override now keys the markup,
which is the point of having a pair at all.

**Undo does not reach these**, and the editor says so rather than leaving it
to be discovered. The framework's history covers parameters, and the override
table is plug-in state rather than a parameter — which is exactly what lets it
travel with a saved project. A second history with its own Ctrl+Z semantics
would be worse than a Reset that is where you are already looking.

### Presets — ours, on the engine's format

Factory presets come from the engine in the manifest meta, read-only, because
they are physics and belong next to the code that defines the ranges they sit
in. A user's own live in the UI store beside the mode table, so they ride
inside the plug-in state and a saved project brings its own presets back.
**Applying is the page's job for both**, and that is a fact rather than a
division of labour: a parameter change has to go through the host to be
recorded and undoable, and the engine cannot set a host parameter behind the
host's back.

**Loading replaces the mode table, and the view says so before you do it.**
`modes` is mandatory in the format and an empty one *clears* whatever you had
— one rule, no ambiguity, and the kind of thing a user has to be told rather
than discover. The save dialog offers to leave your retuned partials out,
which writes an empty table and says what that will mean.

**Every covered parameter is set on load** — from the preset where it has a
value and from its own default where it does not — so a preset fully
determines the state and cannot leave a stray control behind from whatever
was loaded before it. `bypass` is never in a preset: it is a transport
control, and one that silently bypasses the plug-in is a support ticket.

**Pairs are found structurally and shown as pairs.** Two presets that differ
in exactly one control exist to be compared, so the browser marks both rows
and names the control. The one that matters is the same string at the same
budget with Selection on Loudest and on Lowest — the argument of the whole
device, met by accident rather than explained. Detecting it by diffing values
rather than by naming convention means it survives a rename and finds any
other deliberate pair too.

**Whether it has been edited since loading is diffed here**, not tracked by
the engine: a second copy of the truth across a wire is a second thing that
can disagree, and plausible disagreement is the failure this project keeps
catching. The mode table is part of the comparison, because it is part of the
preset. It reads through the framework's parameter *handles* rather than the
client's own `Param`, which is not reactive — read from there the diff never
re-evaluated and the edited dot never appeared however far you moved a knob,
which is a status wrong in the reassuring direction.

**Checked against a running plug-in rather than against design mode**, because
design mode is where a preset system would look perfect and prove nothing.
Driving the real panel over the real bridge: the 33 factory presets arrive from
the engine's manifest and draw in ten groups; loading Glockenspiel puts the
object on Beam and Tune on 880 Hz in plain units; the pair loads with every
*knob* identical and Selection alone moved, which is the whole argument;
`Hand Bell` arrives with its five retuned partials marked on the display, and
loading a preset with an empty `modes` clears them again; moving a control
raises the edited dot; and a saved user preset survives a page reload, which is
what proves it is in the plug-in state rather than in the page.

### The browse view — **the sibling lab did this first**

Choosing an object is a view, one per row, grouped by family, with a "Good
for" line naming actual uses. That whole shape is
[Noob CompressorLab](../../noob-compressorlab/web/README.md)'s model browser,
and it is adopted rather than reinvented.

**Every row says where its own numbers came from.** All ten are the engine's
now, the air columns included — those are measured off the running delay loop
rather than assumed from the ideal `2n − 1`, which turns out to matter: a real
loop with a filtered reflection is dispersive, and its fifty-third resonance
sits about 2.7 cents sharp of where the closed form puts it. The label stays
even though every row currently reads the same, because "these are the
engine's partials" and "these are the equation's partials" are not the same
claim and the day one row stops being the first, the face should say so rather
than the code quietly forgetting.

**What makes it fit better here is the preview.** The lab previews a
compressor with its real faceplate, so a thumbnail cannot drift from the panel
it represents. A resonator has no faceplate — a bar, a drum head and a length
of pipe all look like the same grey box — but it has something better: the
series *is* the difference between them. So each row draws that object's own
partials on one shared six-octave ruler, from a table generated out of the
engine — because a browser showing ten objects cannot read a stream that only
carries the loaded one, and solving ten eigenvalue problems in the front end is
what this page's architecture forbids. The bar heights there are
a drawing convention; only the positions mean anything, which is also the
honest thing, since an object's levels depend on damping the row knows nothing
about. The browser does not merely list eight
names; it shows why a beam is not a string before you have committed.

**It is a layer over the panel, not a page in place of it.** It replaced the
whole panel once, and a user pressing the button reported that everything had
disappeared — which it had. The settings you are choosing *for* are most of
the value of browsing, so the panel now stays visible and blurred behind it,
and the way out is a brass **← Back to <object>** with an `Esc` chip rather
than a dim word in a corner. The object's name in the top bar is a label and
not a button, for the same reason: it reads as a status, and pressing it swept
the panel away.

**Browsing does not touch the audio.** The object that is loaded keeps ringing
with its own settings the whole time; `type` is written only when a row is
chosen. Leaving by Escape, by the close button, or by choosing the object
already loaded writes nothing at all.

**Each engine group carries a diagram**, because the prose was not working: a
reader who had read every paragraph about mode banks and waveguides still
could not say what the relationship was. The picture makes one point — a mode
bank is a stack of filters, one per partial, each paid for; a waveguide is one
delay loop with a reflection at each end and every partial falls out of it for
the same price. That is why Modes is live on one and greyed on the other.

### The first seven objects and the control set — **Ableton did this first**

Beam, Marimba, String, Membrane, Plate, Pipe and Tube are Corpus's model list
in Corpus's own index order, and Decay, Material, Brightness, Inharmonics,
Ratio, Radius, Opening, Hit, Pos, Bleed, Spread and Width are Corpus's
controls. Adopting them makes an A/B honest and costs a musician nothing to
learn, and we are saying so rather than pretending we arrived at the same
seven objects independently.

**Three are ours, and each adds a family rather than a variation.**

* **Membrane Round.** Corpus's Membrane is a rectangle and a drum head is a
  circle; the two have genuinely different series — the circular one is the
  zeros of the Bessel functions, 1 : 1.593 : 2.136 : 2.296 — and a circle has
  no aspect, so Ratio is meaningless on it rather than merely unused.
* **Tine.** The same bar as Beam with one end held instead of free, which is
  one sign in the frequency equation — `cos β · cosh β = −1` rather than `+1` —
  and a different instrument. A free bar's overtones sit at 2.76 and 5.40; a
  cantilever's at 6.27 and 17.5, so there is nothing left in the range where a
  glockenspiel clangs and it rings almost pure. A tuning fork, a music box
  tooth, an electric piano tine.
* **Plate Round.** A disc held at its rim by its own stiffness rather than by
  tension: 1 : 2.08 : 3.41 : 3.89 : 5.00, far wider than the round head's,
  because a stiff object's frequencies go as the square of the eigenvalue
  where a tensioned one goes as the eigenvalue itself. Same outline as a drum
  head, entirely different instrument. Its own caveat is on the face: a real
  cymbal is *free* at its rim rather than clamped, and its crash is a
  nonlinearity no linear resonator has — this is the clamped disc, which is a
  bell plate.

Each is **appended rather than slotted in** beside the object it is related
to, because a saved project's object is its index.

**And the catalogue is checked against the engine rather than trusted**, which
it had to be after it went stale: the engine appended Tine and Plate Round,
this page still listed eight, and `objectAt` clamps — so choosing either would
have drawn "Membrane Round" on the face over a different object's partials. A
wrong name printed confidently is worse than a blank. `tools/gen-previews.mjs`
now refuses to write anything if the two lists disagree in either direction, or
if an index and a label disagree.

Greying out the controls an object has nothing for is theirs too. **The page
does not derive which ones**: the engine publishes an `objects` table in its
manifest meta and `uses` is the truth, because that is what the engine
actually reads. What the page adds is the sentence saying why, because a
greyed control with no explanation is the thing this panel exists to improve
on. A build that publishes no table greys nothing and the bench says so.

**`id` in that table is the object's *index*, not a string.** A saved project
loads an object by its position, so that is what identifies one on the wire.
Keying the lookup by the catalogue's string id instead matched nothing — and
matched nothing *silently*, so the panel greyed no control at all while
looking entirely correct against a design manifest that happened to use
strings. It is looked up by index now.

**And the page checks that list before it greys anything from it.** The engine
renamed the mode-budget parameter from `modes` to `mode_budget` and the object
table went on naming `modes`, so every bank object's `uses` array named a
control that did not exist and did not name the one that did. The panel greys
from `uses`, so the headline knob of the whole device would have been dark in
the host and alive everywhere it was tested — silent, and invisible to any test
that reads only one side. So the page compares the two lists it already has:
**where `uses` names a control this build does not publish, the list is stale,
the panel greys nothing at all from it, and it says so on the object bar in the
warning colour.** A stale list is not evidence about any control, and failing
towards a working panel with a visible complaint is better than failing towards
a dead knob in silence. Fixed at the engine's end; the check stays.

**The engine took half the argument.** The air columns keep Bright, Hit and
both pickups, which is the physics: a pipe loses its highs to the walls and
the open end just as a bar loses them to internal friction, and injecting a
wave a third of the way along a delay loop cancels every third harmonic. What
they do not get is the mode budget, the Selection, Material and the damping
law's other controls — a loop's loss is a property of the loop rather than of
a per-mode law, and there is no budget to spend, so there is nothing to
choose between. Every one of those greyed controls says which of those it is.

**`forces` is the engine's, and the panel stopped writing it.** A Tube is a
Pipe with its far end fully open — one loop at two settings of one
termination — so the engine pins `opening` for it and publishes that as
`forces`, rather than the browse view setting the parameter itself as it used
to. One authority for what an object pins, and the panel free to say so: the
engine's own note about the pair is printed under the object's name.

### The level readout — ours

In, out, and what the limiter is taking off. **A resonator needs one more than
most effects do:** a long decay and a large mode budget is a bank of
resonators being fed continuously, and it can go on climbing after the input
has stopped. The limiter catches that, is on by default and is optional, so
the number that matters is how hard it is working — a user who cannot see it
has no way to tell a device that is behaving from one that is only being
rescued.

**In the top bar and not in the deck**, which is a size decision as much as a
design one: the deck scrolls at small windows, and a level readout that can
scroll out of sight is not one. It is also deliberately outside the accent
scheme — teal, amber and rose each mean one thing about the physics, and a
meter means none of them, so it is plain ink with the warning colour only at
the top of the scale and on the clip lamp. The peak hold decays because a bar
that only ever rises is unreadable; the clip lamp latches because a clip you
missed is the one worth knowing about.

### Four readouts that earned their place — ours

The engine publishes twelve numbers on `info` and the panel shows the ones
that change what a reader would do. Each of these replaces a control that
would otherwise be a bare percentage:

| field | where it lands | why |
|---|---|---|
| `build` | *still building the mode table · 60%* | the bank spreads its mode search across blocks, so the display can be looking at a table that is still filling in. A half-built series that does not say so is one a reader takes at face value. |
| `inharm_b` | on the Inharm knob, as `B` | the coefficient the stiff-string relation is actually written in and the one you can look up. A percentage says nothing about what the device is doing. |
| `open_hz` | on the Opening knob | a partly-open end is not open at every frequency: below this it still reflects like a closed one. That is the physical fact the control is setting. |
| `tail_db` | beside the Tail switch | otherwise the switch is a mystery with no reading. |

`engine` is read too, and **only as a cross-check**: the panel already knows
which engine is running because the catalogue says so, and this says it again
from the other side of the wire. A field that always agrees costs nothing; the
one time it disagrees, the page and the DSP have drifted about what is being
synthesised, and the display would otherwise draw a perfectly reasonable
picture of the wrong engine. A disagreement is printed in the fault colour.

### Controls that state what they are doing — ours

A control the reader cannot reason about is a defect, which is the rule the
sibling plug-in set when it made its colour width a stated Q. Decay is
seconds, Material is the exponent of the damping law, Brightness is decibels
per octave, Spread prints its detuning in cents, Hit and the pickups print the
partial they are currently killing, and the air column prints its length, its
loop time and the end correction its bore is worth.

**Where a unit is ours rather than theirs, it is ours because Ableton do not
have one.** Eleven of their parameters serialise as a bare zero-to-one with no
unit anywhere on disk, the Tune knob in hertz among them. Their *ranges* are
established — twenty-six factory presets, byte-identical, sitting on both
endpoints of nine parameters — but there is no default file for this device,
so no default here is theirs either.

### Two controls that expose a builder's choice — ours

The marimba's second partial is cut to 4 for a marimba and 3 for a xylophone,
and its third is given as about 9.2 in one source and 10 in another. **That is
not a discrepancy to average away** — it depends on how deeply the bar is
undercut, so it is a decision a maker makes, and both values are right for the
bar their author was holding. Averaging them into 9.6 would describe a bar
nobody has ever built. So both are controls, and a device that fixes one
silently is the thing being improved on.

---

## Honesty

**The stamp says which of two things is filling the streams.** Live, the
partials came from the engine and there is no stamp. In design mode it reads
*every level here is the page's own arithmetic · the ratios are the engine's
table*, and it sits inside the plot on the level axis rather than floating as a
caption, because a reader needs to know precisely which picture they are
looking at.

**That stamp got narrower rather than softer**, and the distinction is the
point. It used to say the whole picture was the page's arithmetic, which was
true when the page solved every series itself and became an overclaim the day
the ratios started coming off the engine's own table. Overclaiming in the
direction of caution sounds harmless and is not: a warning that is loose about
what it warns of is one a reader learns to discount, and the sentence that has
to survive a screenshot is *every level here is invented*.

**Nine of the ten series are the solution of a stated closed form**, and
`test/design-physics.test.js` holds the engine's own numbers to those forms
rather than solving them a second time — each beam ratio turned back into its
eigenvalue and substituted into `cos β · cosh β = 1`, each tine ratio into
`cos β · cosh β = −1`, each round-head ratio multiplied by `j₀₁` and put through
the Bessel integral, the rectangles compared against the page's own lattice,
and the air columns against the ideal loop they are a dispersive version of.
Two results came out of writing those tests rather than out of reading a
source:

- **The beam ratio quoted everywhere as 2.756 is a truncation of 2.75654, not
  a rounding of it.** Correctly rounded it is 2.757. Computing it rather than
  quoting it is the only way that stays true.
- **An undercut bar has no closed form.** Its ratios are a maker's tuning
  target, which is why it is marked in the warning colour as a tuning target
  against the other nine's closed forms, and why the two published values for
  its third partial are offered as a builder's choice rather than averaged
  into a bar nobody has built. It carries the further caveat that the mode
  shapes used for its node positions are still the uniform bar's, because the
  arch moves those too and nothing describes the cut.

The two air columns get the same treatment from the other direction: their
closed forms are the two ends of Opening and are exactly right only there, so
with the control anywhere between, the panel says so and the object's name
stays where it was rather than flipping to the other one.

**An unset field must be absent, not plausible.** The design manifest's `info`
frame was zero-filled once, so every field the stand-in never computed
published a convincing `0.0` — and the level meter dutifully reported
`0.0 dB GR`, a measurement nothing had made. It is `NaN`-filled now, and a
non-finite value is how an engine says *not computed*: the page's field reader
turns it into an absence and the readout that wanted it goes dark. A real zero
still survives, because zero is a measurement.

**A control that does nothing says so.** The clamped disc's mode shape needs a
modified Bessel function, which is exactly the machinery that left this
directory, so design mode does not model it: Hit and the two pickups do nothing
on Plate Round until a plug-in answers, and the bench says which object and
why. A dead knob that looks alive teaches the wrong thing about the product;
a dead knob that is labelled teaches the right thing about the stand-in.

**Nothing on this page animates.** The design generators are pure functions of
the parameter values, so the panel sits perfectly still until a control moves
and a screenshot of it is a screenshot of the controls.

**One thing is deliberately left unresolved.** Ableton's own conditional table
lists Bleed unconditionally while their manual says it is deactivated for the
air columns. The same file is known to omit a conditional elsewhere, so it is
authoritative where it speaks and proves nothing by its silence. The page
asserts neither: Bleed is live on every object until somebody looks at the
real device.

## The look is ours, and the layout is not theirs

This is an affectionate spoof, not a clone and not a parity replacement.
Corpus's model list, index order, control names and greying behaviour are
adopted and credited above, and so is the sibling lab's browse view. None of
Ableton's colours, proportions, control arrangement or naming is: their device
is a pale flat strip of Live-standard controls with no display of the partials
at all, and the panel here carries NOOB names throughout.

---

## Layout

```
top bar          39 px   what this is, what is loaded, undo / redo / A-B, the bench key
object bar               the loaded object, its engine, its blurb and its source
select strip             which partials the bank runs, and what that costs
display          1fr     where the partials land, how long they ring, where it runs out
deck                     body · damping · contact · out, following the physical story
bench                    off by default; floats over the deck when the Bench key opens it
browse view              replaces everything but the top bar
```

The order down the page is the physical story rather than the signal path:
what is ringing, which of its partials survive, what that leaves, and only
then the controls that shape them. A resonator has no signal path worth
arranging a panel around.

**The display is never behind a tab** and takes every pixel the window can
spare: 564 px at 1900 × 1000, and 86 at the 900 × 520 minimum.

The panel lays out down to **900 × 520**, which is what `WINDOW_MIN` in
`composables/useResonator.js` declares. Four things give at the narrow end,
each by a stated rule:

- **The deck becomes two by two below 1200 px** rather than being left to
  wrap. Wrapping put three groups on one row and the fourth alone on a second,
  and whether it wrapped at all came down to a few pixels of slack.
- **The deck scrolls rather than pushing the display out.** It holds forty-one
  host parameters in six groups now, and at a window that cannot show them all
  every control stays reachable while the one plate carrying the argument
  keeps its floor. The scrollbar is the honest signal that there is more.
- **A control's name wraps rather than running into its neighbour.** `Damp
  Corner` sets sixty-six pixels wide and the cell it sits in is fifty once the
  knob scale is down to 0.78, so at 900 px the damping row read
  *INHARMDAMP CORNERHF SLOPE*. The label is the parameter's own name and the
  panel does not get to shorten it — that is the name the host prints in an
  automation lane, and a face that disagrees with the lane is worse than a face
  that takes a second line. Every deck label reserves the room for a second
  line whether it needs one or not, so a row of figures stays on one baseline.
- **The Select strip loses its paragraph below 660 px of height**, keeping its
  three figures, which are the argument.
- **The deck's second lines go below 640 px**, and the object's description
  below 560. Both are still somewhere: the deck's on each control's tooltip,
  the object's in the browse view.
- **The display drops its own rows by measuring itself.** The legend and the
  provenance line go when the plate is too short for them — measured against
  the plate, not the window, because how much room the display has depends on
  how tall the deck is and the deck's height depends on how many groups an
  object has. A viewport media query got that wrong and the provenance line
  spilled over the deck, which was patched with `overflow: hidden` — a clip,
  not a fix, and one that would have silently swallowed the next thing added.
  Nothing is clipped now.
- **The ring-time lane is dropped when the plot is under 120 px**, and the
  legend says so.

Two states are called out wherever you are, because both explain a silent
device: **bypass** gets a rose chip in the top bar that switches it back in
when clicked, and the level readout's **clip lamp** latches until cleared.

What never gives is the reading size of a figure, or the stamp.

The bench floats over the deck rather than sitting under it, because a second
plate in the flow pushed the display out through the bottom of the window at
the minimum size and the page began to scroll.

---

## Running it

The page needs a manifest. Either run the plug-in and let it supply one:

```
cargo run --bin noob-resonator-standalone      # terminal 1, port 4246
cd web && NOOB_VST_WEBGUI_FRAMEWORK_PORT=4246 npm run dev    # terminal 2
```

…or run the page alone and let it fall back to the design manifest:

```
cd web && npm run dev
```

**Offline design mode is where this page was built**, before the DSP existed —
`src/dsp/` is empty as this is written, so `dev/manifest.js` is the
specification the engine half will be held to rather than a mirror of one.
Production builds do not include the file at all.

```
npm run build     # dist/, which the Rust side serves or embeds
npm test          # the design-mode equations, and the stream reader
npm run previews  # rebuild the browse view's ratio table from them
```

**Verify against `dist`, not only against the dev server.** A production-only
fault is invisible to a dev-server sweep — the editor came up blank in a host
once while every dev check was green. The standing sweep loads all three dev
views *and* builds, serves `dist` and loads that, asserting the page has
content **and visible text**. That distinction is what separates "never
loaded" from "loaded and waiting": with no plug-in behind it the production
bundle reaches `connecting to the plug-in`, so a blank window means the page
never loaded at all.

`npm test` does not test the plug-in — the engine's own tests guard what
ships. It guards `src/dev/physics/`, so that the thing anybody looks at while
designing is not quietly wrong.

---

## Files

| file | what |
|---|---|
| `objects.js` | the catalogue: ten objects, what each is, what it is for, and the equation its series is cited from. No arithmetic. |
| `streams.js` | reading a stream by the names it declares, and the rule that a non-finite value means *not computed*. No framework imports, so it can be tested. |
| `previews.js` | forty ratios per object for the browse view's rows, **generated from the engine**; rebuild with `npm run previews`. |
| `dev/series-table.js` | **dev only** — the same numbers with their mode indices, for the design-mode stand-in to walk. Generated, never hand-edited. |
| `tools/gen-previews.mjs` | writes both, out of `benchmark --dump series`; refuses if the catalogue and the engine disagree about which objects exist, or if the dump is not in frequency order |
| `composables/useResonator.js` | the parameter handles, the four streams read by field name, the override table, the published `uses` lookup, `WINDOW_MIN`. No physics. |
| `dev/manifest.js` | **dev only** — the contract, and the stand-in that fills the four streams so the page renders without a plug-in |
| `dev/physics/` | **dev only** — the mode shapes and the two laws applied over the engine's series table. No root-finding and no special functions: those are the engine's. |
| `components/ObjectBar.vue` | what is loaded, its far end when that is not what the name says, and the way in to changing it |
| `components/EngineDiagram.vue` | the two engines, drawn, because the prose was not landing |
| `components/PresetBrowser.vue` | presets, one per row, grouped by object, with the A/B pairs marked |
| `composables/usePresets.js` | the preset format, loading, saving and the edited-since diff |
| `components/LevelStrip.vue` | in, out, clip and the limiter's gain reduction |
| `components/TypeBrowser.vue` | the browse view |
| `components/SeriesPreview.vue` | one object's series, drawn small, as its row's preview |
| `components/SelectStrip.vue` | the headline: which partials the bank runs |
| `components/ModeDisplay.vue` | the partial display |
| `components/ModeEditor.vue` | one partial's pitch, level and ring time |
| `components/Deck.vue` | every other control |
| `components/ResKnob.vue` | the knob face |
| `components/PanelPage.vue`, `TopBar.vue` | the panel and its bar |
| `components/DevPanel.vue` | the bench: the partial table, and where each column came from |
| `style.css` | every colour and every dimension |
| `test/design-physics.test.js` | the design-mode equations, solved from scratch |
| `test/layout.test.js` | reading a stream by name, and every way a field can be absent |

The bench also carries the standalone's demo source — a resonator supplies a
body and the incoming audio supplies the strike, so with no host feeding it
there is nothing to excite and the panel looks broken rather than idle. Those
three parameters are absent under a plug-in, where the host is the exciter.

Every stream is optional and every reader says so: a build that has not got as
far as publishing its own partial list renders the series from the equations
and prints which, rather than a blank page or a lie.
