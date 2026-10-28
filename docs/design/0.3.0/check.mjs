import { chromium } from 'playwright';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
const html = readFileSync(resolve('screens.html'), 'utf8');
const ids = [...html.matchAll(/<section class="screen[^"]*" id="([^"]+)"/g)].map(m => m[1]);
const browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true });
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
const errors = [];
page.on('console', msg => { if (msg.type() === 'error') errors.push(msg.text()); });
page.on('pageerror', err => errors.push(String(err)));
for (const id of ids) {
  await page.goto('file://' + resolve('screens.html') + '?screen=' + id);
  await page.waitForTimeout(80);
  const m = await page.evaluate(() => ({
    visible: [...document.querySelectorAll('.screen')].filter(s => !s.hidden).length,
    sw: document.documentElement.scrollWidth,
    sh: document.documentElement.scrollHeight,
    bodyW: document.body.getBoundingClientRect().width,
    icons: [...document.querySelectorAll('.icon use')].filter(u => {
      const r = u.getBoundingClientRect(); return r.width === 0 || r.height === 0;
    }).length,
    missingUse: [...document.querySelectorAll('.icon use')].filter(u => !document.querySelector(u.getAttribute('href'))).length,
  }));
  console.log(id, JSON.stringify(m));
}
console.log('errors', errors);
await browser.close();
