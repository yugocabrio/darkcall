// Full-page design snapshots of the local demo (scratch review aid, not evidence).
// usage: node snap.mjs OUT_DIR WIDTH [hash-or-click]
import puppeteer from 'puppeteer-core';
const [out, width, mode] = process.argv.slice(2);
const browser = await puppeteer.launch({ executablePath: '/Applications/Brave Browser.app/Contents/MacOS/Brave Browser', headless: true,
  args: ['--no-first-run', '--disable-extensions', '--hide-scrollbars'], defaultViewport: { width: Number(width), height: 900, deviceScaleFactor: 1 } });
const page = await browser.newPage();
await page.goto(process.env.DARKCALL_URL || 'http://127.0.0.1:8877/', { waitUntil: 'networkidle0' });
if (mode) { await page.click(mode); await new Promise(r => setTimeout(r, 1500)); }
await page.screenshot({ path: `${out}/${width}-${(mode || 'landing').replace(/[^a-z]/g, '')}.png`, fullPage: true });
await browser.close();
