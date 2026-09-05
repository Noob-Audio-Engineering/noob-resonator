// Read a running bridge's manifest and hold it to every claim the panel makes
// on screen.
//
//   node tools/contract.mjs 4246
//
// Checked against the wire rather than against the design manifest, which is
// the only version of this check worth having: a page and a stand-in written
// by the same hand agree with each other by construction. Three of the faults
// this project has caught were two internally consistent halves disagreeing
// with each other, and each of them was invisible until something read the
// live one.
const port = process.argv[2] || '4246';
const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
ws.binaryType = 'arraybuffer';

let bad = 0;
const fail = (m) => {
  bad++;
  console.log(`  FAIL  ${m}`);
};
const ok = (m) => console.log(`  ok    ${m}`);

setTimeout(() => {
  console.log('no manifest');
  process.exit(1);
}, 8000);

ws.onmessage = (ev) => {
  if (typeof ev.data !== 'string') return;
  const m = JSON.parse(ev.data);
  if (m.t !== 'manifest') return;
  check(m);
  ws.close();
  console.log(bad === 0 ? '\nCONTRACT CLEAN' : `\n${bad} CONTRACT PROBLEM(S)`);
  process.exit(bad === 0 ? 0 : 1);
};

function check(m) {
  const params = m.params || m.manifest?.params || [];
  const meta = m.meta || m.manifest?.meta || {};
  const ids = new Set(params.map((p) => p.id));
  const spec = new Map(params.map((p) => [p.id, p]));

  console.log('=== object table ===');
  const objects = meta.objects || [];
  console.log(`  ${objects.length} objects`);
  for (const o of objects) {
    const unknown = (o.uses || []).filter((id) => !ids.has(id));
    if (unknown.length) fail(`${o.label} names controls this build does not publish: ${unknown.join(', ')}`);
    if (typeof o.id !== 'number') fail(`${o.label} is keyed by ${JSON.stringify(o.id)} rather than by index`);
  }
  if (!objects.some((o) => (o.uses || []).includes('mode_budget'))) fail('no object uses mode_budget');
  else ok('every bank object names mode_budget, and every named control exists');
  for (const o of objects) {
    const disc = /round/i.test(o.label);
    if (disc && o.coords !== 'polar') fail(`${o.label} is a disc and its coords are ${o.coords}`);
  }
  ok(`coords: ${objects.map((o) => `${o.label}=${o.coords}`).join(', ')}`);

  console.log('\n=== presets ===');
  const presets = meta.presets || [];
  console.log(`  ${presets.length} factory presets, version ${meta.preset_version}, store key ${JSON.stringify(meta.presets_key)}`);
  if (meta.preset_version !== 1) fail(`the page writes version 1 and the engine publishes ${meta.preset_version}`);
  if (meta.presets_key !== 'presets') fail(`the page reads the store key "presets" and the engine names ${JSON.stringify(meta.presets_key)}`);

  const EXCLUDED = new Set(['bypass', 'src_kind', 'src_level', 'src_freq']);
  let valueCount = 0;
  for (const p of presets) {
    for (const k of ['v', 'name', 'group', 'description', 'values', 'modes']) {
      if (!(k in p)) fail(`preset ${JSON.stringify(p.name)} has no ${k}`);
    }
    if (!Array.isArray(p.modes)) fail(`preset ${JSON.stringify(p.name)} has a modes that is not an array`);
    for (const [id, v] of Object.entries(p.values || {})) {
      valueCount++;
      const s = spec.get(id);
      if (!s) {
        fail(`preset ${JSON.stringify(p.name)} sets ${id}, which this build does not publish`);
        continue;
      }
      if (EXCLUDED.has(id)) fail(`preset ${JSON.stringify(p.name)} sets ${id}, which the page never applies`);
      if (typeof v !== 'number' || !Number.isFinite(v)) fail(`${p.name}.${id} is ${JSON.stringify(v)}`);
      else if (s.min != null && s.max != null && (v < s.min - 1e-6 || v > s.max + 1e-6)) {
        fail(`${p.name}.${id} = ${v} is outside ${s.min}..${s.max}`);
      }
    }
    for (const e of p.modes || []) {
      if (!Number.isFinite(e.i)) fail(`${p.name} has a mode edit with no i`);
      for (const f of ['cents', 'db', 'decay']) {
        if (!(f in e)) fail(`${p.name} mode ${e.i}:${e.j} has no ${f}`);
      }
    }
  }
  ok(`${valueCount} preset values, every id published and in range`);

  // The page covers every parameter but the four it excludes; a preset that
  // named fewer would leave a control behind from whatever loaded before it.
  const covered = params.filter((p) => !EXCLUDED.has(p.id)).map((p) => p.id);
  for (const p of presets) {
    const missing = covered.filter((id) => !(id in (p.values || {})));
    if (missing.length) fail(`preset ${JSON.stringify(p.name)} does not set: ${missing.join(', ')}`);
  }
  ok(`each preset sets all ${covered.length} covered parameters`);

  // The A/B pair the browser finds structurally.
  const pairs = [];
  for (let a = 0; a < presets.length; a++) {
    for (let b = a + 1; b < presets.length; b++) {
      const keys = new Set([...Object.keys(presets[a].values), ...Object.keys(presets[b].values)]);
      const differ = [...keys].filter((k) => Math.abs(Number(presets[a].values[k]) - Number(presets[b].values[k])) > 1e-9);
      if (differ.length === 1) pairs.push(`${presets[a].name} / ${presets[b].name} on ${differ[0]}`);
    }
  }
  if (!pairs.length) fail('no preset pair differs in exactly one control, so the browser has nothing to A/B');
  else ok(`pairs found structurally: ${pairs.join(' · ')}`);

  console.log('\n=== streams ===');
  for (const s of m.streams || []) {
    console.log(`  ${s.id.padEnd(9)} layout ${JSON.stringify(s.meta?.layout || '')}`);
  }
  const modes = (m.streams || []).find((s) => s.id === 'modes');
  const info = (m.streams || []).find((s) => s.id === 'info');
  /**
   * **The fields the page reads, present — not the layout, identical.**
   *
   * This asserted byte equality once and failed the moment the engine appended
   * `voice_available`, which is a field arriving rather than a contract
   * breaking. That contradicted the page's own design: every stream field is
   * looked up by name precisely so the engine can add one without breaking a
   * panel, and `test/layout.test.js` pins that a longer layout does not break a
   * shorter reader. A probe that forbids growth is a probe that will be
   * silenced the first time it is right about something.
   */
  const want = {
    modes: ['i', 'j', 'hz', 'db_l', 'db_r', 't60_s', 'db_bare', 'base_hz'],
    info: ['modes_used', 'modes_available', 'crossover_hz', 'tail_db', 'limit_gr_db', 'inharm_b',
           'column_m', 'loop_ms', 'open_hz', 'engine', 'build', 'f0_hz', 'ceiling_hz'],
  };
  for (const [id, names] of Object.entries(want)) {
    const stream = id === 'modes' ? modes : info;
    // A field may be declared as `name` or as `name[n]` for a run of slots.
    const have = new Set(String(stream?.meta?.layout || '').split(',').map((n) => n.trim().replace(/\[\d+\]$/, '')));
    const missing = names.filter((n) => !have.has(n));
    if (missing.length) fail(`the ${id} layout is missing what the page reads: ${missing.join(', ')}`);
    else {
      const extra = [...have].filter((n) => n && !names.includes(n));
      ok(`${id}: every field the page reads is present` + (extra.length ? ` (and ${extra.join(', ')}, which it does not read yet)` : ''));
    }
  }
}
