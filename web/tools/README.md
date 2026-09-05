# Four things that check the page, and what each one can prove

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
