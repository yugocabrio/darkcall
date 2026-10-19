// Drive ONE real local DarkCall auction through the browser UI and capture stills.
//
// Create → four demo bids → certify → Fund on Zcash → Clear → own result →
// Settle on Zcash → own shielded receipts (won/lost) → receipt tamper refusal.
// Every wait is on the UI's own verified status text; nothing is simulated or
// skipped. Output: numbered 1920x1080 PNGs plus manifest.json with UTC times,
// elapsed seconds, SHA-256 and the public auction/transaction identifiers.
// Synthetic demo bids only. Requires ./demo.sh and the local NU7 node.
import puppeteer from 'puppeteer-core';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

const BRAVE = '/Applications/Brave Browser.app/Contents/MacOS/Brave Browser';
const BASE = process.env.DARKCALL_URL || 'http://127.0.0.1:8877/';
const out = process.argv[2];
if (!out) throw new Error('usage: node capture.mjs NEW_OUTPUT_DIRECTORY');
fs.mkdirSync(out, { mode: 0o700 });

const started = Date.now();
const manifest = { format: 1, scope: 'one real local auction driven through the browser UI; synthetic demo bids; isolated NU7 regtest',
  started_at: new Date(started).toISOString(), viewport: { width: 1920, height: 1080 }, bids: [77, 61, 43, 21], shots: [], facts: {} };
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
const log = message => console.log(`[${((Date.now() - started) / 1000).toFixed(1)}s] ${message}`);

const browser = await puppeteer.launch({ executablePath: process.env.DARKCALL_BROWSER || BRAVE, headless: true,
  args: ['--no-first-run', '--disable-extensions', '--hide-scrollbars'],
  defaultViewport: { width: 1920, height: 1080, deviceScaleFactor: 1 } });
const page = await browser.newPage();

async function shot(name, selector, note) {
  if (selector) await page.$eval(selector, node => node.scrollIntoView({ block: 'center' }));
  else await page.evaluate(() => window.scrollTo(0, 0));
  await sleep(700);
  const file = `${String(manifest.shots.length + 1).padStart(2, '0')}-${name}.png`;
  await page.screenshot({ path: path.join(out, file) });
  const bytes = fs.readFileSync(path.join(out, file));
  manifest.shots.push({ name, file, at: new Date().toISOString(), elapsed_seconds: Math.round((Date.now() - started) / 100) / 10,
    note, sha256: crypto.createHash('sha256').update(bytes).digest('hex') });
  log(`captured ${file}`);
}
const text = selector => page.$eval(selector, node => node.textContent);
async function waitText(selector, needle, timeout) {
  await page.waitForFunction((s, t) => (document.querySelector(s)?.textContent || '').includes(t),
    { timeout, polling: 1000 }, selector, needle);
}
async function waitEnabled(selector, timeout) {
  await page.waitForFunction(s => { const node = document.querySelector(s); return node && !node.disabled && !node.hidden; },
    { timeout, polling: 1000 }, selector);
}

try {
  await page.goto(BASE, { waitUntil: 'networkidle0' });
  manifest.chain = await page.evaluate(() => fetch('/api/state').then(r => r.json()).then(s => s.live_chain || 'regtest'));
  if (manifest.chain === 'testnet') manifest.scope = manifest.scope.replace('isolated NU7 regtest', 'public Zcash testnet (NU7)');
  await shot('landing', null, 'Landing: sealed bids, proven price, shielded settlement; a real settled auction beside the claim');

  await page.click('#start-auction');
  await waitEnabled('#create-auction', 30000);
  await shot('create', '#create-form', 'Seller terms: 3 units, reserve 20, budget 100 zatoshi, close about 2 minutes ahead');
  await page.click('#create-auction');
  await page.waitForSelector('#auction-room:not([hidden])', { timeout: 60000 });
  manifest.facts.auction_id = (await page.evaluate(() => location.hash)).replace('#auction=', '');

  for (const [index, bid] of manifest.bids.entries()) {
    await page.click(`[data-bid="${bid}"]`);
    await page.click('#seal-bid');
    await waitText('#room-count', `${index + 1} / 4`, 120000);
    if (index === 0) await shot('first-bid', '#bid-accepted', 'Bid sealed: client ZK proof, 3 share pairs, 3 operator signatures');
  }
  await shot('sealed', '#sealed-slots', 'Four sealed tickets; no value is shown');

  await waitEnabled('#close-auction', 240000);
  await page.click('#close-auction');
  await waitText('#close-status', 'certified', 60000);
  await shot('closed', '#fund-section', 'Closed book certified; clearing waits for funding');

  await page.click('#fund-auction');
  await sleep(15000);
  await shot('funding', '#fund-section', 'Funding: real shielded deposits on regtest, then chain admission');
  // Regtest funds in about 2 minutes; public testnet adds a 10-minute window and real confirmations.
  await waitText('#fund-status', 'Funded and admitted', 2400000);
  manifest.facts.deposit_txid = await text('#fund-deposit');
  await shot('funded', '#fund-section', 'Deposits locked; same proofs and shares re-verified; same root');

  await page.click('#clear-auction');
  await sleep(1500);
  await shot('clearing', '#clearing-section', 'Three operators clear the secret shares (REP3) and prove it (coNoir)');
  await waitText('#clearing-status', 'Auction verified', 300000);
  manifest.facts.clearing = await text('#clearing-status');
  await shot('cleared', '#fresh-outcome', 'One public price; no bid reconstructed');

  await page.select('#my-ticket', '0');
  await page.click('#open-fresh-result');
  await waitText('#fresh-result', 'verified', 120000);
  await shot('own-result', '#fresh-outcome', 'Ticket 01 opens only its own verified result');

  await page.click('#settle-auction');
  await sleep(12000);
  await shot('settle-start', '#settle-steps', 'Settle: MPC delivery on the same shares');
  const seen = new Set();
  for (;;) {
    const status = await text('#settle-status');
    if (status.includes('Settled')) break;
    if (status.includes('stopped')) throw new Error('Settlement stopped: ' + status);
    const running = await page.$$eval('#settle-steps li', items => items.map(item => item.className + ':' + item.id + ':' + item.querySelector('em').textContent));
    const active = running.find(item => item.startsWith('running'));
    const key = active ? active.split(':')[1] + (active.includes('row 4') ? '-row4' : '') : '';
    if (key && !seen.has(key)) { seen.add(key); await shot('settle-' + key.replace('settle-step-', ''), '#settle-steps', 'Settlement in progress: ' + active.split(':').slice(2).join(':')); }
    await sleep(5000);
  }
  manifest.facts.txid = await text('#settle-txid');
  manifest.facts.same_book = await text('#settle-root');
  manifest.facts.outcome = await text('#settle-price');
  manifest.facts.settle_status = await text('#settle-status');
  await shot('settled', '#settle-steps', 'Settled and publicly verified');
  await shot('who-learned', '.knowledge-grid', 'Who learned what');

  await page.select('#my-native-ticket', '0');
  await page.click('#open-native-result');
  await waitText('#native-result', 'Decrypted from', 120000);
  await shot('receipt-won', '#native-result', 'Ticket 01 decrypts its own shielded receipt: WON');
  await page.select('#my-native-ticket', '3');
  await page.click('#open-native-result');
  await waitText('#native-result', 'No unit this time', 120000);
  await shot('receipt-lost', '#native-result', 'Ticket 04 decrypts its own shielded receipt: LOST, full refund');
  await page.click('#tamper-native');
  await waitText('#native-tamper-result', 'rejected', 120000);
  await shot('receipt-tamper', '#native-tamper-result', 'Corrupted receipt proof rejected before the key is read');
  manifest.completed = true;
} catch (error) {
  manifest.completed = false;
  manifest.error = String(error).slice(0, 300);
  await page.screenshot({ path: path.join(out, 'failure.png') }).catch(() => {});
  throw error;
} finally {
  manifest.finished_at = new Date().toISOString();
  manifest.total_seconds = Math.round((Date.now() - started) / 100) / 10;
  fs.writeFileSync(path.join(out, 'manifest.json'), JSON.stringify(manifest, null, 2) + '\n', { mode: 0o600 });
  await browser.close();
}
