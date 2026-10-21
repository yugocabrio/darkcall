// Drive the spike page in headless Brave: type a bid, seal, and record every request.
// Any request to another origin is aborted and reported.
// Usage: node drive.mjs <puppeteer-core dir> <url> <bid>
const [puppeteerDir, url, bid] = process.argv.slice(2);
const { default: puppeteer } = await import(puppeteerDir + '/lib/esm/puppeteer/puppeteer-core.js');
const browser = await puppeteer.launch({ executablePath: '/Applications/Brave Browser.app/Contents/MacOS/Brave Browser',
  headless: true, args: ['--no-first-run', '--disable-extensions', '--incognito'] });
const page = await browser.newPage();
const origin = new URL(url).origin, requests = [], foreign = [];
await page.setRequestInterception(true);
page.on('request', request => {
  const target = request.url();
  const local = target.startsWith(origin) || target.startsWith('data:') || target.startsWith('blob:');
  requests.push({ method: request.method(), url: target.startsWith('data:') ? 'data:…' : target.replace(origin, ''),
    body_bytes: (request.postData() || '').length });
  if (!local) { foreign.push(target); return request.abort(); }
  request.continue();
});
const consoleErrors = [];
page.on('console', m => m.type() === 'error' && consoleErrors.push(m.text().slice(0, 300)));
await page.goto(url, { waitUntil: 'load' });
await page.type('#bid', bid);
await page.click('#seal');
await page.waitForFunction(() => window.__result, { timeout: 300000 });
const result = await page.evaluate(() => window.__result);
const post = requests.find(r => r.method === 'POST');
console.log(JSON.stringify({ result, requests: requests.length, foreign_requests_blocked: foreign,
  post, console_errors: consoleErrors.slice(0, 5) }, null, 1));
await browser.close();
