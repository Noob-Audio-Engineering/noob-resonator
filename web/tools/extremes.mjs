// Drive every control to both ends on every object and look for a number that
// should not be there.
//
// The check is the SVG as much as the text: a NaN in a path's `d` or a
// coordinate draws nothing at all and never says why, which is the shape of
// bug a screenshot at default settings will never show.
//
//   node res-extremes.mjs <pageUrl>
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

const browser = await chromium.launch(EXE ? { executablePath: EXE } : {});
const page = await browser.newPage({ viewport: { width: 1400, height: 900 } });
page.on('pageerror', (e) => fail(`page error: ${String(e).slice(0, 200)}`));
page.on('console', (m) => {
  if ((m.type() === 'error' || m.type() === 'warning') && !/\/ws\b/.test(m.text())) {
    fail(`console ${m.type()}: ${m.text().slice(0, 200)}`);
  }
});

await page.goto(URL, { waitUntil: 'networkidle' });
await page.waitForTimeout(2500);

/** Anything printed or drawn that should never be there. */
async function inspect(where) {
  const r = await page.evaluate(() => {
    const badText = [];
    for (const el of document.querySelectorAll('*')) {
      if (el.children.length) continue;
      const t = (el.textContent || '').trim();
      if (/\bNaN\b|Infinity|undefined|\[object/.test(t)) badText.push(t.slice(0, 60));
    }
    const badAttr = [];
    for (const el of document.querySelectorAll('svg *')) {
      for (const a of el.attributes) {
        if (/NaN|Infinity/.test(a.value)) badAttr.push(`${el.tagName}.${a.name}=${a.value.slice(0, 50)}`);
      }
    }
    return {
      text: [...new Set(badText)].slice(0, 5),
      attr: [...new Set(badAttr)].slice(0, 5),
      len: (document.querySelector('#app')?.innerText || '').trim().length,
      bars: document.querySelectorAll('.g-bars line, .g-handles line').length,
    };
  });
  if (r.text.length) fail(`${where} prints ${JSON.stringify(r.text)}`);
  if (r.attr.length) fail(`${where} draws ${JSON.stringify(r.attr)}`);
  if (r.len < 400) fail(`${where} has only ${r.len} characters of text`);
  return r;
}

/** Every knob, driven to one end. */
async function driveAll(key) {
  const n = await page.evaluate(() => document.querySelectorAll('.knob__dial').length);
  for (let i = 0; i < n; i++) {
    await page.evaluate((ix) => document.querySelectorAll('.knob__dial')[ix]?.focus(), i);
    await page.keyboard.press(key);
  }
  await page.waitForTimeout(700);
  return n;
}

async function pickObject(ix) {
  await page.evaluate(async (i) => {
    [...document.querySelectorAll('button')].find((b) => /change resonator/i.test(b.textContent || ''))?.click();
    await new Promise((r) => setTimeout(r, 400));
    document.querySelectorAll('.browse__card')[i]?.click();
    await new Promise((r) => setTimeout(r, 400));
  }, ix);
  await page.waitForTimeout(900);
}

const count = await page.evaluate(async () => {
  [...document.querySelectorAll('button')].find((b) => /change resonator/i.test(b.textContent || ''))?.click();
  await new Promise((r) => setTimeout(r, 500));
  const n = document.querySelectorAll('.browse__card').length;
  document.querySelector('.browse__close')?.click();
  return n;
});
console.log(`${count} objects to walk`);

/**
 * Load a sane factory preset and see whether the display comes back.
 *
 * **The recovery is the test, not the extreme.** A panel with nothing drawn
 * while the fundamental sits above Nyquist is correct; a panel still empty
 * once every parameter has been set back is an engine that did not recover,
 * and a user who turned a knob up and back would never get their instrument
 * again.
 */
async function recovers() {
  await page.click('.bar__preset');
  await page.waitForTimeout(600);
  await page.evaluate(async () => {
    const row = [...document.querySelectorAll('.preset')].find(
      (el) => (el.querySelector('.preset__name')?.textContent || '').trim().startsWith('Nylon'),
    );
    row?.querySelector('.preset__pick')?.click();
    await new Promise((r) => setTimeout(r, 600));
  });
  await page.keyboard.press('Escape');
  await page.waitForTimeout(2200);
  return page.evaluate(() => {
    const tip = document.querySelector('.g-handles line title')?.textContent || '';
    return {
      handles: document.querySelectorAll('.g-handles line').length,
      first: tip.slice(0, 46),
    };
  });
}

for (let i = 0; i < count; i++) {
  await pickObject(i);
  const name = await page.evaluate(() => document.querySelector('.pick__name')?.textContent?.trim());
  const knobs = await driveAll('Home');
  const lo = await inspect(`${name} at every minimum`);
  await driveAll('End');
  const hi = await inspect(`${name} at every maximum`);
  const back = await recovers();
  const wedged = back.handles === 0 || /Mode \(\d\d\d/.test(back.first);
  if (wedged) fail(`after ${name} at its extremes the engine did not recover: ${JSON.stringify(back)}`);
  console.log(
    `  ${String(name).padEnd(15)} ${knobs} knobs · min ${lo.bars} · max ${hi.bars} · ` +
      `recovery ${wedged ? 'STUCK ' + back.first : 'ok'}`,
  );
  if (wedged) break;
}

await browser.close();
console.log(bad === 0 ? '\nEXTREMES CLEAN' : `\n${bad} PROBLEM(S)`);
process.exit(bad === 0 ? 0 : 1);
