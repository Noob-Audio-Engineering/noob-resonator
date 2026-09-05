// Load a running bridge's page in a real browser and report what it does.
// This is the rig the project did not have: it exercises the **built bundle**
// against a **real bridge**, which is the combination a dev server never tests.
import { chromium } from 'playwright';

const url = process.argv[2] || 'http://127.0.0.1:4246/';
const shot = process.argv[3] || null;

// Reuse the browser already on this machine rather than downloading another;
// the disk on it has been full once today.
const EXE = String.raw`C:/Users/elyci/AppData/Local/ms-playwright/chromium_headless_shell-1234/chrome-headless-shell-win64/chrome-headless-shell.exe`;
const browser = await chromium.launch({ executablePath: EXE });
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
