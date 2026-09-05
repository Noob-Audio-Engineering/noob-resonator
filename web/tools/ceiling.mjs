// Are the partials the engine holds at the ceiling drawn as what they are?
//
//   node tools/ceiling.mjs http://localhost:5173/
//
// A pitch move can push a partial past Nyquist, and the engine clamps it there
// rather than letting it alias — so it sounds at the ceiling and not where the
// object's ratios put it. Several arrive at once and land on the same pixel,
// which drew as one bright partial at the top of the series instead of the
// twenty it was.
//
// Two cases, and the second keeps the first honest: with the oscillator
// pitching a dense object up, the stack should be marked, dashed and counted;
// with it off and the object at rest, nothing should be marked at all.
//
// **It sets up its own conditions.** The first version assumed the oscillator
// was already on from whatever had been done to the plug-in beforehand, and
// against a freshly started one it reported "no stack" — which is the answer a
// broken marking would also give. A probe that passes because somebody
// configured the device by hand is not a check.
const { chromium } = await import('playwright').catch(() => {
  console.error('This probe drives a real browser and needs Playwright:');
  console.error('  npm i -D playwright');
  process.exit(2);
});

const URL = process.argv[2] || 'http://localhost:5173/';
/** The browser to drive. Set `RES_CHROMIUM` to override Playwright's own. */
const EXE = process.env.RES_CHROMIUM || undefined;

let bad = 0;
const fail = (m) => {
  bad++;
  console.log(`  FAIL  ${m}`);
};
const ok = (m) => console.log(`  ok    ${m}`);

const b = await chromium.launch(EXE ? { executablePath: EXE } : {});
const p = await b.newPage({ viewport: { width: 1500, height: 950 } });
p.on('pageerror', (e) => fail(`page error: ${String(e).slice(0, 160)}`));
await p.goto(URL, { waitUntil: 'networkidle' });
await p.waitForTimeout(2500);

if (await p.evaluate(() => !!document.querySelector('.md__stamp'))) {
  console.log('design mode — nothing to measure without a plug-in');
  await b.close();
  process.exit(2);
}

/** Drive one named knob to one end of its travel. */
async function drive(label, key) {
  const found = await p.evaluate((want) => {
    const k = [...document.querySelectorAll('.knob')].find(
      (x) => (x.querySelector('.knob__label')?.textContent || '').trim().toLowerCase() === want.toLowerCase(),
    );
    k?.querySelector('.knob__dial')?.focus();
    return !!k;
  }, label);
  if (!found) return fail(`no control named ${label}`);
  await p.keyboard.press(key);
  await p.waitForTimeout(350);
}

/** The oscillator, set to a state rather than toggled blindly. */
async function setLfo(on) {
  const state = await p.evaluate((want) => {
    const g = [...document.querySelectorAll('.deck__group')].find((x) =>
      /^LFO/.test((x.querySelector('.deck__head')?.textContent || '').trim()),
    );
    const stack = [...(g?.querySelectorAll('.deck__stack') || [])].find((s) =>
      /LFO/.test(s.querySelector('.deck__cap')?.textContent || ''),
    );
    const btn = stack?.querySelector('button');
    if (!btn) return null;
    const isOn = () => btn.className.includes('is-on');
    // Clicking blind turned it *off* on the first run, and the still picture
    // then read as "no pile-up" rather than "no oscillator".
    if (isOn() !== want) btn.click();
    return want;
  }, on);
  if (state == null) fail('no LFO toggle on the panel');
  await p.waitForTimeout(600);
}

/** A dense object, which is the one whose series reaches the ceiling. */
await p.evaluate(async () => {
  [...document.querySelectorAll('button')].find((x) => /change resonator/i.test(x.textContent || ''))?.click();
  await new Promise((r) => setTimeout(r, 400));
  document.querySelectorAll('.browse__card')[3]?.click();
});
await p.waitForTimeout(1500);

const hz = (t) => {
  const m = (t || '').match(/([\d.]+)\s*(k?)Hz/);
  return m ? Number(m[1]) * (m[2] === 'k' ? 1000 : 1) : null;
};

const marking = () =>
  p.evaluate(() => {
    const cut = [...document.querySelectorAll('.g-cut text')].map((t) => t.textContent.trim());
    const tip = document.querySelector('.g-handles line.held title')?.textContent || '';
    return {
      label: document.querySelector('.g-held text')?.textContent?.trim() || null,
      dashed: document.querySelectorAll('.g-handles line.held').length,
      note: document.querySelector('.md__prov.is-note')?.textContent?.trim() || null,
      wall: cut[0] || null,
      wallSub: cut[1] || null,
      stackTip: tip,
    };
  });

// --- the pile, made rather than hoped for ---------------------------------
await setLfo(true);
await drive('Depth', 'End');
await drive('Rate', 'End');
await drive('Tune', 'End');
await p.waitForTimeout(1500);

/**
 * The wall and the stack are two true statements that sometimes land on one
 * line, and each frame has to be self-consistent about which case it is in.
 *
 * Checked on every sampled frame rather than by catching the rare state by
 * hand: when the top of the bank *is* the clamp, both captions have to say so;
 * when they are different frequencies — the ordinary case — neither may claim
 * they coincide.
 */
let seen = null;
let coincided = null;
let apart = 0;
for (let i = 0; i < 800; i++) {
  const m = await marking();
  if (m.label) {
    if (!seen) seen = m;
    const together = m.wall != null && hz(m.wall) != null && hz(m.stackTip) != null &&
      Math.abs(hz(m.wall) - hz(m.stackTip)) <= hz(m.stackTip) * 0.001;
    const saysTogether = /held partials are on it/.test(m.wallSub || '') ||
      /same line the bank stops at/.test(m.note || '');
    if (together && !saysTogether) {
      fail(`the wall and the stack are both at ${hz(m.stackTip)} Hz and neither caption says so`);
      break;
    }
    if (!together && saysTogether) {
      fail(`the captions claim one line while the wall is at ${hz(m.wall)} and the stack at ${hz(m.stackTip)}`);
      break;
    }
    if (together) coincided = m;
    else apart++;
    if (coincided && apart) break;
  }
  await p.waitForTimeout(20);
}
if (!seen) {
  fail('the series never piled up at the ceiling, so nothing was marked and nothing was tested');
} else {
  console.log(`  marker: ${seen.label}`);
  console.log(`  dashed: ${seen.dashed} handles`);
  console.log(`  note  : ${seen.note}`);
  const n = Number((seen.label.match(/(\d+)/) || [])[1]);
  if (!(n >= 3)) fail(`the marker should count at least three partials, it says ${seen.label}`);
  else ok(`${n} partials marked as held, and drawn dashed rather than as ordinary partials`);
  if (seen.dashed !== n) fail(`${n} counted but ${seen.dashed} drawn as held`);
  if (!seen.note || !/ceiling|alias/.test(seen.note)) fail('the stack is drawn but not explained');
  else ok('and the line under the plot says why they are there');

  if (coincided) {
    console.log(`  together: ${coincided.wallSub}`);
    ok('when the top of the bank is the clamp, both captions say the two are one line');
  } else {
    console.log('  (no frame caught where the wall and the stack were the same line)');
  }
  if (apart) ok(`and in ${apart} frames where they were different lines, neither claimed otherwise`);
}

// --- and at rest, nothing at all -------------------------------------------
await setLfo(false);
await drive('Tune', 'Home');
await p.waitForTimeout(3000);
const rest = await marking();
console.log(`  at rest: marker ${rest.label || '(none)'}, ${rest.dashed} dashed, note ${rest.note ? 'present' : '(none)'}`);
if (rest.label || rest.dashed || rest.note) {
  fail('something is marked as held on an object that is not being pushed anywhere');
} else {
  ok('nothing marked at rest, so the marking is not decorating an ordinary series');
}

await b.close();
console.log(bad === 0 ? '\nCEILING CLEAN' : `\n${bad} PROBLEM(S)`);
process.exit(bad === 0 ? 0 : 1);
