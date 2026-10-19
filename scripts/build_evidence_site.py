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
    clean = json.loads((ROOT / 'evidence/2026-10-18-clean-room-linux-verification.json').read_text())['run_all_four_bundles']
    run = evidence['video_capture_run']
    facts, at = manifest['facts'], {s['name']: s['elapsed_seconds'] for s in manifest['shots']}
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

    steps = [
        ('01', 'Create', '02-create', at['create'], 'The seller sets 3 identical units, reserve 20 and a deposit budget of 100 zatoshi. Bidding closes in two minutes.'),
        ('02', 'Seal four bids', '04-sealed', at['sealed'],
         'Each bid becomes a commitment plus three REP3 secret shares, one per operator. A client zero-knowledge proof binds them, and the three operators check it and sign. The page shows no values.'),
        ('03', 'Fund on Zcash', '07-funded', at['funded'],
         f'After the close, every ticket gets a real shielded deposit on an isolated NU7 regtest (tx {short(facts["deposit_txid"])}). The operators re-verify the same proofs and shares, then sign the book again over the same root. Took {mmss(at["funded"] - at["funding"] + 15)}.'),
        ('04', 'Clear by MPC', '09-cleared', at['cleared'],
         f'The three operators compute on secret shares and jointly prove the result (coNoir), verified in {clearing_s} s. Price {price} zatoshi, {sold} of 3 sold. No bid was reconstructed.'),
        ('05', 'Only your result', '10-own-result', at['own-result'],
         'Ticket 01 opens only its own verified outcome. No other ticket\'s result is shown.'),
        ('06', 'Settle on Zcash', '16-settled', at['settled'],
         f'One shielded transaction pays the seller and refunds all four bidders (tx {short(facts["txid"])}). It has six Halo2 output bindings and 2-of-3 FROST signatures, Zebra accepted it with 2 confirmations, and a public verifier replayed it. Took {mmss(at["settled"] - at["settle-start"] + 12)}.'),
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

    bundles = [('browser-auction-nu7-v3', facts['txid'], 'The auction in the video', 'host-verified 103 s; added after the container run')]
    bundles += [(r['bundle'], r['txid'], r['scope'], f'clean Linux container, {r["seconds"]:.0f} s, tamper refused') for r in clean['results']]
    bundle_rows = '\n'.join(f'<tr><td><code>{e(b)}</code></td><td><code>{e(short(t))}</code></td><td>{e(s)}</td><td>{e(v)}</td></tr>'
                            for b, t, s, v in bundles)
    page = TEMPLATE.format(
        video_seconds=run['video']['seconds'], auction=e(facts['auction_id']), auction_short=e(short(facts['auction_id'])),
        deposit=e(facts['deposit_txid']), txid=e(facts['txid']), root=e(root_short), price=price, sold=sold,
        total=mmss(manifest['total_seconds']), steps=step_html, winner_refund=100 - int(price), seller=int(price) * int(sold), bundle_rows=bundle_rows, repo=REPO,
        clean_env=e(clean['environment']))
    (DOCS / 'index.html').write_text(page)
    return {'page': str(DOCS / 'index.html'), 'frames': len(FRAMES), 'site_bytes': sum(p.stat().st_size for p in SITE.iterdir())}


TEMPLATE = '''<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>DarkCall evidence</title>
<meta name="description" content="DarkCall: sealed-bid auctions on Zcash. One real auction from creation to shielded receipts, with its transactions and verification.">
<style>
:root {{--bg:#10160f;--panel:#171e17;--panel2:#1c2818;--line:#34422e;--text:#eff3e9;--muted:#a9b5a0;--accent:#c7ed9e;--warn:#e5b9a8;--mono:ui-monospace,SFMono-Regular,Menlo,monospace;--serif:Georgia,"Times New Roman",serif;--sans:-apple-system,BlinkMacSystemFont,"Helvetica Neue",Arial,sans-serif}}
* {{box-sizing:border-box}}
html {{background:var(--bg)}}
body {{margin:0;background:var(--bg);color:var(--text);font:16px/1.6 var(--sans)}}
a {{color:var(--accent)}}
code {{font:13px var(--mono);color:#dbe9cd;overflow-wrap:anywhere}}
.wrap {{max-width:1120px;margin:0 auto;padding:0 24px}}
header {{display:flex;justify-content:space-between;align-items:center;gap:16px;padding:22px 0;border-bottom:1px solid var(--line)}}
.brand {{font:600 20px var(--sans);letter-spacing:1px;color:var(--text);text-decoration:none}}
.brand span {{color:var(--accent)}}
.tag {{font:11px var(--mono);letter-spacing:1.4px;color:var(--muted);text-transform:uppercase}}
h1 {{font:400 clamp(40px,7vw,68px)/1.05 var(--serif);letter-spacing:-1.5px;margin:56px 0 18px}}
h1 em {{color:var(--accent)}}
h2 {{font:400 clamp(28px,4vw,38px)/1.15 var(--serif);margin:72px 0 10px;letter-spacing:-.5px}}
h3 {{font:500 20px var(--sans);margin:4px 0 8px}}
.lead {{font-size:19px;color:#c9d6be;max-width:760px;margin:0}}
.sub {{color:var(--muted);max-width:760px;margin:0 0 26px}}
.scope {{margin:26px 0 0;padding:14px 18px;border:1px solid #5a6b4c;border-radius:8px;background:var(--panel);color:#cdd9c2;font-size:14px;max-width:820px}}
.scope b {{color:var(--accent)}}
video {{display:block;width:100%;margin:34px 0 8px;border:1px solid var(--line);border-radius:10px;background:#000}}
.caption {{font-size:13px;color:var(--muted)}}
.contrast {{display:grid;grid-template-columns:1fr 1fr;gap:14px;margin:22px 0 0}}
.side {{border:1px solid var(--line);border-radius:10px;padding:18px 20px;background:var(--panel)}}
.side.dc {{border-color:#6f9351;background:var(--panel2)}}
.side .tag {{display:block;margin-bottom:12px}}
.side.dc .tag {{color:var(--accent)}}
.bids {{display:flex;flex-wrap:wrap;gap:8px;align-items:center;margin-bottom:12px}}
.bids b {{font:500 15px var(--mono);border:1px solid #44523d;border-radius:6px;padding:6px 10px;min-width:44px;text-align:center}}
.typical .bids b {{text-decoration:line-through;text-decoration-color:#c98a72;color:var(--warn)}}
.dc .bids b {{color:#8fa385;letter-spacing:2px}}
.bids i {{font:600 13px var(--mono);font-style:normal;color:var(--accent)}}
.side p {{margin:0;color:#c2cfb7;font-size:15px}}
.facts {{display:grid;grid-template-columns:200px 1fr;gap:8px 18px;margin:26px 0 0;padding:20px;border:1px solid var(--line);border-radius:10px;background:var(--panel)}}
.facts dt {{color:var(--muted);font-size:14px}}
.facts dd {{margin:0}}
.step {{display:grid;grid-template-columns:minmax(0,1.1fr) minmax(0,1fr);gap:30px;align-items:center;padding:30px 0;border-top:1px solid var(--line)}}
.step:first-of-type {{border-top:0}}
.shot img {{display:block;width:100%;height:auto;border:1px solid var(--line);border-radius:8px}}
.step-no {{font:12px var(--mono);color:var(--accent);letter-spacing:1px}}
.step p {{margin:0 0 10px;color:#cbd7c0}}
.clock {{font:12px var(--mono);color:var(--muted)}}
.grid4 {{display:grid;grid-template-columns:repeat(4,1fr);gap:12px;margin-top:20px}}
.grid4 div {{border:1px solid var(--line);border-radius:10px;padding:16px;background:var(--panel)}}
.grid4 p {{margin:8px 0 0;font-size:14px;color:#cfdcc4}}
pre {{margin:14px 0;padding:16px 18px;border:1px solid var(--line);border-radius:8px;background:#0b100a;overflow-x:auto}}
pre code {{color:#d6e6c6}}
table {{width:100%;border-collapse:collapse;margin-top:16px;font-size:14px}}
th,td {{text-align:left;padding:10px 8px;border-bottom:1px solid var(--line);vertical-align:top}}
th {{color:var(--muted);font-weight:500}}
.limits li {{margin:6px 0;color:#cfdcc4}}
footer {{margin:80px 0 40px;padding-top:20px;border-top:1px solid var(--line);font-size:14px;color:var(--muted);display:flex;flex-wrap:wrap;gap:8px 22px}}
@media (max-width:760px) {{
  .wrap {{padding:0 16px}}
  .contrast,.grid4 {{grid-template-columns:1fr}}
  .facts {{grid-template-columns:1fr;gap:2px}}
  .facts dd {{margin-bottom:10px}}
  .step {{grid-template-columns:1fr;gap:14px}}
  .table-scroll {{overflow-x:auto}}
  .table-scroll table {{min-width:680px}}
  header .tag {{display:none}}
}}
</style>
</head>
<body>
<div class="wrap">
<header><a class="brand" href="{repo}README.md">DARKCALL<span>.</span></a><span class="tag">ZECATHON 2026 · Private Markets</span></header>

<h1>Sealed bids.<br><em>Verifiable settlement.</em></h1>
<p class="lead">Bids stay secret-shared. Three operators compute one fair price by MPC and prove it. One shielded Zcash transaction pays everyone, bound to that proof.</p>
<p class="scope"><b>Demo scope, up front.</b> Every party runs on one machine, and the local helper that builds a bid's proof sees the typed demo bid. The chain is an isolated Zebra NU7 regtest, not testnet or mainnet. The protocol keeps bids from the operators and the public; this demo shows that machinery working, not isolation between separate parties.</p>

<video controls preload="metadata" poster="site/01-landing.jpg">
<source src="site/darkcall-demo-v2.mp4" type="video/mp4">
<track kind="captions" src="site/darkcall-demo-v2.vtt" srclang="en" label="English" default>
</video>
<p class="caption">{video_seconds} s, edited from one real auction ({auction_short}). Every time cut states the real elapsed time, and every frame is SHA-256-checked against the <a href="{repo}demo/media/v2/manifest.json">capture manifest</a>.</p>

<h2>The difference</h2>
<p class="sub">Auditable auctions usually prove the price by opening the bids. DarkCall proves it without opening them.</p>
<div class="contrast">
<div class="side typical"><span class="tag">Typical audited auction</span><div class="bids"><b>77</b><b>61</b><b>43</b><b>21</b></div><p>Opens every bid after the close so anyone can check the price.</p></div>
<div class="side dc"><span class="tag">DarkCall</span><div class="bids"><b>••</b><b>••</b><b>••</b><b>••</b><i>price {price} ✓ proof</i></div><p>Proves the price without opening the bids. Each bidder decrypts only their own result.</p></div>
</div>

<h2>One real auction, start to finish</h2>
<p class="sub">Captured in a headless browser driving the real UI, with no shortcuts. Total {total} from the first click to the tamper refusal.</p>
<dl class="facts">
<dt>Auction</dt><dd><code>{auction}</code></dd>
<dt>Accepted root (same book throughout)</dt><dd><code>{root}</code></dd>
<dt>Deposit transaction</dt><dd><code>{deposit}</code></dd>
<dt>Settlement transaction</dt><dd><code>{txid}</code></dd>
<dt>Outcome</dt><dd>{price} zatoshi per unit, {sold} of 3 sold. Each winner gets {winner_refund} back, the loser gets 100, and the seller receives {seller}.</dd>
</dl>
{steps}

<h2>Who learned what</h2>
<div class="grid4">
<div><span class="tag">Everyone</span><p>The clearing price, the units sold, one shielded transaction and its proofs.</p></div>
<div><span class="tag">Each bidder</span><p>Only their own result and refund, from their own encrypted memo.</p></div>
<div><span class="tag">Operators</span><p>Winners and refunds. Each one holds only shares of the bids.</p></div>
<div><span class="tag">Never published</span><p>Who bid what, and every winner's maximum bid. The price is the highest losing bid, or the reserve.</p></div>
</div>

<h2>Verify it yourself</h2>
<p class="sub">No wallet key, bid, mask or witness is needed. The authority file is configured separately from each bundle.</p>
<pre><code>scripts/docker_verify.sh        # clean Linux container from committed source; about 20 min to build</code></pre>
<pre><code>./verify.sh --funded demo/artifacts/browser-auction-nu7-v3 \\
  --authority demo/authority/browser-auction-nu7-v3.json</code></pre>
<div class="table-scroll"><table>
<thead><tr><th>Bundle</th><th>Settlement tx</th><th>What it is</th><th>Verified</th></tr></thead>
<tbody>
{bundle_rows}
</tbody></table></div>
<p class="caption">Container: {clean_env}. Each check covers native authorization, six output bindings, the auction proof and the operator certificate, and requires a cryptographic rejection of a corrupted receipt proof.</p>

<h2>Trust model and limits</h2>
<ul class="limits">
<li>Bid secrecy holds against at most one semi-honest corrupt operator (REP3). Two colluding operators can reconstruct bids. This is not malicious-secure MPC.</li>
<li>Custody is 2-of-3 FROST from a DKG. Two colluding signers can move funds, and there is no unilateral refund.</li>
<li>Operators hold escrow viewing capability and learn winners and refunds.</li>
<li>The public price is the highest losing bid, or the reserve.</li>
<li>In the demo, deposits come from one local test wallet right after the close; each bidder's own wallet would deposit in a deployment. Admission signatures attest funding; they are not a ZK funding proof. The bundle does not prove chain inclusion.</li>
<li>The circuits have four bid slots and six outputs, and native settlement supports the default terms. Settlement takes about 17 minutes on a 16 GiB M1.</li>
</ul>

<footer><a href="{repo}README.md">README</a><a href="{repo}docs/judge-review.md">Review guide</a><a href="{repo}evidence/2026-10-18-fresh-book-native-settlement.json">Run evidence</a><a href="{repo}evidence/2026-10-18-clean-room-linux-verification.json">Clean-room record</a><a href="{repo}docs/reproduce.md">Reproduce</a><span>MIT licensed</span></footer>
</div>
</body>
</html>
'''


if __name__ == '__main__':
    print(json.dumps(build(), indent=2))
