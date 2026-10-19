// Draw the demo video's caption bands and title cards with the page's own fonts.
//
// Input: a JSON list written by scripts/render_demo_video_v2.py, one entry per
// scene: {kind: 'band'|'card', title, text, identity, badge, file}. Bands are
// transparent 1920x1080 PNGs composited over the captured frame; cards are full
// frames. IBM Plex (SIL OFL 1.1) is embedded from demo/fonts as data URLs, so no
// network or server is involved.
import puppeteer from 'puppeteer-core';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const plan = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
const font = name => `url(data:font/woff2;base64,${fs.readFileSync(path.join(root, 'demo/fonts', name)).toString('base64')}) format("woff2")`;
const esc = value => String(value).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
const lines = value => esc(value).split('\n').join('<br>');

const style = `
@font-face {font-family:Plex;src:${font('ibm-plex-sans-latin-400-normal.woff2')};font-weight:400}
@font-face {font-family:Plex;src:${font('ibm-plex-sans-latin-500-normal.woff2')};font-weight:500}
@font-face {font-family:Plex;src:${font('ibm-plex-sans-latin-600-normal.woff2')};font-weight:600}
@font-face {font-family:"Plex Mono";src:${font('ibm-plex-mono-latin-400-normal.woff2')};font-weight:400}
html,body {margin:0;width:1920px;height:1080px;background:transparent;-webkit-font-smoothing:antialiased}
body {font-family:Plex,sans-serif}
.band {position:absolute;left:0;right:0;bottom:0;height:212px;box-sizing:border-box;padding:24px 80px 0;background:rgba(20,20,19,.95)}
.band h1 {margin:0;font-size:40px;line-height:1.1;font-weight:600;letter-spacing:-.01em;color:#fff}
.band p {margin:12px 0 0;font-size:26px;line-height:1.38;color:#e9e7df}
.foot {position:absolute;left:80px;right:80px;bottom:18px;display:flex;justify-content:space-between;font-size:17px}
.foot .id {font-family:"Plex Mono",monospace;color:#a9a79e}
.foot .badge {color:#f4b728;font-weight:500}
.card {position:absolute;inset:0;background:#f6f4ee}
.card .badge {position:absolute;left:160px;top:60px;font-size:20px;font-weight:500;color:#8a6408}
.card .bar {position:absolute;left:160px;top:290px;width:64px;height:6px;background:#f4b728}
.card h1 {position:absolute;left:160px;right:160px;top:322px;margin:0;font-size:64px;line-height:1.1;font-weight:600;letter-spacing:-.025em;color:#141413}
.card p {position:absolute;left:160px;right:160px;top:446px;margin:0;font-size:34px;line-height:1.5;color:#4a4944}
.card .id {position:absolute;left:160px;bottom:62px;font-family:"Plex Mono",monospace;font-size:18px;color:#706e66}
`;

const browser = await puppeteer.launch({ executablePath: process.env.DARKCALL_BROWSER || '/Applications/Brave Browser.app/Contents/MacOS/Brave Browser',
  headless: true, args: ['--no-first-run', '--disable-extensions'], defaultViewport: { width: 1920, height: 1080, deviceScaleFactor: 1 } });
try {
  const page = await browser.newPage();
  for (const scene of plan) {
    const body = scene.kind === 'band'
      ? `<div class="band"><h1>${esc(scene.title)}</h1><p>${lines(scene.text)}</p><div class="foot"><span class="id">${esc(scene.identity)}</span><span class="badge">${esc(scene.badge)}</span></div></div>`
      : `<div class="card"><span class="badge">${esc(scene.badge)}</span><span class="bar"></span><h1>${esc(scene.title)}</h1><p>${lines(scene.text)}</p><span class="id">${esc(scene.identity)}</span></div>`;
    await page.setContent(`<!doctype html><html><head><meta charset="utf-8"><style>${style}</style></head><body>${body}</body></html>`);
    await page.evaluate(() => document.fonts.ready);
    if (!(await page.evaluate(() => document.fonts.check('600 40px Plex')))) throw new Error('Plex did not load');
    await page.screenshot({ path: scene.file, omitBackground: scene.kind === 'band' });
  }
} finally {
  await browser.close();
}
