#!/usr/bin/env node
/**
 * Regenerate the two partial tables the page ships, **from the engine**.
 *
 *   node tools/gen-previews.mjs              # builds and runs the engine
 *   node tools/gen-previews.mjs --from x.csv # a dump you already have
 *
 * It writes:
 *
 * * `src/previews.js` — forty ratios per object, for the browse view's rows.
 *   The browser draws ten objects at once and only one of them is loaded, so
 *   those rows cannot read the `modes` stream and cannot solve anything
 *   either. They read a table, and this is where the table comes from.
 * * `src/dev/series-table.js` — the same numbers with their mode indices, for
 *   the design-mode stand-in, so a page opened with no plug-in running draws
 *   the engine's own series rather than a second implementation of it.
 *
 * **The engine is the authority and this makes that literal.** The previous
 * version of this script generated both from the page's own arithmetic, which
 * meant two implementations of every eigenvalue problem in the product and
 * nothing checking they agreed — and they had already drifted once, silently,
 * when the engine appended two objects the page had never heard of. Reading
 * `benchmark --dump series` closes that: the table cannot disagree with the
 * engine because it *is* the engine's output.
 *
 * The dump is `id,object,i,j,ratio` with a header, one row per partial: the
 * object's **index**, which is what a saved project stores and so what
 * identifies an object on the wire, its display name, its two mode indices and
 * its ratio to the fundamental. Both keys are read and cross-checked against
 * the catalogue, so an object appended, renamed or reordered on either side
 * stops this script rather than quietly shifting every row by one.
 *
 * It carries the air columns too, measured off the running delay loop rather
 * than assumed, which is why every row this writes is engine-sourced.
 *
 * One thing it does not carry: **the controls that move a series.** The dump
 * is taken at default aspect with no inharmonicity and one setting of Opening,
 * so it is each object's *bare* series. Ratio, Bar Tuning, Opening and Inharm
 * are applied on top of it in design mode, from the same laws the engine uses.
 */
import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { OBJECTS } from '../src/objects.js';

/** Enough partials to show the shape of a series at forty-six pixels tall. */
const N_PREVIEW = 40;

/**
 * How many partials the design-mode table keeps per object.
 *
 * Past this the stand-in reports its available count as **not computed**
 * rather than as a wall — a limit on a table is not a property of an object,
 * and a page that draws one as the other is the false-ceiling bug again.
 */
const N_TABLE = 256;

// ---------------------------------------------------------------------------

const args = process.argv.slice(2);
const fromIx = args.indexOf('--from');

function dump() {
  if (fromIx >= 0) return readFileSync(args[fromIx + 1], 'utf8');
  return execFileSync('cargo', ['run', '--release', '--quiet', '--bin', 'benchmark', '--', '--dump', 'series'], {
    cwd: new URL('../..', import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1'),
    encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024,
  });
}

/**
 * `id,object,i,j,ratio` into `{ catalogue id: [{ i, j, r }] }`, each sorted by
 * frequency.
 *
 * **Keyed by the engine's index and checked against the engine's label.** The
 * index is what a saved project stores, so it is what identifies an object;
 * the label is the second opinion, and demanding that both agree with the
 * catalogue is what turns "the engine appended two objects" from a silent
 * off-by-two into a failed build.
 */
function parse(csv) {
  const out = {};
  for (const line of csv.split(/\r?\n/)) {
    if (!line || line.startsWith('id,')) continue;
    const [ix, label, i, j, r] = line.split(',');
    const o = OBJECTS[Number(ix)];
    if (!o) throw new Error(`the dump has an object at index ${ix} and the catalogue stops at ${OBJECTS.length - 1}`);
    if (o.label !== label) {
      throw new Error(`index ${ix} is "${label}" to the engine and "${o.label}" to the catalogue`);
    }
    (out[o.id] ||= []).push({ i: Number(i), j: Number(j), r: Number(r) });
  }
  // **The dump has to arrive in frequency order, and this checks rather than
  // assumes.** It did not, once: the walk is index-major, so on a membrane the
  // whole `j` sweep for `i = 1` ran to five hundred and sixty-five partials
  // before `i = 2` began, and the cap kept three complete families and part of
  // a fourth — dropping every mode with `i >= 5` from the seventeenth partial
  // up, both halves of two degenerate pairs among them. A prefix of a sorted
  // list is complete; a prefix of a walk is not, and nothing in the file says
  // which one you have.
  //
  // It cannot be repaired here either, because the smallest missing ratio
  // belongs to a family that is not in the file. So this refuses, and the
  // engine sorts.
  for (const [id, list] of Object.entries(out)) {
    for (let k = 1; k < list.length; k++) {
      if (list[k].r < list[k - 1].r - 1e-9) {
        throw new Error(
          `the dump is not in frequency order for ${id}: partial ${k} sits at ${list[k].r} below ` +
            `${list[k - 1].r}. A prefix of an index-ordered walk is missing whole mode families from ` +
            'the bottom of the series. Sort by ratio before taking, and dump again.',
        );
      }
    }
  }
  return out;
}

const table = parse(dump());

const missing = OBJECTS.filter((o) => !table[o.id]);
if (missing.length) throw new Error(`the dump has no series for: ${missing.map((o) => o.id).join(', ')}`);
const extra = Object.keys(table).filter((id) => !OBJECTS.some((o) => o.id === id));
if (extra.length) throw new Error(`the engine has objects the catalogue does not: ${extra.join(', ')}`);

/** Six significant figures is plenty to draw a forty-six pixel row with. */
const six = (v) => Number(v.toPrecision(6));

/**
 * The design table keeps every digit the dump carried, because the tests feed
 * these ratios back through the equations that define them — `cos β · cosh β`
 * for the bars, the Bessel integral for the round head — and six figures is
 * not enough to tell a real disagreement from a rounding of the table.
 */
const full = (v) => v;
/**
 * Whether a row's numbers came off the engine.
 *
 * All ten do now — the dump measures the air columns off the running delay
 * loop rather than assuming the closed form. The distinction is kept because
 * "these are the engine's partials" and "these are the equation's partials"
 * are not the same claim, and the day one row stops being the first the face
 * should say so rather than the code quietly forgetting.
 */
const engineSourced = () => true;

// ---------------------------------------------------------------------------
// src/previews.js
// ---------------------------------------------------------------------------

const rows = OBJECTS.map((o) => {
  const r = table[o.id].slice(0, N_PREVIEW).map((m) => six(m.r));
  return `  ${o.id}: [${r.join(', ')}],`;
}).join('\n');

const sources = OBJECTS.map((o) => `  ${o.id}: '${engineSourced(o.id) ? 'engine' : 'closed form'}',`).join('\n');

writeFileSync(
  new URL('../src/previews.js', import.meta.url),
  `/**
 * Characteristic partial ratios, one row per object, for the browse view's
 * previews — **a generated table, not a computation.**
 *
 * The panel renders and does not compute: every partial it draws for the
 * loaded object arrives on the \`modes\` stream. A browser showing every object
 * at once cannot do that, because only one of them is loaded, and solving ten
 * eigenvalue problems in the front end to draw ten thumbnails is exactly what
 * this architecture forbids. So the rows read a table.
 *
 * **These are the shape of each series, not your settings.** They are taken at
 * reference settings — default aspect, no inharmonicity — so a row shows what
 * an object *is* rather than what it would sound like at your current damping,
 * which is the comparison the browser exists to make anyway.
 *
 * **Generated from the engine** by \`tools/gen-previews.mjs\`, out of
 * \`cargo run --release --bin benchmark -- --dump series\`. Rerun it when the
 * engine's series move. \`PREVIEW_SOURCE\` says, per row, whether the numbers
 * came off the engine or from the closed form the catalogue cites — the air
 * columns are the second, because a waveguide has no mode list to dump.
 *
 * @type {Record<string, number[]>}
 */
export const PREVIEW_RATIOS = {
${rows}
};

/**
 * Where each row's numbers came from. Printed in the browser, because "these
 * are the engine's partials" and "these are the equation's partials" are not
 * the same claim and the face does not get to blur them.
 *
 * @type {Record<string, 'engine'|'closed form'>}
 */
export const PREVIEW_SOURCE = {
${sources}
};
`,
);

// ---------------------------------------------------------------------------
// src/dev/series-table.js
// ---------------------------------------------------------------------------

const tableRows = OBJECTS.map((o) => {
  const list = table[o.id].slice(0, N_TABLE);
  const body = list.map((m) => `[${m.i},${m.j},${full(m.r)}]`).join(',');
  return `  ${o.id}: [${body}],`;
}).join('\n');

writeFileSync(
  new URL('../src/dev/series-table.js', import.meta.url),
  `/**
 * Every object's bare partial series, with mode indices — **development only**,
 * and generated from the engine by \`tools/gen-previews.mjs\`.
 *
 * This is what the design-mode stand-in walks. It exists so that a page opened
 * with no plug-in running draws the engine's own ratios rather than a second
 * implementation of them: before this table, the page solved each eigenvalue
 * problem again in JavaScript, which is two of everything and nothing checking
 * they agree.
 *
 * Each entry is \`[i, j, ratio]\`, sorted by frequency. \`j\` is 0 where the
 * object has one mode index. The series are **bare**: default aspect, no
 * inharmonicity, the marimba at its default tuning. Ratio, Bar Tuning and
 * Inharm are applied on top in \`physics/\`, from the same one-line laws the
 * engine uses.
 *
 * **The list stops at ${N_TABLE} partials, and that is a limit on this table
 * rather than a property of any object.** Where the stand-in runs off the end
 * it publishes its available count as not computed, because a wall the page
 * invented is the exact failure this project keeps catching.
 *
 * @type {Record<string, [number, number, number][]>}
 */
export const SERIES_TABLE = {
${tableRows}
};

/** How many partials the table holds per object. */
export const TABLE_LIMIT = ${N_TABLE};
`,
);

const counts = OBJECTS.map((o) => `${o.id} ${Math.min(table[o.id].length, N_TABLE)}`).join(', ');
console.log(`wrote src/previews.js and src/dev/series-table.js — ${counts}`);
