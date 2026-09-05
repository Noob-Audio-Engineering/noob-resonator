// Presets, end to end, against a running plug-in: load one, check the
// parameters actually moved, check the mode table travelled, save one, reload
// the page and check it came back.
//
//   node res-presets-e2e.mjs <pageUrl> <shotDir>
/**
 * Playwright is not a dependency of this package and deliberately is not.
 *
 * It pulls a browser download behind it, and this page's own tests
 * (`npm test`) run in node with none. So the probes ask for it at run time and
 * say how to get it, rather than making every clone pay for a browser to build
 * a plug-in panel.
 */
const { chromium } = await import('playwright').catch(() => {
  console.error('This probe drives a real browser and needs Playwright:');
  console.error('  npm i -D playwright');
  process.exit(2);
});

const URL = process.argv[2] || 'http://localhost:5199/';
const SHOTS = process.argv[3] || '.';
/**
 * The browser to drive.
 *
 * Set `RES_CHROMIUM` to a Chromium or Chrome executable. Left unset,
 * Playwright uses whatever it downloaded for itself — which is the right
 * default and not always what is on the machine, so the override exists.
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
page.on('pageerror', (e) => fail(`page error: ${e}`));
page.on('console', (m) => {
  if (m.type() === 'error' && !/\/ws\b/.test(m.text())) fail(`console: ${m.text().slice(0, 200)}`);
});

await page.goto(URL, { waitUntil: 'networkidle' });
await page.waitForTimeout(2500);

const live = await page.evaluate(() => !document.querySelector('.md__stamp'));
console.log(`page is ${live ? 'live against a plug-in' : 'in design mode'}`);
if (!live) {
  console.log('  nothing to verify end to end without a plug-in');
  await browser.close();
  process.exit(2);
}

/** Every knob's label and printed value, which is what a user actually reads. */
const readPanel = () =>
  page.evaluate(() =>
    Object.fromEntries(
      [...document.querySelectorAll('.knob')].map((k) => [
        (k.querySelector('.knob__label')?.textContent || '').trim().toLowerCase(),
        (k.querySelector('.knob__value-text')?.textContent || '').trim(),
      ]),
    ),
  );

/** The segmented controls, which is where Selection lives. */
const readSegments = () =>
  page.evaluate(() =>
    [...document.querySelectorAll('[class*="seg"], .sel__modes')]
      .map((g) => [...g.querySelectorAll('button')].filter((b) => /on|is-on|active/.test(b.className)).map((b) => b.textContent.trim()))
      .flat(),
  );

const openPresets = async () => {
  await page.click('.bar__preset');
  await page.waitForTimeout(700);
};
const closeOverlay = async () => {
  await page.keyboard.press('Escape');
  await page.waitForTimeout(500);
};

async function loadByName(name) {
  const found = await page.evaluate(async (want) => {
    const row = [...document.querySelectorAll('.preset')].find(
      (el) => (el.querySelector('.preset__name')?.textContent || '').trim().startsWith(want),
    );
    if (!row) return false;
    row.querySelector('.preset__pick')?.click();
    await new Promise((r) => setTimeout(r, 500));
    return true;
  }, name);
  await page.waitForTimeout(900);
  if (!found) fail(`no preset row named ${JSON.stringify(name)}`);
  return found;
}

// --- what the browser lists ------------------------------------------------
await openPresets();
const listing = await page.evaluate(() => ({
  rows: document.querySelectorAll('.preset').length,
  groups: [...document.querySelectorAll('.browse__familyname')].map((e) => e.textContent.trim()),
  paired: document.querySelectorAll('.preset.paired').length,
}));
console.log(`  ${listing.rows} presets in ${listing.groups.length} groups: ${listing.groups.join(', ')}`);
console.log(`  ${listing.paired} rows marked as one half of a pair`);
if (listing.rows < 10) fail(`the engine published 33 presets and the browser drew ${listing.rows}`);
else ok(`${listing.rows} rows drawn from the engine's own list`);
if (!listing.paired) fail('no pair marked, so the A/B argument is not offered');
else ok('the pairs are marked');
await page.screenshot({ path: `${SHOTS}/live-presets.png` });

// --- the A/B pair really moves the panel -----------------------------------
await loadByName('A · Loudest');
const a = await readPanel();
const segsA = await readSegments();
await loadByName('B · Lowest');
const b = await readPanel();
const segsB = await readSegments();
const differ = Object.keys(a).filter((k) => a[k] !== b[k]);
console.log(`  the pair's knobs differ in: ${differ.join(', ') || '(nothing, which is the point)'}`);
console.log(`  and its segmented controls read ${JSON.stringify(segsA)} then ${JSON.stringify(segsB)}`);
if (!Object.keys(a).length) fail('no knobs read off the panel');
else ok(`${Object.keys(a).length} controls read off the face`);
if (differ.length) fail(`the pair should differ in exactly one control and it is Selection, not ${differ.join(', ')}`);
if (JSON.stringify(segsA) === JSON.stringify(segsB)) fail('Selection did not move between A and B');
else ok('the one control the pair differs in is Selection, and the face shows it move');

// --- a whole preset applied: every covered control lands on its value ------
await loadByName('Glockenspiel');
const glock = await readPanel();
await closeOverlay();
await page.waitForTimeout(700);
const objName = await page.evaluate(() => document.querySelector('.pick__name')?.textContent?.trim());
console.log(`  Glockenspiel loads ${objName}, Tune ${glock.tune}, Decay ${glock.decay}`);
if (objName !== 'Beam') fail(`Glockenspiel is a Beam preset and the panel shows ${objName}`);
else ok('the object parameter travelled with the preset');
if (!/880/.test(glock.tune || '')) fail(`Glockenspiel tunes to 880 Hz, the panel reads ${glock.tune}`);
else ok('and so did Tune, in plain units');

// --- a preset that carries mode edits --------------------------------------
await openPresets();
await loadByName('Hand Bell');
await closeOverlay();
await page.waitForTimeout(1200);
const bell = await page.evaluate(() => ({
  name: document.querySelector('.pick__name')?.textContent?.trim(),
  edited: document.querySelectorAll('.g-handles line.edited').length,
  tune: [...document.querySelectorAll('.knob')].find((k) => /TUNE/i.test(k.querySelector('.knob__label')?.textContent || ''))
    ?.querySelector('.knob__value-text')?.textContent?.trim(),
}));
console.log(`  Hand Bell: ${bell.name} at ${bell.tune}, ${bell.edited} partials marked edited`);
if (bell.name !== 'String') fail(`Hand Bell is a String preset and the panel shows ${bell.name}`);
if (bell.edited < 5) fail(`Hand Bell carries five mode edits and the display marks ${bell.edited}`);
else ok('the mode table travelled with the preset and reached the display');
await page.screenshot({ path: `${SHOTS}/live-handbell.png` });

// --- and a preset with none clears them ------------------------------------
await openPresets();
await loadByName('Nylon');
await closeOverlay();
await page.waitForTimeout(1000);
const cleared = await page.evaluate(() => document.querySelectorAll('.g-handles line.edited').length);
console.log(`  after loading a preset with no edits: ${cleared} marked edited`);
if (cleared !== 0) fail(`an empty modes array must clear the table, ${cleared} survived`);
else ok('an empty modes array clears the table, as the format says');

// --- moving a control marks the loaded preset modified ---------------------
const dirtyBefore = await page.evaluate(() => !!document.querySelector('.bar__dirty'));
await page.evaluate(() => {
  const knob = [...document.querySelectorAll('.knob')].find((k) => /TUNE/i.test(k.querySelector('.knob__label')?.textContent || ''));
  knob?.querySelector('.knob__dial')?.focus();
});
for (let i = 0; i < 10; i++) {
  await page.keyboard.press('ArrowUp');
  await page.waitForTimeout(50);
}
await page.waitForTimeout(800);
const dirtyAfter = await page.evaluate(() => !!document.querySelector('.bar__dirty'));
const tuneNow = (await readPanel()).tune;
console.log(`  Tune moved to ${tuneNow}; dirty marker ${dirtyBefore} -> ${dirtyAfter}`);
if (dirtyBefore) fail('the preset was already marked modified before anything moved');
if (!dirtyAfter) fail('moving a control did not mark the preset modified');
else ok('the dirty marker tracks a real edit');

// --- save, reload, and see whether it came back ----------------------------
await openPresets();
await page.click('.browse__save');
await page.waitForTimeout(500);
await page.locator('form.save .save__field input[type="text"]').first().fill('Probe Preset');
await page.waitForTimeout(300);
await page.click('.save__foot button[type="submit"]');
await page.waitForTimeout(900);
const savedNow = await page.evaluate(() => !!document.querySelector('.preset[data-preset="user:Probe Preset"]'));
if (!savedNow) fail('saving did not add a row');
else ok('saving adds a row');
await page.screenshot({ path: `${SHOTS}/live-saved.png` });

await page.reload({ waitUntil: 'networkidle' });
await page.waitForTimeout(2600);
await openPresets();
const back = await page.evaluate(() => !!document.querySelector('.preset[data-preset="user:Probe Preset"]'));
if (!back) fail('a saved user preset did not survive a page reload');
else ok('a user preset survives a reload, so it is in the plug-in state and not the page');

// Clean up rather than leaving a probe's preset in the user's state.
const removal = await page.evaluate(async () => {
  // **Match the row's own name, not the text in it.** The pair finder makes a
  // neighbouring factory row advertise "A/B with Probe Preset", so a substring
  // search on the whole row lands on Nylon — which is the pairing working, and
  // a probe that cannot tell them apart is the probe's fault.
  const row = document.querySelector('.preset[data-preset="user:Probe Preset"]');
  if (!row) return 'no row';
  const buttons = [...row.querySelectorAll('button')].map((b) => (b.textContent || '').trim());
  const del = [...row.querySelectorAll('.preset__tools button')].find((b) => /^delete$/i.test((b.textContent || '').trim()));
  if (!del) return `no delete button among ${JSON.stringify(buttons)}`;
  del.click();
  await new Promise((r) => setTimeout(r, 800));
  return 'clicked';
});
console.log(`  removal: ${removal}`);
await page.waitForTimeout(600);
const gone = await page.evaluate(() => !document.querySelector('.preset[data-preset="user:Probe Preset"]'));
console.log(`  probe preset removed afterwards: ${gone}`);
if (!gone) fail('could not remove the probe preset; the user is left with it');

await browser.close();
console.log(bad === 0 ? '\nPRESETS CLEAN END TO END' : `\n${bad} PROBLEM(S)`);
process.exit(bad === 0 ? 0 : 1);
