import { chromium } from 'playwright';
import { resolve } from 'node:path';
const browser = await chromium.launch({
  executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  headless: true,
});
const page = await browser.newPage({ viewport: { width: 1600, height: 1000 }, deviceScaleFactor: 2 });
await page.goto('file://' + resolve('gallery.html'));
await page.waitForTimeout(800);
await page.screenshot({ path: resolve('gallery.png'), fullPage: true });
await browser.close();
