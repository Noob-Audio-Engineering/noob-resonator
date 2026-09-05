// Are the partials the engine holds at the ceiling drawn as what they are?
//
//   node tools/ceiling.mjs http://localhost:5173/
//
// Two cases, and the second is the one that keeps the first honest: with the
// oscillator pitching a dense object up, a stack should be marked and counted;
// with it off and the object at rest, nothing should be marked at all.
const { chromium } = await import('playwright').catch(() => {
  console.error('This probe drives a real browser and needs Playwright:');
  console.error('  npm i -D playwright');
  process.exit(2);
});
/** The browser to drive. Set `RES_CHROMIUM` to override Playwright's own. */
const EXE = process.env.RES_CHROMIUM || undefined;
const b = await chromium.launch(EXE ? { executablePath: EXE } : {});
const p = await b.newPage({ viewport: { width: 1500, height: 950 } });
p.on('pageerror', (e) => console.log('PAGEERROR', String(e).slice(0, 200)));
await p.goto(process.argv[2], { waitUntil: 'networkidle' });
await p.waitForTimeout(2500);
await p.evaluate(async () => {
  [...document.querySelectorAll('button')].find((x) => /change resonator/i.test(x.textContent || ''))?.click();
  await new Promise((r) => setTimeout(r, 400));
  document.querySelectorAll('.browse__card')[3]?.click();
});
await p.waitForTimeout(1200);
await p.evaluate(() => {
  const k = [...document.querySelectorAll('.knob')].find((x) => /^tune$/i.test((x.querySelector('.knob__label')?.textContent || '').trim()));
  k?.querySelector('.knob__dial')?.focus();
});
await p.keyboard.press('End');
await p.waitForTimeout(1500);
let seen = 0;
for (let i = 0; i < 300; i++) {
  const s = await p.evaluate(() => ({
    label: document.querySelector('.g-held text')?.textContent?.trim() || null,
    note: document.querySelector('.md__prov.is-note')?.textContent?.trim() || null,
    dashed: document.querySelectorAll('.g-handles line.held').length,
  }));
  if (s.label) { console.log('marker :', s.label); console.log('dashed :', s.dashed, 'handles'); console.log('note   :', s.note); seen++; break; }
  await p.waitForTimeout(25);
}
if (!seen) console.log('no stack seen in 300 samples');
// And with the oscillator off, nothing should be marked.
await p.evaluate(() => {
  const g = [...document.querySelectorAll('.deck__group')].find((x) => /^LFO/.test((x.querySelector('.deck__head')?.textContent || '').trim()));
  const stack = [...(g?.querySelectorAll('.deck__stack') || [])].find((s) => /LFO/.test(s.querySelector('.deck__cap')?.textContent || ''));
  stack?.querySelector('button')?.click();
});
await p.evaluate(() => {
  const k = [...document.querySelectorAll('.knob')].find((x) => /^tune$/i.test((x.querySelector('.knob__label')?.textContent || '').trim()));
  k?.querySelector('.knob__dial')?.focus();
});
await p.keyboard.press('Home');
await p.waitForTimeout(3000);
console.log('at rest:', await p.evaluate(() => ({
  marker: document.querySelector('.g-held text')?.textContent?.trim() || '(none)',
  note: document.querySelector('.md__prov.is-note')?.textContent?.trim() || '(none)',
})));
await b.close();
