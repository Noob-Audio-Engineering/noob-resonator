// Does every partial sit where its own ratio says, on the ruler drawn beside
// it? Sampled as fast as the page will answer, through a fast Tune sweep and
// after it.
//
//   node tools/pairing.mjs http://localhost:5173/
//
// **The two halves of this picture arrive on different streams.** `info` goes
// out every block and the mode table only when it changes, so a page holding
// the newest ruler and the last bars it received can draw one moment's
// frequencies against another moment's fundamental. The engine now takes
// `f0_hz` at the instant it builds the rows, which removed the systematic
// version of this; what is left is a render-timing transient during rapid
// change, and this is what measures it rather than arguing about it.
//
// It reports a rate rather than passing or failing, because the number that
// matters is how often and for how long, not whether it ever happens.
const { chromium } = await import('playwright').catch(() => {
  console.error('This probe drives a real browser and needs Playwright:');
  console.error('  npm i -D playwright');
  process.exit(2);
});
/** The browser to drive. Set `RES_CHROMIUM` to override Playwright's own. */
const EXE = process.env.RES_CHROMIUM || undefined;
const b = await chromium.launch(EXE ? { executablePath: EXE } : {});
const p = await b.newPage({ viewport: { width: 1400, height: 900 } });
await p.goto(process.argv[2], { waitUntil: 'networkidle' });
await p.waitForTimeout(2500);
await p.evaluate(async () => {
  [...document.querySelectorAll('button')].find((x) => /change resonator/i.test(x.textContent || ''))?.click();
  await new Promise((r) => setTimeout(r, 400));
  document.querySelectorAll('.browse__card')[2]?.click();
});
await p.waitForTimeout(1500);
const sample = () => p.evaluate(() => {
  const tip = document.querySelector('.g-handles line title')?.textContent || '';
  const m = tip.match(/·\s*([\d.]+)\s*(k?)Hz/);
  const lowest = m ? Number(m[1]) * (m[2] === 'k' ? 1000 : 1) : null;
  const ax = (document.querySelectorAll('svg text.hz')[0]?.textContent || '').match(/([\d.]+)\s*(k?)Hz/);
  const axis = ax ? Number(ax[1]) * (ax[2] === 'k' ? 1000 : 1) : null;
  const first = tip.match(/^(Partial|Mode)\s*\(?(\d+)/);
  return { axis, lowest, firstIndex: first ? Number(first[2]) : null };
});
await p.evaluate(() => {
  const k = [...document.querySelectorAll('.knob')].find((x) => /^tune$/i.test((x.querySelector('.knob__label')?.textContent || '').trim()));
  k?.querySelector('.knob__dial')?.focus();
});
await p.keyboard.press('Home');
await p.waitForTimeout(2500);

const rows = [];
const t0 = Date.now();
const gesture = (async () => { for (let i = 0; i < 60; i++) { await p.keyboard.press('ArrowUp'); } })();
while (Date.now() - t0 < 6000) {
  rows.push({ t: Date.now() - t0, ...(await sample()) });
}
await gesture;
await p.waitForTimeout(3000);
for (let i = 0; i < 6; i++) { rows.push({ t: Date.now() - t0, ...(await sample()) }); }

const off = rows.filter((r) => r.axis && r.lowest && r.firstIndex === 1 && Math.abs(r.lowest / r.axis - 1) > 0.01);
console.log(`${rows.length} samples, ${off.length} where partial 1 does not sit on 1x`);
console.log('worst:', off.slice().sort((a, c) => Math.abs(a.lowest / a.axis - 1) - Math.abs(c.lowest / c.axis - 1)).slice(-3)
  .map((r) => `${r.t}ms ${r.lowest}Hz on a ${r.axis}Hz ruler = ${(r.lowest / r.axis).toFixed(3)}x`).join(' | ') || 'none');
console.log('last 6 (after letting go):', rows.slice(-6).map((r) => (r.lowest / r.axis).toFixed(3)).join(' '));
await b.close();
