# Recorded demo draft

The latest local video draft is an **edited still-image demonstration**, with
captions and no audio. Three new native browser screenshots show the working
seller form, actual four-ticket acceptance and cutoff certification. A separate
card explicitly switches to a **different recorded funded settlement**, followed
by the existing verified winner, tamper and public-verification screenshots.
Funding/confirmation and trust cards close the nine-scene story.

The entry uses synthetic deposits and a localhost client helper that sees typed
demo bids. It runs input ZK proofs and signed admission, not a new MPC clearing
or native settlement. Each entry scene has its own OFFLINE/SYNTHETIC footer;
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

The latest Oct5 local render is110seconds,1920×1080,1,339,783bytes,
no audio. It is saved as `.local/darkcall-product-loop-draft-20261005.mp4`.
Duration, dimensions and stream checks pass; entry, explicit transition and trust
frames were visually inspected. Source/video digests are recorded in
`evidence/2026-10-05-seller-bidder-rehearsal.json`. The earlier result-only110s
render remains preserved separately and is superseded for the product story.

The video labels the isolation/recorded scope throughout. Public replay timings
refer to the recorded112.431s benchmark, separate from the edited screen duration.
Full funded recovery1847.653s and its six sequential row services are disclosed.
One-host/semi-honest REP3,2-of-3 custody, operator outcome visibility and remaining
second-machine/public-testnet work are visible in the closing scenes.

Before publishing or submitting, the human should review the copy and choose
whether to add narration or record continuous interaction. Publication and
contest submission are not performed by this render script.
