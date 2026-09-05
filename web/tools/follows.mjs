// Does the bank keep up while a knob is turning, or only once you let go?
//
// The failure this looks for is the one that hides: a bank frozen on its old
// mode set *while the panel appears to follow*, because the control readout
// moves whether or not the audio thread rebuilt anything. So it compares the
// Tune control against the lowest partial the engine is actually publishing,
// during the gesture and after it.
//
//   node follow.mjs <pageUrl> <objectCard>
const { chromium } = await import('playwright').catch(() => {
  console.error('This probe drives a real browser and needs Playwright:');
  console.error('  npm i -D playwright');
  process.exit(2);
});

const URL = process.argv[2] || 'http://localhost:5201/';
const CARD = Number(process.argv[3] || 3);
/**
 * The browser to drive. Set `RES_CHROMIUM` to override Playwright's own.
 */
const EXE = process.env.RES_CHROMIUM || undefined;

let bad = 0;
const fail = (m) => {
  bad++;
  console.log(`  FAIL  ${m}`);
};
const ok = (m) => console.log(`  ok    ${m}`);

const browser = await chromium.launch(EXE ? { executablePath: EXE } : {});
const page = await browser.newPage({ viewport: { width: 1400, height: 900 } });
page.on('pageerror', (e) => fail(`page error: ${String(e).slice(0, 160)}`));
await page.goto(URL, { waitUntil: 'networkidle' });
await page.waitForTimeout(2500);

if (await page.evaluate(() => !!document.querySelector('.md__stamp'))) {
  console.log('design mode — nothing to measure without a plug-in');
  await browser.close();
  process.exit(2);
}

await page.evaluate(async (ix) => {
  [...document.querySelectorAll('button')].find((b) => /change resonator/i.test(b.textContent || ''))?.click();
  await new Promise((r) => setTimeout(r, 400));
  document.querySelectorAll('.browse__card')[ix]?.click();
  await new Promise((r) => setTimeout(r, 400));
}, CARD);
await page.waitForTimeout(2500);

const sample = () =>
  page.evaluate(() => {
    const knob = [...document.querySelectorAll('.knob')].find(
      (k) => (k.querySelector('.knob__label')?.textContent || '').trim().toLowerCase() === 'tune',
    );
    const tuneText = (knob?.querySelector('.knob__value-text')?.textContent || '').trim();
    const tm = tuneText.match(/([\d.]+)\s*(k?)Hz/);
    const tune = tm ? Number(tm[1]) * (tm[2] === 'k' ? 1000 : 1) : null;
    const tip = document.querySelector('.g-handles line title')?.textContent || '';
    // "Mode (1, 1) · 1× · 196 Hz · rings ..." or "Partial 1 · 1× · 196 Hz · ..."
    const m = tip.match(/·\s*([\d.]+)\s*(k?)Hz/);
    const lowest = m ? Number(m[1]) * (m[2] === 'k' ? 1000 : 1) : null;
    // What the display calls 1x: the fundamental it is drawing the axis from,
    // which comes off the info stream and may not be the frame the partials
    // came from.
    const axis = [...document.querySelectorAll('svg text.hz')][0]?.textContent || '';
    const am = axis.match(/([\d.]+)\s*(k?)Hz/);
    return {
      object: document.querySelector('.pick__name')?.textContent?.trim(),
      tune,
      lowest,
      axisF0: am ? Number(am[1]) * (am[2] === 'k' ? 1000 : 1) : null,
      build: document.querySelector('.md__settling')?.innerText || 'settled',
    };
  });

// Start from the bottom of Tune's travel, so the gesture always has somewhere
// to go however the previous run left it.
await page.evaluate(() => {
  const knob = [...document.querySelectorAll('.knob')].find(
    (k) => (k.querySelector('.knob__label')?.textContent || '').trim().toLowerCase() === 'tune',
  );
  knob?.querySelector('.knob__dial')?.focus();
});
await page.keyboard.press('Home');
await page.waitForTimeout(2500);

const start = await sample();
console.log(`  ${start.object}: Tune ${start.tune} Hz, lowest partial ${start.lowest} Hz, ${start.build}`);

// A gesture: forty steps with no pause long enough to count as letting go.
const during = [];
for (let i = 0; i < 40; i++) {
  await page.keyboard.press('ArrowUp');
  if (i % 8 === 7) during.push(await sample());
}
console.log('  during the gesture:');
for (const d of during) {
  const ratio = d.lowest && d.tune ? d.lowest / d.tune : null;
  console.log(`    Tune ${String(d.tune).padStart(7)} Hz  axis 1x ${String(d.axisF0).padStart(7)} Hz  lowest ${String(d.lowest).padStart(7)} Hz  drawn at ${ratio ? (d.lowest / d.axisF0).toFixed(3) : '—'}x  ${d.build}`);
}

await page.waitForTimeout(2500);
const end = await sample();
const ratio = end.lowest / end.tune;
console.log(`  after letting go: Tune ${end.tune} Hz, lowest ${end.lowest} Hz, ratio ${ratio.toFixed(3)}, ${end.build}`);

if (end.tune <= start.tune) fail('the gesture did not move Tune at all');
if (end.build !== 'settled') fail(`the mode table did not settle after the gesture: ${end.build}`);
else ok('the table settles once the gesture stops');
if (!(Math.abs(ratio - 1) < 0.02)) fail(`the lowest partial is not the fundamental: ratio ${ratio.toFixed(3)}`);
else ok('and the bank ends up on the object the controls describe');

// The point of the change: it moved *during* the gesture, not only after it.
const moved = during.filter((d) => d.lowest != null && Math.abs(d.lowest - start.lowest) > 1).length;
if (!moved) fail('the bank published the same partials throughout the gesture, so it only follows once you let go');
else ok(`the bank moved ${moved} of ${during.length} times sampled mid-gesture`);

/**
 * The display's two feeds have to agree with each other, which is a different
 * claim from agreeing with the knob.
 *
 * **The knob leading the engine mid-gesture is correct**: the control is what
 * you asked for and the display is what is being synthesised, and during a
 * fast sweep the bank is a search behind. What would be wrong is the display
 * drawing partials from one state against an axis from another — a partial at
 * 1630 Hz under a ruler whose 1x is 4000 Hz appears at 0.4x, and a reader has
 * no way to know the picture is of two different moments. So this checks that
 * the lowest partial lands on 1x within a tolerance, which is what makes the
 * lag honest rather than confusing.
 */
const incoherent = during.filter((d) => d.axisF0 && d.lowest && Math.abs(d.lowest / d.axisF0 - 1) > 0.25);
if (incoherent.length > 1) {
  fail(`the axis and the partials came from different states ${incoherent.length} times: ` +
    incoherent.map((d) => `${d.lowest}Hz on a ${d.axisF0}Hz ruler`).join(', '));
} else {
  ok('the axis and the partials stay on the same state, so the lag is legible rather than confusing');
}

await browser.close();
console.log(bad === 0 ? '\nFOLLOWS CLEAN' : `\n${bad} PROBLEM(S)`);
process.exit(bad === 0 ? 0 : 1);
