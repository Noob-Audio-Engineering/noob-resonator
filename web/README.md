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
three streams; this page draws them and derives none of them. There is no
physics in `src/` outside one quarantined directory, and a production build
does not contain a byte of that.

That was not true for the first few days of this page's life, and the
correction is worth writing down. The panel was built in offline design mode
before an engine existed, which was the right call at the time — and it left
the front end solving a transcendental beam equation, finding nodes by
bisection and deriving levels in JavaScript. Two implementations of the same
physics is one too many, and the one that ships is not this one.

**How it is arranged now.** `src/dev/` is the quarantine: the design manifest
and, under it, the equations. Its only job is to fill the same three streams
the engine will, so the page still renders before the plug-in is running and
when somebody clones the repository to look at it. It is loaded from a dynamic
import behind `import.meta.env.DEV`, so it is not in a release build; the panel
never imports it and **cannot tell it apart from the engine**, because a panel
that draws streams cannot tell where a stream came from.

The line is easy to check, which is the point of drawing it there:

```
src/objects.js          the catalogue — names, blurbs, sources. No arithmetic.
src/previews.js         a generated ratio table for the browse view. Data, not a computation.
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

**What is asked of the engine, and why.** Five fields on the streams exist so
the panel does not have to derive something it shows. `db_bare` is a partial's
level before the strike and the pickups took their share, which is what lets
the display draw energy *removed* rather than merely absent. `base_hz` is
where a partial sat before Inharm moved it. `ceiling_hz` is where the bank
runs out. `column_m` and `loop_s` are the air column's own dimensions, and
`f0_hz` is the fundamental every ratio on the display is measured against —
which the engine knows and the page would otherwise have to assume. All are
marked as proposals in `dev/manifest.js`. If any is declined, the readout that
needed it goes dark on its own and the bench lists which.

**One thing was lost to the move and is worth naming.** Hit and the two
pickups used to print the partial they were currently killing. That needed
each contact point's weight on each mode, which is not on the wire and would
have to be worked out on this side. The display still shows the nulls, from
`db_bare`, which is the half that matters.

---

## The thing this page is built around

**There are two engines behind one control, and the panel does not hide it.**

Any vibrating object's motion decomposes into normal modes, and each mode is
one exponentially decaying sinusoid — one two-pole resonant bandpass. So six
of the eight objects are a **mode bank**: a countable list of resonators, one
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

The key is `modes` and the shape is `{"edits":[{"i":5,"cents":-250,"db":-9,"decay":3}]}`,
sparse; clearing every override removes the key rather than leaving an empty
array.

### The browse view — **the sibling lab did this first**

Choosing an object is a view that replaces the panel, one per row, grouped by
family, with a "Good for" line naming actual uses. That whole shape is
[Noob CompressorLab](../../noob-compressorlab/web/README.md)'s model browser,
and it is adopted rather than reinvented.

**What makes it fit better here is the preview.** The lab previews a
compressor with its real faceplate, so a thumbnail cannot drift from the panel
it represents. A resonator has no faceplate — a bar, a drum head and a length
of pipe all look like the same grey box — but it has something better: the
series *is* the difference between them. So each row draws that object's own
partials on one shared six-octave ruler, from a table generated out of the
same equations — because a browser showing eight objects cannot read a stream
that only carries the loaded one, and solving eight beam equations in the
front end is what this page's architecture forbids. The bar heights there are
a drawing convention; only the positions mean anything, which is also the
honest thing, since an object's levels depend on damping the row knows nothing
about. The browser does not merely list eight
names; it shows why a beam is not a string before you have committed.

**Browsing does not touch the audio.** The object that is loaded keeps ringing
with its own settings the whole time; `type` is written only when a row is
chosen. Leaving by Escape, by the close button, or by choosing the object
already loaded writes nothing at all.

### The eight objects and the control set — **Ableton did this first**

Beam, Marimba, String, Membrane, Plate, Pipe and Tube are Corpus's model list
in Corpus's own index order, and Decay, Material, Brightness, Inharmonics,
Ratio, Radius, Opening, Hit, Pos, Bleed, Spread and Width are Corpus's
controls. Adopting them makes an A/B honest and costs a musician nothing to
learn, and we are saying so rather than pretending we arrived at the same
seven objects independently.

**The eighth is ours.** Corpus's Membrane is a rectangle and a drum head is a
circle; the two have genuinely different series — the circular one is the
zeros of the Bessel functions, 1 : 1.593 : 2.136 : 2.296 — and a circle has no
aspect, so Ratio is meaningless on it rather than merely unused. It is
appended rather than slotted in beside the rectangle, because a saved
project's object is its index.

Greying out the controls an object has nothing for is theirs too. **The page
does not derive which ones**: the engine publishes an `objects` table in its
manifest meta and `uses` is the truth, because that is what the engine will
actually read, and deriving it from the outside was got wrong twice. What the
page adds is the sentence saying why, because a greyed control with no
explanation is the thing this panel exists to improve on. A build that
publishes no table greys nothing and the bench says so — greying a control the
engine may be reading would be worse than greying none.

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
*these partials are the page's own arithmetic, not the engine's*, and it sits
inside the plot on the level axis rather than floating as a caption, because a
reader needs to know precisely which picture they are looking at.

The equations behind that arithmetic are worth their keep even so. **Seven of
the eight series are the solution of a stated closed form and
`test/design-physics.test.js` solves each one from scratch** — Newton on
`cos x · cosh x = 1` for the beam, the Bessel integral and McMahon's expansion
for the round head — and checks the result against the ratios the panel
prints. Two results came out of writing those tests rather than out of reading
a source, and both belong in the engine's own tests, where they will guard the
numbers that actually ship:

- **The beam ratio quoted everywhere as 2.756 is a truncation of 2.75654, not
  a rounding of it.** Correctly rounded it is 2.757. Computing it rather than
  quoting it is the only way that stays true.
- **An undercut bar has no closed form.** Its ratios are a maker's tuning
  target, which is why it is marked in the warning colour as a tuning target
  against the other seven's closed forms, and why the two published values for
  its third partial are offered as a builder's choice rather than averaged
  into a bar nobody has built. It carries the further caveat that the mode
  shapes used for its node positions are still the uniform bar's, because the
  arch moves those too and nothing describes the cut.

The two air columns get the same treatment from the other direction: their
closed forms are the two ends of Opening and are exactly right only there, so
with the control anywhere between, the panel says so and the object's name
stays where it was rather than flipping to the other one.

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
- **The Select strip loses its paragraph below 660 px of height**, keeping its
  three figures, which are the argument.
- **The deck's second lines go below 640 px**, and the object's description
  and the display's legend below 560. Every one of them is still somewhere:
  the deck's on each control's tooltip, the object's in the browse view.
- **The ring-time lane is dropped when the plot is under 120 px**, and the
  legend says so.

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
npm test          # the design-mode equations, solved from scratch
npm run previews  # rebuild the browse view's ratio table from them
```

`npm test` does not test the plug-in — the engine's own tests guard what
ships. It guards `src/dev/physics/`, so that the thing anybody looks at while
designing is not quietly wrong.

---

## Files

| file | what |
|---|---|
| `objects.js` | the catalogue: eight objects, what each is, what it is for, and the equation its series is cited from. No arithmetic. |
| `previews.js` | a generated ratio table for the browse view's rows. Data, not a computation; rebuild with `npm run previews`. |
| `composables/useResonator.js` | the parameter handles, the three streams read by field name, the override table, the published `uses` lookup, `WINDOW_MIN`. No physics. |
| `dev/manifest.js` | **dev only** — the contract, and the stand-in that fills the three streams so the page renders without a plug-in |
| `dev/physics/` | **dev only** — the equations behind that stand-in |
| `components/ObjectBar.vue` | what is loaded, and the way in to changing it |
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

Every stream is optional and every reader says so: a build that has not got as
far as publishing its own partial list renders the series from the equations
and prints which, rather than a blank page or a lie.
