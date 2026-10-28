import { chromium } from 'playwright';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const ids = process.env.SCREENS ? process.env.SCREENS.split(',') : null;
const html = readFileSync(resolve('screens.html'), 'utf8');
const all = [...html.matchAll(/<section class="screen[^"]*" id="([^"]+)"/g)].map(m => m[1]);
const targets = ids || all;
const browser = await chromium.launch({
  executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  headless: true,
});
const page = await browser.newPage({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });
for (const id of targets) {
  await page.goto('file://' + resolve('screens.html') + '?screen=' + id);
  await page.waitForTimeout(250);
  await page.screenshot({ path: resolve('screens', id + '.png') });
  console.log(id);
}
await browser.close();
