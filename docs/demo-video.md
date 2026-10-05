# Recorded demo draft

The latest local video draft is an **edited still-image demonstration**, with
captions and no audio. New native browser screenshots show the seller form,
actual four-ticket acceptance, and fresh same-book REP3 clearing/own result. A cutoff
card labels the real deadline and edited time jump. A separate
card explicitly switches to a **different recorded funded settlement**, followed
by the existing verified winner, tamper and public-verification screenshots.
Funding/confirmation and trust cards close the ten-scene story.

The entry uses synthetic deposits and a localhost client helper that sees typed
demo bids. The new book runs input ZK proofs, signed admission and fresh MPC
clearing; its own local result is verified against that exact signed root. No
native payment is constructed for that book. The create-form still illustrates
the same2-unit/reserve10 terms; accepted/result stills share one actual fresh
book. Each entry scene has a FRESH LOCAL/SYNTHETIC/NO PAYMENT footer;
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

The latest Oct5 local render is110seconds,1920×1080,1,311,895bytes,
no audio. It is saved as `.local/darkcall-same-book-clearing-draft-20261005.mp4`.
Duration, dimensions and stream checks pass; fresh-result and explicit transition
frames were visually inspected. Source/video digests are recorded in
`evidence/2026-10-05-fresh-browser-clearing.json`. Earlier result-only and
entry-only110s renders remain preserved separately and are superseded.

The video labels the isolation/recorded scope throughout. Public replay timings
refer to the recorded112.431s benchmark, separate from the edited screen duration.
Full funded recovery1847.653s and its six sequential row services are disclosed.
One-host/semi-honest REP3,2-of-3 custody, operator outcome visibility and remaining
second-machine/public-testnet work are visible in the closing scenes.

Before publishing or submitting, the human should review the copy and choose
whether to add narration or record continuous interaction. Publication and
contest submission are not performed by this render script.
