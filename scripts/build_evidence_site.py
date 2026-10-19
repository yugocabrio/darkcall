#!/usr/bin/env python3
"""Build the static evidence page (docs/index.html + docs/site/) for GitHub Pages.

Every number and identifier comes from committed records: the capture manifest
(demo/media/v2/manifest.json) and the evidence JSON. Nothing is deployed; the
page has no scripts, trackers or external resources. Rebuild after new evidence.
"""
import html
import json
import pathlib
import re
import shutil
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[1]
DOCS = ROOT / 'docs'
SITE = DOCS / 'site'
MEDIA = ROOT / 'demo/media/v2'
REPO = 'https://github.com/yugocabrio/ZECATHON/blob/main/'
FRAMES = ['01-landing', '02-create', '04-sealed', '07-funded', '09-cleared', '10-own-result',
          '16-settled', '18-receipt-won', '19-receipt-lost', '20-receipt-tamper']
e = html.escape


def mmss(seconds):
    minutes, rest = divmod(round(seconds), 60)
    return f'{minutes}:{rest:02d}'


def short(value):
    return value[:10] + '…' + value[-8:]


def vtt(srt):
    body = re.sub(r'(\d\d:\d\d:\d\d),(\d\d\d)', r'\1.\2', srt.strip())
    return 'WEBVTT\n\n' + re.sub(r'^\d+\n', '', body, flags=re.M) + '\n'


def build():
    manifest = json.loads((MEDIA / 'manifest.json').read_text())
    evidence = json.loads((ROOT / 'evidence/2026-10-18-fresh-book-native-settlement.json').read_text())
    clean = json.loads((ROOT / 'evidence/2026-10-18-clean-room-linux-verification.json').read_text())['run_head_rebuild']
    run = evidence['video_capture_run']
    testnet = json.loads((ROOT / 'evidence/2026-10-19-public-testnet-settlement.json').read_text())
    # The protocol explanation is the demo page's own section, so the two never drift.
    page = (ROOT / 'demo/index.html').read_text()
    how_html = re.search(r'<section class="how".*?</section>', page, re.S).group(0)
    # No scripts here: show the walkthrough with every stage on and no controls.
    how_html = re.sub(r'<label class="walk-toggle">.*?</label>', '', how_html, flags=re.S)
    how_html = re.sub(r'<div class="walk-controls">.*?</div>\s*<p id="walk-caption"[^>]*></p>', '', how_html, flags=re.S)
    how_html = how_html.replace('class="walk-stage" id="walk-stage" data-step="1"', 'class="walk-stage static"')
    style = (ROOT / 'demo/style.css').read_text()
    how_css = style[style.index('/* How it works'):style.index('/* Why it is different')]
    facts, at = manifest['facts'], {s['name']: s['elapsed_seconds'] for s in manifest['shots']}
    on_testnet = manifest.get('chain') == 'testnet'
    chain_name = 'public Zcash testnet (NU7)' if on_testnet else 'an isolated NU7 regtest'
    accepted_by = 'the testnet accepted it' if on_testnet else 'Zebra accepted it'
    receipts = {r['ticket']: r for r in run['receipts_seen_in_frames']}
    price, sold = re.match(r'(\d+) zatoshi × (\d+) sold', facts['outcome']).groups()
    clearing_s = re.search(r'([\d.]+)s', facts['clearing']).group(1)
    root_short = re.search(r'([0-9a-f]+…[0-9a-f]+)', facts['same_book']).group(1)

    if SITE.exists():
        shutil.rmtree(SITE)
    SITE.mkdir(parents=True)
    for name in FRAMES:
        # Step frames: crop to the app's centred content column so its text stays legible.
        # The landing frame stays full-width as the video poster.
        crop = [] if name == '01-landing' else ['--cropToHeightWidth', '1080', '1120']
        subprocess.run(['sips', '-s', 'format', 'jpeg', '-s', 'formatOptions', '82', *crop,
                        str(MEDIA / f'{name}.jpg'), '--out', str(SITE / f'{name}.jpg')], check=True, capture_output=True)
    shutil.copyfile(MEDIA / 'darkcall-demo-v2.mp4', SITE / 'darkcall-demo-v2.mp4')
    (SITE / 'darkcall-demo-v2.vtt').write_text(vtt((MEDIA / 'darkcall-demo-v2.srt').read_text()))
    (DOCS / '.nojekyll').write_text('')
    # Self-hosted IBM Plex (SIL OFL 1.1), the same files the demo serves.
    (SITE / 'fonts').mkdir()
    for font in sorted((ROOT / 'demo/fonts').iterdir()):
        shutil.copyfile(font, SITE / 'fonts' / font.name)

    steps = [
        ('01', 'Create', '02-create', at['create'], 'The seller sets 3 identical units, reserve 20 and a deposit budget of 100 zatoshi. Bidding closes in two minutes.'),
        ('02', 'Seal four bids', '04-sealed', at['sealed'],
         'Each bid becomes a commitment plus three REP3 secret shares, one per operator. A client zero-knowledge proof binds them, and the three operators check it and sign. The page shows no values.'),
        ('03', 'Fund on Zcash', '07-funded', at['funded'],
         f'After the close, every ticket gets a real shielded deposit on {chain_name} (tx {short(facts["deposit_txid"])}). The operators re-verify the same proofs and shares, then sign the book again over the same root. Took {mmss(at["funded"] - at["funding"] + 15)}.'),
        ('04', 'Clear by MPC', '09-cleared', at['cleared'],
         f'The three operators compute on secret shares and jointly prove the result (coNoir), verified in {clearing_s} s. Price {price} zatoshi, {sold} of 3 sold. No bid was reconstructed.'),
        ('05', 'Only your result', '10-own-result', at['own-result'],
         'Ticket 01 opens only its own verified outcome. No other ticket\'s result is shown.'),
        ('06', 'Settle on Zcash', '16-settled', at['settled'],
         f'One shielded transaction pays the seller and refunds all four bidders (tx {short(facts["txid"])}). It has six Halo2 output bindings and 2-of-3 FROST signatures, {accepted_by} with 2 confirmations, and a public verifier replayed it. Took {mmss(at["settled"] - at["settle-start"] + 12)}.'),
        ('07', 'A winner\'s receipt', '18-receipt-won', at['receipt-won'],
         f'Ticket 01 decrypts its own memo with its own viewing key: {"won" if receipts["01"]["won"] else "lost"}, {receipts["01"]["refund"]} zatoshi refunded.'),
        ('08', 'A losing ticket', '19-receipt-lost', at['receipt-lost'],
         f'Ticket 04 decrypts its own memo: {"won" if receipts["04"]["won"] else "lost"}, the full {receipts["04"]["refund"]} zatoshi refunded.'),
        ('09', 'Try to cheat', '20-receipt-tamper', at['receipt-tamper'],
         'Flipping one byte of the receipt proof makes the fixed verifier reject it before the viewing key is read.'),
    ]
    step_html = '\n'.join(
        f'''<article class="step"><a class="shot" href="site/{img}.jpg"><img src="site/{img}.jpg" alt="{e(title)}" loading="lazy" width="1120" height="1080"></a>
<div class="step-text"><span class="step-no">{no}</span><h3>{e(title)}</h3><p>{e(text)}</p><span class="clock">t + {mmss(t)}</span></div></article>'''
        for no, title, img, t, text in steps)

    video_bundle = run['public_bundle']
    tn_bundle = testnet['public_verification']
    browser_run = testnet['browser_video_run']
    bundles = [(r['bundle'], r['txid'], r['scope'], f'clean Linux container, {r["seconds"]:.0f} s, tamper refused') for r in clean['results']]
    bundle_rows = '\n'.join(f'<tr><td><code>{e(b)}</code></td><td><code>{e(short(t))}</code></td><td>{e(s)}</td><td>{e(v)}</td></tr>'
                            for b, t, s, v in bundles)
    page = TEMPLATE.format(
        video_seconds=run['video']['seconds'], auction=e(facts['auction_id']), auction_short=e(short(facts['auction_id'])),
        deposit=e(facts['deposit_txid']), txid=e(facts['txid']), root=e(root_short), price=price, sold=sold,
        total=mmss(manifest['total_seconds']), steps=step_html, winner_refund=100 - int(price), seller=int(price) * int(sold), bundle_rows=bundle_rows, repo=REPO,
        clean_env=e(clean['environment']), clearing_s=clearing_s,
        bundle_path=e(tn_bundle['bundle']), authority_path=e(tn_bundle['authority']),
        tn_deposit=e(browser_run['deposit_txid']), tn_deposit_height=f"{browser_run['deposit_block']['height']:,}",
        tn_txid=e(browser_run['settlement_txid']), tn_height=f"{browser_run['settlement_block']['height']:,}",
        tn_settle_s=round(browser_run['settlement_driver_seconds']), tn_check_s=round(browser_run['public_verification_seconds']),
        tn_price=testnet['clearing']['price'], tn_sold=testnet['clearing']['sold'],
        tn_api_txid=e(short(testnet['settlement']['txid'])),
        how_html=how_html, how_css=how_css)
    (DOCS / 'index.html').write_text(page)
    return {'page': str(DOCS / 'index.html'), 'frames': len(FRAMES), 'site_bytes': sum(p.stat().st_size for p in SITE.rglob('*') if p.is_file())}


TEMPLATE = '''<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>DarkCall evidence</title>
<meta name="description" content="DarkCall: sealed-bid auctions on Zcash. One real auction from creation to shielded receipts, with its transactions and verification.">
<style>
@font-face {{font-family:"Plex";src:url("site/fonts/ibm-plex-sans-latin-400-normal.woff2") format("woff2");font-weight:400;font-display:swap}}
@font-face {{font-family:"Plex";src:url("site/fonts/ibm-plex-sans-latin-500-normal.woff2") format("woff2");font-weight:500;font-display:swap}}
@font-face {{font-family:"Plex";src:url("site/fonts/ibm-plex-sans-latin-600-normal.woff2") format("woff2");font-weight:600;font-display:swap}}
@font-face {{font-family:"Plex Mono";src:url("site/fonts/ibm-plex-mono-latin-400-normal.woff2") format("woff2");font-weight:400;font-display:swap}}
@font-face {{font-family:"Plex Mono";src:url("site/fonts/ibm-plex-mono-latin-500-normal.woff2") format("woff2");font-weight:500;font-display:swap}}
:root {{--paper:#f6f4ee;--surface:#fff;--surface-2:#faf9f5;--ink:#141413;--ink-2:#4a4944;--ink-3:#706e66;--line:#e6e2d7;--line-strong:#cfc9ba;
  --gold:#f4b728;--gold-ink:#8a6408;--gold-soft:#fdf3d7;--green:#1e7a4c;
  --sans:"Plex",-apple-system,BlinkMacSystemFont,"Helvetica Neue",Arial,sans-serif;--mono:"Plex Mono",ui-monospace,SFMono-Regular,Menlo,monospace}}
* {{box-sizing:border-box}}
html {{background:var(--paper)}}
body {{margin:0;background:var(--paper);color:var(--ink);font:16px/1.6 var(--sans);-webkit-font-smoothing:antialiased}}
a {{color:var(--ink);text-underline-offset:3px}}
code {{font:13px var(--mono);color:var(--ink);overflow-wrap:anywhere}}
.wrap {{max-width:1120px;margin:0 auto;padding:0 32px}}
header {{display:flex;justify-content:space-between;align-items:center;gap:16px;padding:22px 0}}
.brand {{display:inline-flex;align-items:center;gap:10px;font-weight:600;font-size:18px;color:var(--ink);text-decoration:none}}
.brand::before {{content:"";width:12px;height:12px;background:var(--gold);transform:rotate(45deg);border-radius:2px}}
.pill {{font-size:13px;color:var(--ink-2);border:1px solid var(--line-strong);border-radius:999px;padding:4px 12px}}
.kicker {{margin:64px 0 18px;color:var(--gold-ink);font-weight:500;font-size:14px}}
h1 {{font-size:clamp(40px,6vw,72px);line-height:1.03;letter-spacing:-.04em;font-weight:600;margin:0}}
h2 {{font-size:clamp(28px,3.6vw,40px);line-height:1.12;letter-spacing:-.03em;font-weight:600;margin:104px 0 10px}}
h3 {{font-size:20px;font-weight:600;letter-spacing:-.01em;margin:4px 0 8px}}
.lead {{font-size:19px;color:var(--ink-2);max-width:700px;margin:24px 0 0}}
.sub {{color:var(--ink-2);font-size:17px;max-width:680px;margin:0 0 28px}}
.scope {{margin:28px 0 0;padding:14px 18px;border:1px solid var(--line);border-radius:12px;background:var(--surface);color:var(--ink-2);font-size:14px;max-width:820px}}
.scope b {{color:var(--ink);font-weight:600}}
video {{display:block;width:100%;margin:40px 0 10px;border-radius:16px;background:#000;box-shadow:0 1px 2px rgba(20,20,19,.06),0 24px 60px rgba(20,20,19,.10)}}
.caption {{font-size:13px;color:var(--ink-3)}}
.compare-wrap {{background:var(--surface);border:1px solid var(--line);border-radius:18px;overflow:hidden}}
.compare {{width:100%;border-collapse:collapse;font-size:14px;margin:0}}
.compare th,.compare td {{text-align:left;padding:15px 18px;border-bottom:1px solid var(--line);vertical-align:top}}
.compare tbody tr:last-child > * {{border-bottom:0}}
.compare thead th {{font-size:13px;font-weight:500;color:var(--ink-3);background:var(--surface-2)}}
.compare tbody th {{font-weight:500;color:var(--ink);width:26%}}
.compare td {{color:var(--ink-2)}}
.compare td.no {{color:var(--ink-3)}}
.compare .us {{background:var(--gold-soft);color:var(--ink);font-weight:500}}
.compare thead .us {{background:var(--gold);font-weight:600}}
.measured {{display:grid;grid-template-columns:repeat(4,1fr);gap:28px;margin-top:48px}}
.measured div {{border-top:2px solid var(--ink);padding-top:14px}}
.measured strong {{display:block;font-size:40px;line-height:1.05;font-weight:500;letter-spacing:-.04em}}
.measured span {{display:block;margin-top:8px;color:var(--ink-2);font-size:14px;line-height:1.5}}
.facts {{display:grid;grid-template-columns:220px 1fr;gap:10px 20px;margin:0;padding:22px 24px;border:1px solid var(--line);border-radius:16px;background:var(--surface)}}
.facts dt {{color:var(--ink-3);font-size:14px}}
.facts dd {{margin:0}}
.step {{display:grid;grid-template-columns:minmax(0,1.15fr) minmax(0,1fr);gap:40px;align-items:center;padding:36px 0;border-top:1px solid var(--line)}}
.facts + .step {{margin-top:28px}}
.shot img {{display:block;width:100%;height:auto;border:1px solid var(--line);border-radius:12px;background:var(--surface)}}
.step-no {{font:12px var(--mono);color:var(--gold-ink)}}
.step p {{margin:0 0 10px;color:var(--ink-2)}}
.clock {{font:12px var(--mono);color:var(--ink-3)}}
.grid4 {{display:grid;grid-template-columns:repeat(4,1fr);gap:12px}}
.grid4 div {{border:1px solid var(--line);border-radius:14px;padding:18px;background:var(--surface)}}
.grid4 b {{font-weight:600;font-size:15px}}
.grid4 p {{margin:6px 0 0;font-size:14px;color:var(--ink-2)}}
pre {{margin:14px 0;padding:16px 18px;border-radius:12px;background:var(--ink);overflow-x:auto}}
pre code {{color:#f1efe8}}
.table-scroll {{margin-top:18px;background:var(--surface);border:1px solid var(--line);border-radius:16px;overflow:hidden}}
table {{width:100%;border-collapse:collapse;font-size:14px}}
.bundles th,.bundles td {{text-align:left;padding:12px 16px;border-bottom:1px solid var(--line);vertical-align:top}}
.bundles tbody tr:last-child td {{border-bottom:0}}
.bundles th {{color:var(--ink-3);font-weight:500;background:var(--surface-2)}}
.limits {{padding-left:20px;max-width:820px}}
.limits li {{margin:8px 0;color:var(--ink-2)}}
footer {{margin:104px 0 48px;padding-top:24px;border-top:1px solid var(--line);font-size:14px;color:var(--ink-3);display:flex;flex-wrap:wrap;gap:8px 22px}}
footer a {{color:var(--ink-2)}}
{how_css}
.lead-key {{font-size:18px;line-height:1.55;max-width:660px}}
.lead-key b {{color:var(--ink);font-weight:600}}
.hero-how {{list-style:none;margin:22px 0 0;padding:16px 0 2px;border-top:1px solid var(--line);display:grid;gap:14px;max-width:660px}}
.hero-how li {{display:grid;gap:4px}}
.hero-cat {{display:flex;align-items:center;gap:8px;font-size:12px;font-weight:600;letter-spacing:.06em;text-transform:uppercase;color:var(--ink-3)}}
.hero-tag {{font:500 12px var(--mono);letter-spacing:0;text-transform:none;color:var(--gold-ink);background:var(--gold-soft);border-radius:6px;padding:1px 7px}}
.hero-how p {{margin:0;font-size:15px;line-height:1.55;color:var(--ink-2)}}
.hero-how b {{color:var(--ink);font-weight:600}}
.how {{margin-top:104px}}
.walk-stage.static .w-from {{opacity:1;transform:none}}
.walk-stage.static .w-chips i {{opacity:1;transform:none}}
.how h2 {{margin:0}}
.section-lead {{font-size:17px;color:var(--ink-2);margin:12px 0 0;max-width:680px}}
@media (max-width:760px) {{
  .lead-key {{font-size:17px}}
  .wrap {{padding:0 16px}}
  .kicker {{margin-top:36px}}
  .lead {{font-size:17px}}
  h2 {{margin-top:72px}}
  .grid4 {{grid-template-columns:1fr}}
  .measured {{grid-template-columns:1fr 1fr;gap:24px 18px}}
  .measured strong {{font-size:32px}}
  .facts {{grid-template-columns:1fr;gap:2px}}
  .facts dd {{margin-bottom:10px}}
  .step {{grid-template-columns:1fr;gap:16px}}
  .compare-wrap,.table-scroll {{overflow-x:auto}}
  .compare {{min-width:640px}}
  .bundles {{min-width:680px}}
  .pill {{display:none}}
}}
</style>
</head>
<body>
<div class="wrap">
<header><a class="brand" href="{repo}README.md">DarkCall</a><span class="pill">ZECATHON 2026 · Private Markets</span></header>

<p class="kicker">Verifiable sealed-bid auctions on Zcash</p>
<h1>No bid is ever opened.<br>The price is still proven.<br>Settled on Zcash.</h1>
<p class="lead lead-key">DarkCall is a sealed-bid auction that never opens a bid: not at the close, and not for any single operator. <b>Even the zero-knowledge proof of the clearing price is generated without opening the bids</b>, and one shielded Zcash transaction pays the seller and refunds every bidder exactly as proven.</p>
<ol class="hero-how">
<li><span class="hero-cat"><span class="hero-tag">REP3</span>Multi-party computation</span><p><b>Split.</b> Each bid becomes secret shares; no single operator can read one.</p></li>
<li><span class="hero-cat"><span class="hero-tag">coNoir</span>Multi-party ZK proving</span><p><b>Prove.</b> The operators compute the clearing price and jointly generate one zero-knowledge proof from those shares; no single prover ever sees the bid book.</p></li>
<li><span class="hero-cat"><span class="hero-tag">Halo2</span>Proof-bound shielded settlement</span><p><b>Pay.</b> Six proofs bind the proven payouts to the outputs of one shielded Zcash transaction.</p></li>
</ol>
<p class="scope"><b>Demo scope, up front.</b> The one public number is the clearing price, which by the auction rule equals the highest losing bid or the reserve. Every party runs on one machine, and each bidder's own client, the only party that sees the typed bid, runs as a local service on that machine; operators receive shares and a proof. The video and the walkthrough below are one auction on public Zcash testnet, run from the browser; earlier runs used an isolated regtest. No mainnet funds. The protocol keeps bids from the operators and the public; this demo shows that machinery working, not isolation between separate parties.</p>

<video controls preload="metadata" poster="site/01-landing.jpg">
<source src="site/darkcall-demo-v2.mp4" type="video/mp4">
<track kind="captions" src="site/darkcall-demo-v2.vtt" srclang="en" label="English" default>
</video>
<p class="caption">{video_seconds} s, edited from one real auction ({auction_short}). Every time cut states the real elapsed time, and every frame is SHA-256-checked against the <a href="{repo}demo/media/v2/manifest.json">capture manifest</a>.</p>

{how_html}

<h2>Settled on public Zcash testnet.</h2>
<p class="sub">On October 19 the auction in the video was created, funded, cleared and settled by clicking through the browser on standard public testnet (NU7), with testnet coins and the same admission, proofs and signing as everything below.</p>
<dl class="facts">
<dt>Deposits</dt><dd><code>{tn_deposit}</code><br>block {tn_deposit_height}, two confirmations; four client proofs and twelve share pairs re-verified</dd>
<dt>Clearing</dt><dd>REP3 price {tn_price} zatoshi, {tn_sold} of 3 sold, after funding</dd>
<dt>Settlement</dt><dd><code>{tn_txid}</code><br>block {tn_height}, two confirmations; six output bindings and 2-of-3 FROST, {tn_settle_s} s</dd>
<dt>Public check</dt><dd>{tn_check_s} s with no secrets; a corrupted receipt proof is refused</dd>
<dt>Own receipts</dt><dd>Ticket 01 won, refund 79; Ticket 04 lost, refund 100; each decrypted from its own memo</dd>
</dl>
<p class="caption">An earlier testnet auction driven by the HTTP harness (settlement {tn_api_txid}) also passed every refusal check. Chain data came from one lightwalletd provider (testnet.zec.rocks) over validated TLS. <a href="{repo}evidence/2026-10-19-public-testnet-settlement.json">Full record</a>.</p>

<h2>Other auctions open the bids to prove the price.</h2>
<p class="sub">Commit–reveal and published viewing keys show every bid amount after the close. DarkCall proves the price without opening any, then pays out in shielded ZEC.</p>
<div class="compare-wrap"><table class="compare">
<thead><tr><th scope="col"></th><th scope="col" class="us">DarkCall</th><th scope="col">Open on-chain auction</th><th scope="col">Commit–reveal or published viewing key</th><th scope="col">Trusted auctioneer or key committee</th></tr></thead>
<tbody>
<tr><th scope="row">Bids hidden from the public</th><td class="us">Yes, never opened</td><td class="no">No</td><td>Only until the close</td><td>Yes</td></tr>
<tr><th scope="row">Bids hidden from whoever runs it</th><td class="us">Yes, from any single operator</td><td class="no">No</td><td class="no">No</td><td class="no">No, it decrypts them</td></tr>
<tr><th scope="row">Anyone can check the price</th><td class="us">Yes, by zero-knowledge proof</td><td>Yes</td><td>Yes, by opening every bid amount</td><td class="no">No, you trust it</td></tr>
<tr><th scope="row">Payments and refunds stay private</th><td class="us">Yes, one shielded transaction</td><td class="no">No</td><td>Possible, if paid shielded</td><td>Possible, if it pays shielded</td></tr>
<tr><th scope="row">Payout provably follows the result</th><td class="us">Yes, six output proofs</td><td>Yes, by contract</td><td>By contract, or audited afterwards</td><td class="no">No, the signers decide</td></tr>
</tbody></table></div>
<div class="measured">
<div><strong>0.6 s</strong><span>to seal a bid with its zero-knowledge proof</span></div>
<div><strong>{clearing_s} s</strong><span>to clear four sealed bids by MPC in this run, proof included</span></div>
<div><strong>1 tx</strong><span>pays the seller and refunds all four bidders</span></div>
<div><strong>~100 s</strong><span>for anyone to verify a settlement, no secrets needed</span></div>
</div>
<p class="caption">Common designs compared, not specific projects. Timings from one 16 GiB laptop.</p>

<h2>One real auction, start to finish.</h2>
<p class="sub">Captured in a headless browser driving the real UI, with no shortcuts. Total {total} from the first click to the tamper refusal.</p>
<dl class="facts">
<dt>Auction</dt><dd><code>{auction}</code></dd>
<dt>Accepted root (same book throughout)</dt><dd><code>{root}</code></dd>
<dt>Deposit transaction</dt><dd><code>{deposit}</code></dd>
<dt>Settlement transaction</dt><dd><code>{txid}</code></dd>
<dt>Outcome</dt><dd>{price} zatoshi per unit, {sold} of 3 sold. Each winner gets {winner_refund} back, the loser gets 100, and the seller receives {seller}.</dd>
</dl>
{steps}

<h2>Who learned what.</h2>
<div class="grid4">
<div><b>Everyone</b><p>The clearing price, the units sold, one shielded transaction and its proofs.</p></div>
<div><b>Each bidder</b><p>Only their own result and refund, from their own encrypted memo.</p></div>
<div><b>The operators</b><p>Winners and refunds. Each one holds only shares of the bids.</p></div>
<div><b>Never published</b><p>Who bid what, and every winner's maximum bid. The price is the highest losing bid, or the reserve.</p></div>
</div>

<h2>Verify it yourself.</h2>
<p class="sub">No wallet key, bid, mask or witness is needed. The authority file is configured separately from each bundle.</p>
<pre><code>scripts/docker_verify.sh        # clean Linux container from committed source; about 20 min to build</code></pre>
<pre><code>./verify.sh --funded {bundle_path} \\
  --authority {authority_path}</code></pre>
<div class="table-scroll"><table class="bundles">
<thead><tr><th>Bundle</th><th>Settlement tx</th><th>What it is</th><th>Verified</th></tr></thead>
<tbody>
{bundle_rows}
</tbody></table></div>
<p class="caption">Container: {clean_env}. Each check covers native authorization, six output bindings, the auction proof and the operator certificate, and requires a cryptographic rejection of a corrupted receipt proof.</p>

<h2>Trust model and limits.</h2>
<ul class="limits">
<li>Bid secrecy holds against at most one semi-honest corrupt operator (REP3). Two colluding operators can reconstruct bids. This is not malicious-secure MPC.</li>
<li>Custody is 2-of-3 FROST from a DKG. Two colluding signers can move funds, and there is no unilateral refund.</li>
<li>Operators hold escrow viewing capability and learn winners and refunds.</li>
<li>The public price is the highest losing bid, or the reserve.</li>
<li>In the demo, deposits come from one local test wallet right after the close; each bidder's own wallet would deposit in a deployment. Admission signatures attest funding; they are not a ZK funding proof. The bundle does not prove chain inclusion.</li>
<li>The circuits have four bid slots and six outputs, and native settlement supports the default terms. Settlement takes about 17 minutes on a 16 GiB M1.</li>
</ul>

<footer><a href="{repo}README.md">README</a><a href="{repo}docs/judge-review.md">Review guide</a><a href="{repo}evidence/2026-10-18-fresh-book-native-settlement.json">Run evidence</a><a href="{repo}evidence/2026-10-18-clean-room-linux-verification.json">Clean-room record</a><a href="{repo}docs/reproduce.md">Reproduce</a><span>MIT licensed · IBM Plex under SIL OFL 1.1</span></footer>
</div>
</body>
</html>
'''

if __name__ == '__main__':
    print(json.dumps(build(), indent=2))
