// Render the deck in tools/demo-capture/slides to numbered 1920x1080 PNGs for the video and the PDF.
//
// Each slide is one <section> with inline styles (the published deck's format).
// This page adds only what the deck runtime would: the slide box, reset margins,
// table rules, the arrow shape and IBM Plex (SIL OFL 1.1) from demo/fonts, so no
// network is involved. Speaker notes (<aside>) are hidden and written to notes.json.
import puppeteer from 'puppeteer-core';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const deckDir = path.join(root, 'tools/demo-capture/slides');
const out = process.argv[2];
if (!out) throw new Error('usage: node slides.mjs OUTPUT_DIRECTORY');
fs.mkdirSync(out, { recursive: true, mode: 0o700 });
const deck = JSON.parse(fs.readFileSync(path.join(deckDir, 'deck.json'), 'utf8'));
const font = name => `url(data:font/woff2;base64,${fs.readFileSync(path.join(root, 'demo/fonts', name)).toString('base64')}) format("woff2")`;

// Plex 700 is not vendored; headings use the 600 file rather than a synthesized bold.
const style = `
@font-face {font-family:"IBM Plex Sans";src:${font('ibm-plex-sans-latin-400-normal.woff2')};font-weight:400}
@font-face {font-family:"IBM Plex Sans";src:${font('ibm-plex-sans-latin-500-normal.woff2')};font-weight:500}
@font-face {font-family:"IBM Plex Sans";src:${font('ibm-plex-sans-latin-600-normal.woff2')};font-weight:600 700}
@font-face {font-family:"IBM Plex Mono";src:${font('ibm-plex-mono-latin-400-normal.woff2')};font-weight:400}
@font-face {font-family:"IBM Plex Mono";src:${font('ibm-plex-mono-latin-500-normal.woff2')};font-weight:500 700}
*,*::before,*::after {margin:0;box-sizing:border-box}
html,body {width:1920px;height:1080px;overflow:hidden;-webkit-font-smoothing:antialiased}
section {position:relative;width:1920px;height:1080px;overflow:hidden}
aside {display:none}
b {font-weight:600}
ul {padding-left:1.1em} li + li {margin-top:.35em} li::marker {color:#f4b728}
table {width:100%;border-collapse:collapse;line-height:1.35}
th {text-align:left;font-weight:600;color:#706e66;padding:0 20px 16px 0;border-bottom:2px solid #141413;vertical-align:bottom}
td {padding:18px 20px 18px 0;border-bottom:1px solid #d9d4c6;vertical-align:top}
td:first-child, th:first-child {padding-left:16px}
x-shape {display:block;flex:none}
x-shape[kind="arrow-right"] {clip-path:polygon(0 35%,58% 35%,58% 0,100% 50%,58% 100%,58% 65%,0 65%)}
`;

const browser = await puppeteer.launch({ executablePath: process.env.DARKCALL_BROWSER || '/Applications/Brave Browser.app/Contents/MacOS/Brave Browser',
  headless: true, args: ['--no-first-run', '--disable-extensions', '--hide-scrollbars'], defaultViewport: { width: 1920, height: 1080, deviceScaleFactor: 1 } });
const notes = [];
try {
  const page = await browser.newPage();
  for (const [index, id] of deck.order.entries()) {
    const html = fs.readFileSync(path.join(deckDir, `${id}.html`), 'utf8');
    await page.setContent(`<!doctype html><html><head><meta charset="utf-8"><style>${style}</style></head><body>${html}</body></html>`);
    await page.evaluate(() => document.fonts.ready);
    if (!(await page.evaluate(() => document.fonts.check('600 72px "IBM Plex Sans"')))) throw new Error('Plex did not load');
    const overflow = await page.evaluate(() => [...document.querySelectorAll('section *')]
      .filter(node => node.getBoundingClientRect().right > 1921 || node.getBoundingClientRect().bottom > 1081).map(node => node.tagName));
    if (overflow.length) throw new Error(`${id} overflows the slide: ${overflow.join(',')}`);
    const file = `${String(index + 1).padStart(2, '0')}-${id}.png`;
    await page.screenshot({ path: path.join(out, file) });
    notes.push({ id, file, notes: await page.$eval('aside', node => node.textContent.trim()).catch(() => '') });
  }
} finally {
  await browser.close();
}
fs.writeFileSync(path.join(out, 'notes.json'), JSON.stringify(notes, null, 2) + '\n');
console.log(`rendered ${notes.length} slides to ${out}`);
