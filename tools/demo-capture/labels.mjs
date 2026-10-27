// Draw the v3 video's labels as transparent 1920x1080 PNGs with IBM Plex.
//
// Input: a JSON list written by scripts/render_demo_video_v3.py, one entry per
// scene: {file, chip, badge, band}. The chip (top left) says what the footage
// is, the badge (top right) states a speed-up and the real time it covers, and
// the band (bottom) is one line for viewers watching without sound.
import puppeteer from 'puppeteer-core';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const plan = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
const font = name => `url(data:font/woff2;base64,${fs.readFileSync(path.join(root, 'demo/fonts', name)).toString('base64')}) format("woff2")`;
const esc = value => String(value).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');

const style = `
@font-face {font-family:Plex;src:${font('ibm-plex-sans-latin-500-normal.woff2')};font-weight:500}
@font-face {font-family:Plex;src:${font('ibm-plex-sans-latin-600-normal.woff2')};font-weight:600}
html,body {margin:0;width:1920px;height:1080px;background:transparent;-webkit-font-smoothing:antialiased;font-family:Plex,sans-serif}
.chip {position:absolute;left:40px;top:36px;display:flex;align-items:center;gap:14px;padding:12px 22px;border-radius:999px;
  background:rgba(20,20,19,.88);color:#f6f4ee;font-size:22px;font-weight:500;letter-spacing:.02em}
.chip i {width:12px;height:12px;border-radius:50%;background:#f4b728}
.chip.live i {background:#e5484d}
.badge {position:absolute;right:40px;top:36px;padding:12px 22px;border-radius:999px;background:#f4b728;color:#141413;font-size:24px;font-weight:600}
.band {position:absolute;left:0;right:0;bottom:0;height:104px;box-sizing:border-box;padding:0 64px;display:flex;align-items:center;
  background:rgba(20,20,19,.92);color:#fff;font-size:36px;font-weight:500;letter-spacing:-.005em}
`;

const browser = await puppeteer.launch({ executablePath: process.env.DARKCALL_BROWSER || '/Applications/Brave Browser.app/Contents/MacOS/Brave Browser',
  headless: true, args: ['--no-first-run', '--disable-extensions'], defaultViewport: { width: 1920, height: 1080, deviceScaleFactor: 1 } });
try {
  const page = await browser.newPage();
  for (const scene of plan) {
    const live = scene.chip && scene.chip.startsWith('LIVE');
    const body = (scene.chip ? `<div class="chip${live ? ' live' : ''}"><i></i>${esc(scene.chip)}</div>` : '')
      + (scene.badge ? `<div class="badge">${esc(scene.badge)}</div>` : '')
      + (scene.band ? `<div class="band">${esc(scene.band)}</div>` : '');
    await page.setContent(`<!doctype html><html><head><meta charset="utf-8"><style>${style}</style></head><body>${body}</body></html>`);
    await page.evaluate(() => document.fonts.ready);
    if (!(await page.evaluate(() => document.fonts.check('500 22px Plex')))) throw new Error('Plex did not load');
    const wide = await page.evaluate(() => [...document.querySelectorAll('.band')].some(node => node.scrollWidth > node.clientWidth));
    if (wide) throw new Error('Band text does not fit: ' + scene.band);
    await page.screenshot({ path: scene.file, omitBackground: true });
  }
} finally {
  await browser.close();
}
