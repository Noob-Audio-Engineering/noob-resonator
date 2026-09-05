# Eight things that check the page, and what each one can prove

These are in the repository rather than in a session scratchpad for the reason
the engine's `tools/README.md` gives: a check that is lost is a check nobody
runs. Each exists because something here was believed on evidence that did not
distinguish the case.

Three of them drive a real browser and so need Playwright, which is
**deliberately not a dependency of this package** — it pulls a browser download
behind it, and the page's own tests (`npm test`) run in node with none. Install
it when you want to run them:

```sh
npm i -D playwright
```

Set `RES_CHROMIUM` to an executable if Playwright's own download is not the
browser you want driven.

## `gen-previews.mjs` — the partial tables, from the engine

```sh
node tools/gen-previews.mjs                # builds and runs the engine
node tools/gen-previews.mjs --from x.csv   # a dump you already have
```

Writes `src/previews.js`, the forty ratios per object the browse rows draw,
and `src/dev/series-table.js`, the same numbers with their mode indices for the
design-mode stand-in to walk. Both come out of
`cargo run --release --bin benchmark -- --dump series`, so the page cannot
disagree with the engine about where a partial lands: it is not a second
implementation, it is the engine's output.

**It refuses rather than guessing.** It stops if the dump names an object the
catalogue does not have, if the catalogue has one the dump does not, if an
index and a label disagree, or **if the rows are not in frequency order** —
that last one because they were not, once. The walk is index-major, so a
membrane spends five hundred and sixty-five rows on `i = 1` before it reaches
`i = 2`, and a plain `take` kept three complete families and dropped every mode
with `i >= 5`: both halves of two degenerate pairs, missing from the
seventeenth partial up, in a file that looked complete. A prefix of a sorted
list is complete and a prefix of a walk is not, and nothing inside the file
says which one you have.

## `page-sweep.mjs` — every view at every size, and the built bundle

```sh
node tools/page-sweep.mjs http://localhost:5173/ http://localhost:4173/ ./shots
```

Opens the panel, the browse layer and the preset layer at 900 × 520,
1100 × 620 and 1900 × 1000; walks every object and screenshots each; then loads
the **built bundle** from a static server. It fails on any console error, page
error, failed request, horizontal scroll, or `NaN`, `undefined` and
`[object Object]` reaching the screen.

**The production pass is the point.** A dev server that has served a stale
module graph cannot be trusted for a one-off measurement, and design mode is
where a page looks perfect against a manifest it wrote itself. Twice, states
were measured here that did not exist.

## `preset-e2e.mjs` — presets against a running plug-in

```sh
node tools/preset-e2e.mjs http://localhost:5173/ ./shots
```

Refuses to run in design mode, which is exactly where a preset system would
look perfect and prove nothing. Against a real bridge it checks that the
factory list arrives from the engine's manifest and draws grouped; that the A/B
pair loads with every *knob* identical and Selection alone moved; that a whole
preset lands on the face in plain units; that `Hand Bell` arrives with its five
retuned partials marked and that a preset with an empty `modes` clears them
again; that moving a control raises the edited dot; and that a saved preset
survives a page reload, which is what proves it is in the plug-in state rather
than in the page. It removes the preset it saved.

## `contract.mjs` — the live manifest against what the panel claims

```sh
node tools/contract.mjs 4246
```

Needs no browser. It connects to a running bridge and holds the manifest to
every claim the panel makes on screen: that every id in every object's `uses`
is a parameter this build publishes, that both discs report polar contact
coordinates, that every preset value names a published id and sits inside its
range, that each preset sets all forty covered parameters, that some pair
differs in exactly one control so the browser has an A/B to find, and that both
stream layouts are exactly the strings the page reads by name.

**Against the wire, never against the design manifest.** A page and a stand-in
written by the same hand agree with each other by construction. Three of the
faults caught here were two internally consistent halves disagreeing — an object
table keyed by index while the page looked up by string, a `uses` array naming
a parameter that had been renamed, contact coordinates the audio thread did not
use — and every one of them was invisible until something read the live one.

## `follows.mjs` — does the bank keep up while a knob is turning?

```sh
node tools/follows.mjs http://localhost:5173/ 3
```

Sweeps Tune from the bottom of its travel with no pause long enough to count as
letting go, sampling the control, the display's axis and the lowest partial the
engine is actually publishing.

**The failure it looks for is the one that hides.** A bank frozen on its old
mode set still *looks* like it is following, because the control readout moves
whether or not the audio thread rebuilt anything — and the engine's own author
flagged that restarting the search on every settings change would cause exactly
that under host automation. So this compares the knob against the partials, and
then the two halves of the picture against each other: **a partial drawn at
1630 Hz under a ruler whose 1x is 4000 Hz appears at 0.4x**, and a reader has no
way to know they are looking at two different moments.

The knob leading the engine mid-gesture is correct and expected — the control is
what you asked for, the display is what is being synthesised. What is not
correct is the axis and the partials coming from different states, which this
catches and which the page cannot fix on its own.

## `pairing.mjs` — does every partial sit where its own ratio says?

```sh
node tools/pairing.mjs http://localhost:5173/
```

Samples the display as fast as it will answer, through a fast Tune sweep and
after it, and reports how often the lowest partial does *not* land on the 1x
line. **It reports a rate rather than passing or failing**, because the number
that matters is how often and for how long.

The two halves of that picture arrive on different streams — `info` every block,
the mode table only when it changes — so a page holding the newest ruler and the
last bars it received draws one moment's frequencies against another moment's
fundamental. The engine takes `f0_hz` at the instant it builds the rows now,
which removed the systematic version; the steady state is exact and about 0.3%
of frames during a fast gesture are not.

**That remainder is a seam rather than a browser artefact**, and the engine's
author was right to correct me on it: `f0_hz` now describes the table's moment,
but it still does not travel *in the same frame* as the bars, so two streams are
still being read together. It is closable at the engine — a ruler on the modes
frame, or a ratio per row — and deliberately is not, because nobody has measured
the arrival order directly and the one page-side fix that was tried made it
worse. The option is on the record so that it stays a decision rather than
becoming an oversight.

**A page-side attempt to close that remainder is what this probe disproved.**
Pinning the ruler to whatever the fundamental was when the bars last changed
looked obviously right and **doubled the rate**, from 0.33% to 0.73% of samples
— so the ordering is not the one that reasoning suggested, and the change was
reverted rather than shipped on the strength of the argument for it.

## `ceiling.mjs` — are the held partials drawn as what they are?

```sh
node tools/ceiling.mjs http://localhost:5173/
```

A pitch move can push a partial past Nyquist, and the engine **clamps it there
rather than letting it alias** — so it sounds at the ceiling and not where the
object's ratios put it. Several arrive at once and land on the same pixel,
which drew as one bright partial at the top of the series instead of the twenty
it was.

Four cases, and the ones that keep the first honest are the rest: with the
oscillator pitching a dense object up, the stack should be marked, dashed and
counted; **with it off and the object at rest, nothing should be marked at
all**; when the bank's ceiling and the clamp fall on one frequency both captions
must say so; and when they are different frequencies neither may claim they
coincide. The last two are checked on every sampled frame rather than by
catching the rare state by hand.

**It sets up its own conditions.** The first version assumed the oscillator was
already on from whatever had been done to the plug-in beforehand, and against a
freshly started one it reported "no stack" — which is the answer a broken
marking would also give.

**The detection is the interesting part.** Testing `hz` against Nyquist found
nothing — the clamp sits at 23.52 kHz on a 24 kHz band, so a threshold tight
enough to mean "at the ceiling" missed every one, and one loose enough to catch
them would have been a number chosen to make the test pass. What is observable
is that several distinct modes are sharing one frequency, which cannot be true
of an object: two modes do not sound at one pitch, bar the degenerate pairs a
square has, and those come in twos and fours anywhere in the series rather than
three-deep at its top.

## `extremes.mjs` — every control at both ends, on every object

```sh
node tools/extremes.mjs http://localhost:5173/
```

Drives all thirty knobs to their minimum and then their maximum on each object
and looks for a number that should not be there — in the text *and* in the SVG,
because a `NaN` in a path's `d` draws nothing at all and never says why.

**Then it loads a sane preset and checks the instrument comes back**, which is
the half that matters. A panel with nothing drawn while the fundamental sits
above Nyquist is correct. A panel still empty once every parameter has been set
back is an engine that did not recover, and it found one: Tune, Transpose and
Fine together at their minimum on a Membrane put the fundamental at 1.2 Hz,
where the mode search sets itself a task it cannot finish and never resets —
so the plug-in never rings again, whatever you do to it afterwards.
