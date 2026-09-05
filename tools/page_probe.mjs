// Load a running bridge's page in a real browser and report what it does.
// This is the rig the project did not have: it exercises the **built bundle**
// against a **real bridge**, which is the combination a dev server never tests.
import { createRequire } from 'node:module';
import { existsSync, readdirSync } from 'node:fs';
import { homedir } from 'node:os';
import path from 'node:path';

const url = process.argv[2] || 'http://127.0.0.1:4246/';
const shot = process.argv[3] || null;

// **Find playwright rather than importing it by name.** An `import` from this
// file resolves upward from `tools/`, so it can only ever see a copy inside
// the repository — and this probe is deliberately not a dependency of the
// page's build, because it tests the built bundle from outside. So look in
// the places a copy actually lives, and say what to do when there is none
// instead of throwing a module-resolution stack trace at somebody.
const here = path.dirname(new URL(import.meta.url).pathname.replace(/^\/(\w:)/, '$1'));
const roots = [
  process.env.PLAYWRIGHT_DIR,
  path.join(here, '..', 'web'),
  path.join(here, '..'),
  process.cwd(),
].filter(Boolean);
let chromium = null;
for (const root of roots) {
  try {
    const req = createRequire(path.join(root, 'noop.js'));
    ({ chromium } = req('playwright'));
    break;
  } catch {
    /* keep looking */
  }
}
if (!chromium) {
  console.log('NO PLAYWRIGHT. This probe needs the package, not just the browsers.');
  console.log('  npm install --no-save playwright   (in any directory, then run from it)');
  console.log('  or set PLAYWRIGHT_DIR to a directory whose node_modules has it.');
  console.log(`  looked in: ${roots.join(', ')}`);
  process.exit(2);
}

// **Reuse the browser already on this machine rather than downloading
// another**, because the disk here has been full once already — but find it
// by looking rather than by naming a build number, which goes stale the first
// time playwright updates.
function headlessShell() {
  const base = path.join(homedir(), 'AppData', 'Local', 'ms-playwright');
  if (!existsSync(base)) return null;
  const builds = readdirSync(base)
    .filter((d) => d.startsWith('chromium_headless_shell-'))
    .sort((a, b) => Number(b.split('-')[1]) - Number(a.split('-')[1]));
  for (const b of builds) {
    const exe = path.join(base, b, 'chrome-headless-shell-win64', 'chrome-headless-shell.exe');
    if (existsSync(exe)) return exe;
  }
  return null;
}
const exe = headlessShell();
const browser = await chromium.launch(exe ? { executablePath: exe } : {});
const page = await browser.newPage({ viewport: { width: 1100, height: 700 } });

const console_lines = [];
const errors = [];
const failed = [];
page.on('console', (m) => console_lines.push(`${m.type()}: ${m.text()}`));
page.on('pageerror', (e) => errors.push(String(e && e.stack ? e.stack.split('\n')[0] : e)));
page.on('requestfailed', (r) => failed.push(`${r.url()} — ${r.failure()?.errorText}`));
page.on('response', (r) => {
  if (r.status() >= 400) failed.push(`${r.url()} — HTTP ${r.status()}`);
});

try {
  await page.goto(url, { waitUntil: 'networkidle', timeout: 20000 });
} catch (e) {
  console.log(`GOTO FAILED: ${e.message}`);
}
await page.waitForTimeout(4000);

const state = await page.evaluate(() => {
  const app = document.querySelector('#app');
  return {
    title: document.title,
    appChildren: app ? app.children.length : -1,
    appTextLen: app ? (app.innerText || '').trim().length : -1,
    bodyHTMLLen: document.body.innerHTML.length,
    firstText: app ? (app.innerText || '').trim().slice(0, 200) : '',
  };
});

console.log(`url            ${url}`);
console.log(`title          ${state.title}`);
console.log(`#app children  ${state.appChildren}`);
console.log(`#app text      ${state.appTextLen} chars`);
console.log(`body html      ${state.bodyHTMLLen} chars`);
if (state.firstText) console.log(`first text     ${JSON.stringify(state.firstText)}`);
console.log(`\n--- page errors (${errors.length}) ---`);
errors.slice(0, 10).forEach((e) => console.log(`  ${e}`));
console.log(`--- failed requests (${failed.length}) ---`);
failed.slice(0, 10).forEach((f) => console.log(`  ${f}`));
console.log(`--- console (${console_lines.length}) ---`);
console_lines.slice(0, 25).forEach((l) => console.log(`  ${l}`));

if (shot) {
  await page.screenshot({ path: shot, fullPage: false });
  console.log(`\nscreenshot ${shot}`);
}
await browser.close();
