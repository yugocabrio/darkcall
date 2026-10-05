# Recorded demo draft

The local video draft is an **edited still-image demonstration**, with captions
and no audio. It uses native browser screenshots of actual verified results from
the canonical funded NU7 transaction47019e02…a211938. It is not a real-time screen
recording, a live auction or a claim that proof generation finishes in110 seconds.

The source storyboard is `demo/video-storyboard.json`. The eight scenes show
the public book, a winner's own verified receipt, a losing bidder's own receipt,
actual corrupted-proof rejection, recorded public settlement verification,
controlled funding/confirmation and the explicit trust assumptions. Fixture
refund amounts are public demo material; no key, pad, witness or maximum bid
opening is shown. Selecting a fixture ticket is not production authentication.

To render locally with FFmpeg/ffprobe and a system font:

```sh
python3 -B scripts/render_demo_video.py --output .local/darkcall-demo-draft.mp4
# On Linux, supply an installed font with --font /path/to/font.ttf.
```

The command creates a fresh ignored output and retains its build clips/captions;
it refuses to overwrite. It uses two encoder threads, does no proving, accesses
no wallet and uploads nothing. The screenshot source assets remain small and
tracked; the encoded video stays ignored until the human chooses its release.

The Oct5 local render is110 seconds,1920×1080,1,381,053bytes. It passed duration,
dimensions and no-audio checks; winner/public-verification/trust frames were
visually inspected. Source image and video digests are recorded in
`evidence/2026-10-05-judge-demo-presentation.json`.

The video labels the isolation/recorded scope throughout. Public replay timings
refer to the recorded112.431s benchmark, separate from the edited screen duration.
Full funded recovery1847.653s and its six sequential row services are disclosed.
One-host/semi-honest REP3,2-of-3 custody, operator outcome visibility and remaining
second-machine/public-testnet work are visible in the closing scenes.

Before publishing or submitting, the human should review the copy and choose
whether to add narration or record continuous interaction. Publication and
contest submission are not performed by this render script.
