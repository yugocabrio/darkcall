// Screen-record the DarkCall UI in real time for the v3 video.
//
//   node record.mjs live    OUT   one new local auction: create, seal four bids
//                                 (client ZK proofs), certify, clear by REP3 and
//                                 coNoir, open one result. Reserve 10 is outside
//                                 the native settlement terms, so it clears
//                                 without deposits and touches no chain.
//   node record.mjs run     OUT   one new auction on the hosted site, on public
//                                 testnet: create, seal four bids, certify, fund,
//                                 clear, open a result, settle, open two shielded
//                                 receipts (about 25 minutes, real time).
//   node record.mjs intro   OUT   the landing: hero, the problem, how it works.
//   node record.mjs settled OUT   the Oct 19 testnet auction: two bidders open
//                                 their receipts, the public settlement check
//                                 runs, two cheats are refused.
//
// Output: OUT-frames/ (1920x1080 JPEG frames as the page repaints, each with the
// browser's own timestamp) and OUT.json with those times, a mark in seconds from
// the start of the recording at every action and every result, and the result
// texts. scripts/render_demo_video_v3.py turns the frames into real-time video. The editor cuts and speeds up only between marks, and labels
// the real duration of each sped-up span. The pointer is drawn into the page
// for the viewer; clicks go through the real buttons. Synthetic demo bids only.
import puppeteer from 'puppeteer-core';
import fs from 'node:fs';

const BRAVE = '/Applications/Brave Browser.app/Contents/MacOS/Brave Browser';
const BASE = process.env.DARKCALL_URL || 'http://127.0.0.1:8877/';
const [mode, out] = process.argv.slice(2);
if (!['live', 'run', 'intro', 'settled'].includes(mode) || !out) throw new Error('usage: node record.mjs live|run|intro|settled OUTPUT_PREFIX');
const frameDir = out + '-frames';
fs.mkdirSync(frameDir, { mode: 0o700 });

const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
const browser = await puppeteer.launch({ executablePath: process.env.DARKCALL_BROWSER || BRAVE, headless: true, protocolTimeout: 1800000,
  args: ['--no-first-run', '--disable-extensions', '--hide-scrollbars'],
  defaultViewport: { width: 1280, height: 720, deviceScaleFactor: 1.5 } });
const page = await browser.newPage();
const record = { format: 1, mode, base: BASE, viewport: '1280x720 CSS px at 1.5x = 1920x1080', started_at: null, marks: [], results: {}, frames: [] };
let cdp, origin;
const mark = label => { const t = Math.round((Date.now() - origin) / 100) / 10; record.marks.push({ t, label }); console.log(`[${t}s] ${label}`); };

// A plain arrow pointer and click ring, built with DOM and CSSOM calls only.
async function pointer() {
  await page.evaluate(() => {
    if (document.getElementById('rec-pointer')) return;
    const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    svg.id = 'rec-pointer'; svg.setAttribute('width', '26'); svg.setAttribute('height', '26'); svg.setAttribute('viewBox', '0 0 26 26');
    const path = document.createElementNS('http://www.w3.org/2000/svg', 'path');
    path.setAttribute('d', 'M3 2 L3 21 L8.2 16.2 L11.6 24 L15 22.5 L11.7 14.9 L18.6 14.9 Z');
    path.setAttribute('fill', '#141413'); path.setAttribute('stroke', '#ffffff'); path.setAttribute('stroke-width', '1.6'); path.setAttribute('stroke-linejoin', 'round');
    svg.append(path);
    Object.assign(svg.style, { position: 'fixed', left: '0', top: '0', zIndex: '2147483647', pointerEvents: 'none',
      transform: 'translate(900px, 560px)', transition: 'transform 650ms cubic-bezier(.4,0,.2,1)', filter: 'drop-shadow(0 1px 2px rgba(0,0,0,.3))' });
    const ring = document.createElement('div'); ring.id = 'rec-ring';
    Object.assign(ring.style, { position: 'fixed', left: '0', top: '0', width: '34px', height: '34px', margin: '-17px 0 0 -17px', borderRadius: '50%',
      border: '3px solid #f4b728', zIndex: '2147483646', pointerEvents: 'none', opacity: '0', transition: 'opacity 380ms, transform 380ms' });
    document.body.append(svg, ring);
  });
}
async function moveTo(selector) {
  await page.$eval(selector, node => node.scrollIntoView({ block: 'center', behavior: 'smooth' }));
  await sleep(700);
  const box = await (await page.$(selector)).boundingBox();
  const x = box.x + Math.min(box.width / 2, 60), y = box.y + box.height / 2;
  await page.evaluate((x, y) => { document.getElementById('rec-pointer').style.transform = `translate(${x - 3}px, ${y - 2}px)`; }, x, y);
  await sleep(750);
  return { x, y };
}
async function click(selector) {
  const { x, y } = await moveTo(selector);
  await page.evaluate((x, y) => {
    const ring = document.getElementById('rec-ring');
    ring.style.transition = 'none'; ring.style.left = x + 'px'; ring.style.top = y + 'px'; ring.style.opacity = '1'; ring.style.transform = 'scale(.6)';
    requestAnimationFrame(() => { ring.style.transition = 'opacity 450ms, transform 450ms'; ring.style.opacity = '0'; ring.style.transform = 'scale(1.5)'; });
  }, x, y);
  await page.click(selector);
  await sleep(300);
}
async function typeInto(selector, value) {
  await click(selector);
  await page.$eval(selector, node => node.select());
  await page.keyboard.type(value, { delay: 120 });
  await sleep(400);
}
async function view(selector, block = 'start') {
  await page.$eval(selector, (node, b) => node.scrollIntoView({ block: b, behavior: 'smooth' }), block);
  await sleep(1000);
}
const text = selector => page.$eval(selector, node => node.textContent.trim().replace(/\s+/g, ' '));
async function waitText(selector, needle, timeout) {
  await page.waitForFunction((s, t) => (document.querySelector(s)?.textContent || '').includes(t), { timeout, polling: 250 }, selector, needle);
}
async function start(url) {
  await page.goto(url, { waitUntil: 'networkidle0' });
  await pointer();
  await sleep(500);
  // Frames carry the compositor's timestamp; under proving load they arrive in
  // bursts, so the editor places each one at its own time, not at a fixed rate.
  cdp = await page.createCDPSession();
  cdp.on('Page.screencastFrame', ({ data, metadata, sessionId }) => {
    const file = String(record.frames.length).padStart(6, '0') + '.jpg';
    fs.writeFileSync(`${frameDir}/${file}`, Buffer.from(data, 'base64'));
    record.frames.push({ file, t: Math.round((metadata.timestamp * 1000 - origin) * 10) / 10000 });
    cdp.send('Page.screencastFrameAck', { sessionId }).catch(() => {});
  });
  origin = Date.now(); record.started_at = new Date(origin).toISOString();
  await cdp.send('Page.startScreencast', { format: 'jpeg', quality: 90, maxWidth: 1920, maxHeight: 1080, everyNthFrame: 1 });
  mark('start');
}

async function live() {
  await start(BASE);
  await sleep(2500);
  await click('#start-auction');
  await page.waitForFunction(() => !document.querySelector('#create-auction').disabled, { timeout: 30000 });
  await pointer();
  mark('create-form');
  await typeInto('#create-reserve', '10');
  // The close time is typed by the page itself (now + 2 min); bring it to now + 75 s.
  await page.$eval('#create-close', node => {
    const close = new Date(Date.now() + 75000);
    node.value = new Date(close - close.getTimezoneOffset() * 60000).toISOString().slice(0, 19);
    node.dispatchEvent(new Event('input', { bubbles: true })); node.dispatchEvent(new Event('change', { bubbles: true }));
  });
  await sleep(600);
  await click('#create-auction');
  await page.waitForSelector('#auction-room:not([hidden])', { timeout: 60000 });
  record.results.auction_id = (await page.evaluate(() => location.hash)).replace('#auction=', '');
  record.results.terms = await text('#room-terms');
  mark('room');
  await sleep(1500);
  for (const [index, bid] of [77, 61, 43, 21].entries()) {
    await click(`[data-bid="${bid}"]`);
    await click('#seal-bid');
    mark(`seal-${index + 1}-start`);
    await waitText('#room-count', `${index + 1} / 4`, 180000);
    mark(`seal-${index + 1}-done`);
    if (index === 0) { await view('#bid-accepted', 'center'); record.results.first_acceptance = await text('#bid-accepted'); await sleep(3500); }
  }
  await view('#sealed-slots', 'center');
  await sleep(2500);
  mark('wait-close');
  await page.waitForFunction(() => { const b = document.querySelector('#close-auction'); return b && !b.disabled && !b.hidden; }, { timeout: 240000, polling: 500 });
  mark('close-ready');
  await click('#close-auction');
  mark('close-start');
  await waitText('#close-status', 'certified', 120000);
  mark('close-done');
  record.results.close = await text('#close-status');
  await sleep(2500);
  await click('#clear-auction');
  mark('clear-start');
  await sleep(1500);
  await view('#clearing-section', 'start');
  await waitText('#clearing-status', 'Auction verified', 900000);
  mark('clear-done');
  record.results.clearing = await text('#clearing-status');
  await view('#fresh-outcome', 'center');
  record.results.outcome = await text('#fresh-outcome');
  await sleep(4000);
  await page.select('#my-ticket', '0');
  await click('#open-fresh-result');
  mark('result-start');
  await waitText('#fresh-result', 'verified', 180000);
  mark('result-done');
  record.results.own_result = await text('#fresh-result');
  await view('#fresh-result', 'center');
  await sleep(4500);
  mark('end');
}

async function run() {
  await start(new URL('demo/run', BASE).href);
  await page.waitForFunction(() => !document.querySelector('#create-auction').disabled, { timeout: 30000 });
  await pointer();
  await sleep(2500);
  mark('create-form');
  await click('#create-auction');
  await page.waitForSelector('#auction-room:not([hidden])', { timeout: 60000 });
  record.results.terms = await text('#room-terms');
  mark('room');
  await sleep(1500);
  for (const [index, bid] of ['82', '64', '47', '35'].entries()) {
    await typeInto('#maximum-bid', bid);
    await click('#seal-bid');
    mark(`seal-${index + 1}-start`);
    await waitText('#room-count', `${index + 1} / 4`, 180000);
    mark(`seal-${index + 1}-done`);
    if (index === 0) { await view('#bid-accepted', 'center'); record.results.first_acceptance = await text('#bid-accepted'); await sleep(3000); }
  }
  await view('#sealed-slots', 'center');
  mark('wait-close');
  await page.waitForFunction(() => { const b = document.querySelector('#close-auction'); return b && !b.disabled && !b.hidden; }, { timeout: 240000, polling: 500 });
  mark('close-ready');
  await click('#close-auction');
  mark('close-start');
  await waitText('#close-status', 'certified', 120000);
  mark('close-done');
  await sleep(2000);
  await click('#fund-auction');
  mark('fund-start');
  await view('#fund-section', 'start');
  await waitText('#fund-status', 'Funded and admitted', 1800000);
  mark('fund-done');
  record.results.funding = await text('#fund-section');
  await view('#fund-facts', 'center');
  await sleep(4000);
  await page.waitForFunction(() => { const b = document.querySelector('#clear-auction'); return b && !b.disabled && !b.hidden; }, { timeout: 120000, polling: 500 });
  await click('#clear-auction');
  mark('clear-start');
  await view('#clearing-section', 'start');
  await waitText('#clearing-status', 'Auction verified', 900000);
  mark('clear-done');
  record.results.clearing = await text('#clearing-status');
  record.results.outcome = await text('#fresh-outcome');
  await sleep(3000);
  await page.select('#my-ticket', '0');
  await click('#open-fresh-result');
  mark('result-start');
  await waitText('#fresh-result', 'verified', 180000);
  mark('result-done');
  record.results.own_result = await text('#fresh-result');
  await view('#fresh-result', 'center');
  await sleep(3500);
  await page.waitForFunction(() => { const b = document.querySelector('#settle-auction'); return b && !b.disabled && !b.hidden; }, { timeout: 120000, polling: 500 });
  await click('#settle-auction');
  mark('settle-start');
  await view('#settle-section', 'start');
  await waitText('#settle-status', 'Settled and publicly verified', 2400000);
  mark('settle-done');
  record.results.settlement = await text('#settle-section');
  await sleep(1500);
  await view('#settle-txid', 'center');
  await sleep(4000);
  for (const index of ['0', '3']) {
    await page.select('#my-native-ticket', index);
    await sleep(600);
    await click('#open-native-result');
    mark(`receipt-${index}-start`);
    await page.waitForFunction(() => document.querySelector('#native-result').classList.contains('verified'), { timeout: 180000, polling: 250 });
    mark(`receipt-${index}-done`);
    record.results[`receipt_${index}`] = await text('#native-result');
    await view('#native-result', 'center');
    await sleep(4000);
  }
  await view('#sealed-slots', 'center');
  await sleep(3000);
  mark('end');
}

async function intro() {
  await start(BASE);
  await sleep(5000);
  mark('hero');
  for (const id of ['#problem', '#how', '#tech']) {
    await view(id, 'start');
    mark(id.slice(1));
    await sleep(4500);
  }
  mark('end');
}

async function settled() {
  await start(new URL('demo/settled', BASE).href);
  await waitText('#scope-description', 'testnet', 60000);
  await sleep(3000);
  await view('.workspace', 'start');
  await sleep(1500);
  for (const label of ['Bidder 01', 'Bidder 04']) {
    const value = await page.$$eval('#ticket option', (options, l) => options.find(o => o.textContent === l).value, label);
    await moveTo('#ticket');
    await page.select('#ticket', value);
    await sleep(900);
    await click('#verify');
    mark(`receipt-${label.slice(-2)}-start`);
    await page.waitForFunction(() => document.querySelector('#result').classList.contains('verified'), { timeout: 180000, polling: 250 });
    mark(`receipt-${label.slice(-2)}-done`);
    record.results[`receipt_${label.slice(-2)}`] = await text('#result');
    await view('#result', 'center');
    await sleep(4500);
  }
  await click('#settlement');
  mark('verify-start');
  await view('.settlement-strip', 'start');
  await waitText('#settlement-result', 'Verified again just now', 900000);
  mark('verify-done');
  record.results.settlement = await text('#settlement-result');
  await sleep(1200);
  await view('#settlement-claims', 'center');
  await sleep(4500);
  for (const kind of ['book', 'price']) {
    await page.waitForFunction(() => !document.querySelector('#settlement').disabled, { timeout: 900000, polling: 250 });
    await click(`[data-cheat="${kind}"]`);
    mark(`cheat-${kind}-start`);
    await view('.cheat-strip', 'start');
    await waitText('#tamper-result', 'Refused', 900000);
    mark(`cheat-${kind}-done`);
    record.results[`cheat_${kind}`] = await text('#tamper-result');
    await sleep(1000);
    await view('#cheat-detail', 'center');
    await sleep(4500);
  }
  mark('end');
}

try {
  await ({ live, run, intro, settled })[mode]();
  record.completed = true;
} catch (error) {
  record.completed = false;
  record.error = String(error).slice(0, 300);
  if (origin) mark('failed');
  await page.screenshot({ path: out + '-failure.png' }).catch(() => {});
  throw error;
} finally {
  if (cdp) await cdp.send('Page.stopScreencast').catch(() => {});
  if (origin) record.end = Math.round((Date.now() - origin) / 100) / 10;
  record.finished_at = new Date().toISOString();
  fs.writeFileSync(out + '.json', JSON.stringify(record, null, 2) + '\n', { mode: 0o600 });
  await browser.close();
}
