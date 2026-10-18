# Recorded demo draft (Oct 5, superseded)

> **Superseded on Oct 18.** This draft predates **Settle on Zcash**, so it
> switches to a different recorded auction for payment. The current flow
> settles the same browser auction. Use the capture plan in
> [demo.md](demo.md#capture-ready-demo-plan-about-2-minutes-edited) for a new
> recording. The text below describes the old draft only.

The latest local video draft is an **edited still-image demonstration**, with
captions and no audio. Seven new browser screenshots show the two experience
cards, seller form, actual acceptance, fresh same-book clearing/own result and
recorded winner/tamper/public verification with the polished UI. A cutoff
card labels the real deadline and edited time jump. A separate
card explicitly switches to a **different recorded funded settlement**, followed
by newly captured verified winner, tamper and public-verification screenshots.
Funding/confirmation and trust cards close the eleven-scene story.

The entry uses synthetic deposits and a localhost client helper that sees typed
demo bids. The new book runs input ZK proofs, signed admission and fresh MPC
clearing; its own local result is verified against that exact signed root. No
native payment is constructed for that book. The create-form still illustrates
the same2-unit/reserve10 terms; create/accepted/result stills share actual fresh
book bc615713…56f3bd, root086dc2a7…512cff. Actual clearing8.284s, price43/sold2,
own entitlement57. Each entry scene has a FRESH LOCAL/SYNTHETIC/NO PAYMENT footer;
recorded scenes name transaction47019e02…a211938. No new entry ticket is mapped
to a recorded winner. This is not a real-time screen recording, a live funded
auction or a claim that native proof generation finishes in110seconds.

The source is `demo/video-storyboard.json`. Fixture refunds and public signed
receipts are demo material; no key, pad, witness or maximum-bid opening is shown.
Selecting a recorded ticket is not production authentication.

To render locally with FFmpeg/ffprobe and a system font:

```sh
python3 -B scripts/render_demo_video.py --output .local/darkcall-demo-draft.mp4
# On Linux, supply an installed font with --font /path/to/font.ttf.
```

The command creates a fresh ignored output and retains its build clips/captions;
it refuses to overwrite. It uses two encoder threads, does no proving, accesses
no wallet and uploads nothing. The screenshot source assets remain small and
tracked; the encoded video stays ignored until the human chooses its release.

The latest Oct5 local render is110seconds,1920×1080, no audio, saved as
`.local/darkcall-two-mode-demo-draft-v2-20261005.mp4`. Duration, dimensions and
stream checks pass; fresh-result and explicit transition frames are visually
inspected. Source/video digests are recorded in
`evidence/2026-10-05-two-mode-demo-polish.json`. Earlier result-only, entry and
same-book drafts remain preserved separately and are superseded.

The video labels the isolation/recorded scope throughout. Public replay timings
refer to the recorded112.431s baseline benchmark, separate from the edited screen
duration. The polished UI's new full replay passed in110.622s.
Full funded recovery1847.653s and its six sequential row services are disclosed.
One-host/semi-honest REP3,2-of-3 custody, operator outcome visibility and remaining
second-machine/public-testnet work are visible in the closing scenes.

Before publishing or submitting, the human should review the copy and choose
whether to add narration or record continuous interaction. Publication and
contest submission are not performed by this render script.
