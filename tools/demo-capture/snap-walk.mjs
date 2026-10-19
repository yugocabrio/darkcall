// Design snapshots of the "Follow one auction" walkthrough at each step (review aid).
import puppeteer from 'puppeteer-core';
const [out, width] = process.argv.slice(2);
const browser = await puppeteer.launch({ executablePath: '/Applications/Brave Browser.app/Contents/MacOS/Brave Browser', headless: true,
  args: ['--no-first-run', '--disable-extensions', '--hide-scrollbars'], defaultViewport: { width: Number(width), height: 1000, deviceScaleFactor: 1 } });
const page = await browser.newPage();
await page.goto(process.env.DARKCALL_URL || 'http://127.0.0.1:8877/', { waitUntil: 'networkidle0' });
const walk = await page.$('#walk');
for (const step of [1, 3, 5]) {
  await page.click(`.walk-steps button[data-step="${step}"]`);
  if (step === 5) await page.click('#walk-values');
  await new Promise(r => setTimeout(r, 900));
  await walk.screenshot({ path: `${out}/walk-${width}-${step}.png` });
}
await browser.close();
