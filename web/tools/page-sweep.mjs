// Walk the whole panel at three window sizes, in design mode and against the
// built bundle, and report every console error, page error and failed request.
//
//   node res-sweep.mjs <devUrl> <prodUrl> <outDir>
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
import { mkdirSync } from 'node:fs';

const DEV = process.argv[2] || 'http://127.0.0.1:5199/';
const PROD = process.argv[3] || 'http://127.0.0.1:5200/';
const OUT = process.argv[4] || './shots';
mkdirSync(OUT, { recursive: true });

/**
 * The browser to drive.
 *
 * Set `RES_CHROMIUM` to a Chromium or Chrome executable. Left unset,
 * Playwright uses whatever it downloaded for itself — which is the right
 * default and not always what is on the machine, so the override exists.
 */
const EXE = process.env.RES_CHROMIUM || undefined;
const browser = await chromium.launch(EXE ? { executablePath: EXE } : {});

const SIZES = [
  [900, 520],
  [1100, 620],
  [1900, 1000],
];

let problems = 0;
let expected = 0;

// No plug-in is listening, so the client cannot open its socket and says so.
// That is the design-mode path working rather than a fault, and counting it as
// one buries the faults that matter.
const EXPECTED = /\/ws\b/;

function watch(page, label) {
  page.on('console', (m) => {
    if (m.type() === 'error' || m.type() === 'warning') {
      if (EXPECTED.test(m.text())) {
        expected++;
        return;
      }
      problems++;
      console.log(`  !! ${label} console ${m.type()}: ${m.text().slice(0, 300)}`);
    }
  });
  page.on('pageerror', (e) => {
    problems++;
    console.log(`  !! ${label} pageerror: ${String(e).slice(0, 300)}`);
  });
  page.on('requestfailed', (r) => {
    if (EXPECTED.test(r.url())) {
      expected++;
      return;
    }
    problems++;
    console.log(`  !! ${label} requestfailed: ${r.url()} ${r.failure()?.errorText}`);
  });
  page.on('response', (r) => {
    if (r.status() >= 400) {
      problems++;
      console.log(`  !! ${label} HTTP ${r.status()} ${r.url()}`);
    }
  });
}

/** Nothing on the page may be blank, and the panel has to actually say things. */
async function health(page, label) {
  const s = await page.evaluate(() => {
    const app = document.querySelector('#app');
    const text = app ? (app.innerText || '').trim() : '';
    const bad = [...document.querySelectorAll('*')].filter((el) => /NaN|undefined|\[object/.test(el.textContent || '') && el.children.length === 0);
    return {
      children: app ? app.children.length : -1,
      len: text.length,
      head: text.slice(0, 90).replace(/\s+/g, ' '),
      scrollX: document.documentElement.scrollWidth > document.documentElement.clientWidth,
      bad: bad.slice(0, 4).map((el) => (el.textContent || '').trim().slice(0, 60)),
    };
  });
  const flag = s.len < 200 ? ' <<< NEARLY EMPTY' : '';
  if (s.len < 200) problems++;
  if (s.scrollX) {
    problems++;
    console.log(`  !! ${label} scrolls horizontally`);
  }
  if (s.bad.length) {
    problems++;
    console.log(`  !! ${label} prints ${JSON.stringify(s.bad)}`);
  }
  console.log(`  ${label.padEnd(34)} ${String(s.len).padStart(5)} chars  ${s.head}${flag}`);
}

async function open(url, w, h, label) {
  const page = await browser.newPage({ viewport: { width: w, height: h } });
  watch(page, label);
  await page.goto(url, { waitUntil: 'networkidle', timeout: 25000 });
  await page.waitForTimeout(1800);
  return page;
}

// --- design mode -----------------------------------------------------------
console.log(`\n=== design mode · ${DEV} ===`);
for (const [w, h] of SIZES) {
  const tag = `${w}x${h}`;
  const page = await open(DEV, w, h, `panel ${tag}`);
  await health(page, `panel ${tag}`);
  await page.screenshot({ path: `${OUT}/panel-${tag}.png` });

  // Browse, which is a layer over the panel rather than a page in place of it.
  await page.getByRole('button', { name: /change resonator/i }).first().click();
  await page.waitForTimeout(700);
  await health(page, `browse ${tag}`);
  await page.screenshot({ path: `${OUT}/browse-${tag}.png` });
  await page.keyboard.press('Escape');
  await page.waitForTimeout(400);

  // Presets.
  const preset = page.locator('button', { hasText: /preset/i }).first();
  if (await preset.count()) {
    await preset.click();
    await page.waitForTimeout(700);
    await health(page, `presets ${tag}`);
    await page.screenshot({ path: `${OUT}/presets-${tag}.png` });
    await page.keyboard.press('Escape');
    await page.waitForTimeout(400);
  }
  await page.close();
}

// --- every object, at one size --------------------------------------------
console.log('\n=== every object · 1100x620 ===');
{
  const page = await open(DEV, 1100, 620, 'objects');
  const labels = await page.evaluate(async () => {
    const btn = [...document.querySelectorAll('button')].find((b) => /change resonator/i.test(b.textContent || ''));
    btn?.click();
    await new Promise((r) => setTimeout(r, 500));
    return [...document.querySelectorAll('.browse__card')].map((c) => (c.querySelector('.browse__name')?.textContent || '').trim().split('\n')[0]);
  });
  console.log(`  browse lists ${labels.length}: ${labels.join(' · ')}`);
  if (labels.length !== 10) {
    problems++;
    console.log('  !! the browser should list ten objects');
  }
  await page.keyboard.press('Escape');
  for (let i = 0; i < labels.length; i++) {
    await page.evaluate(async (ix) => {
      const btn = [...document.querySelectorAll('button')].find((b) => /change resonator/i.test(b.textContent || ''));
      btn?.click();
      await new Promise((r) => setTimeout(r, 350));
      document.querySelectorAll('.browse__card')[ix]?.click();
      await new Promise((r) => setTimeout(r, 350));
    }, i);
    await page.waitForTimeout(900);
    const name = await page.evaluate(() => document.querySelector('.pick__name')?.textContent?.trim());
    const bars = await page.evaluate(() => document.querySelectorAll('.md__bar, .md__peak, [class*="md__"]').length);
    console.log(`  ${String(i).padStart(2)} ${String(name).padEnd(16)} display elements ${bars}`);
    if (!name) {
      problems++;
      console.log('  !! no object name on the panel');
    }
    await health(page, `object ${i} ${name}`);
    await page.screenshot({ path: `${OUT}/object-${String(i).padStart(2, '0')}-${String(name).replace(/\W+/g, '')}.png` });
  }
  await page.close();
}

// --- the built bundle ------------------------------------------------------
console.log(`\n=== production bundle · ${PROD} ===`);
{
  const page = await open(PROD, 1100, 620, 'dist');
  const s = await page.evaluate(() => (document.querySelector('#app')?.innerText || '').trim());
  console.log(`  dist says ${JSON.stringify(s.slice(0, 120))}`);
  if (!s) {
    problems++;
    console.log('  !! the built bundle rendered nothing at all');
  }
  await page.screenshot({ path: `${OUT}/dist.png` });
  await page.close();
}

await browser.close();
console.log(`\n${expected} expected socket failures, nothing listening`);
console.log(`${problems === 0 ? 'CLEAN' : `${problems} PROBLEM(S)`}`);
process.exit(problems === 0 ? 0 : 1);
