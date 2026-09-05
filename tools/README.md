# Three probes, and what each one can prove

Each of these exists because something here was believed on evidence that did
not distinguish the case. They are kept in the repository rather than in a
session scratchpad because a check that is lost is a check nobody runs.

## `physics_probe.py` — the partial series, from outside

```sh
cargo run --release --bin benchmark -- --dump series > series.csv
python tools/physics_probe.py --compare series.csv
```

Computes every object's partial series again from the published formulae, in a
language that cannot link against this one, and diffs it against what the
engine produces. Bessel functions come from the integral representation
`J_m(x) = (1/π)∫₀^π cos(mτ − x sin τ)dτ`; beam eigenvalues from bisection on
`cos β ∓ sech β = 0`; the membrane and plate lattices from their closed forms.
Every algorithm differs from the engine's, so a mistake would have to be made
twice, in two languages, in two different ways.

Run with no arguments it checks **itself** first, against Leissa's Tables 4.23
and 4.39, Abramowitz and Stegun's Table 9.5, Russell's circular-membrane
ratios and Lehtonen's inharmonicity figures, and prints the mode-shape
normalisation integrals. Only then is it allowed near anything of ours.

**Its independence is in the implementation, not the directory.** It was in the
scratchpad at first for that reason, which confused location with dependence:
sharing no line of code is what makes it a second opinion, and a scratchpad
only makes it a second opinion nobody has after tonight.

The dump is `id,object,i,j,ratio` behind a header row: the object's index in
the engine's own list, which is what the `type` parameter and every preset
store, and the label the manifest publishes. It used to print the Rust variant
name, which is the one identifier in the whole contract nothing else reads and
we are free to rename — so anything keying off it was keying off the thing most
likely to move. The probe now checks that each label arrives under one index
and no more.

Worst disagreement to date: **0.0001 cents** over some five thousand partials
across the beam, the tine, the string, both membranes and both plates, and
**0.53 cents** over the first sixteen partials of the stopped air column.

The air columns are the one place a disagreement is expected, and the probe
says so rather than hiding it. They are a delay loop with a third-order
Lagrange fractional delay whose phase error grows with frequency, so agreement
with an ideal `2n − 1` column is a band-limited claim: it is held to the same
sixteen partials and the same one cent the benchmark publishes, and the drift
above that — 2.7 cents by the fifty-third partial, near 12 kHz — is printed
beside it as a measurement rather than asserted as a pass.

## `manifest_probe.mjs` — did the page ever connect?

```sh
node tools/manifest_probe.mjs 4246
```

Connects the way the page's own client does, reports whether the manifest
arrives and how long it took, and checks it for duplicate parameter ids and
non-finite ranges.

It exists to separate **"never connected"** from **"connected and choked"**,
which are indistinguishable from a blank window and which cost two people an
hour of guessing between them.

## `page_probe.mjs` — does the built bundle work against a real bridge?

```sh
node tools/page_probe.mjs http://127.0.0.1:4246/ shot.png
```

Loads a running bridge's page in headless Chromium and reports page errors,
failed requests, console output, how much the panel actually rendered, and a
screenshot.

**This is the combination nothing else here tests.** The panel's dev server
never loads `dist`, and until this existed the standalone's only client had
been that dev server — so the built bundle and a real engine had never met
outside a DAW.

It reuses the browser already at `AppData\Local\ms-playwright` rather than
downloading another, because the disk on this machine has been full once
already. It finds that browser by looking for the newest
`chromium_headless_shell-*` rather than by naming a build number, which would
go stale the first time playwright updated.

**It needs the playwright package as well as the browsers, and it is
deliberately not a dependency of the page's build** — the whole point is that
it comes at the built bundle from outside. `npm install --no-save playwright`
anywhere and run the probe from there, or point `PLAYWRIGHT_DIR` at a
directory whose `node_modules` has it. With neither, it says so and exits 2
rather than throwing a module-resolution stack trace: a probe whose failure
mode looks like a bug in the thing being probed is worse than no probe.

## What none of them can do

**Nothing here validates that the embedded web view navigates.** A plug-in run
through nih-plug's standalone wrapper parents its view in a window that does
not drive it the way a host's does, so "no client connected" from that rig is
**no signal at all** — it reproduces on plug-ins that work perfectly in a DAW.
That was established by running the same rig against a plug-in known to work,
and it is written here because the earlier conclusion looked like proof and was
not.
