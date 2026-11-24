# Performance

How fast fables builds a book, how much memory it takes and how many bytes it writes, measured
the same way every time so a change can be held to numbers rather than impressions.

## Measuring

```bash
scripts/bench.py                 # every quick case, three runs each, medians
scripts/bench.py --full          # also the 1080p video (minutes)
scripts/bench.py --only proofs,video-preview --json out/bench.json
```

The script builds the release binary, copies one example book's world into a scratch folder
and swaps its voices for a stand-in that writes a tone as long as each line takes to read, so
the video's timing is real and nothing depends on `say`, an API key or the network. Takes are
recorded once before anything is timed. For each case it reports:

| Column | Meaning |
| --- | --- |
| wall s | Median seconds from start to finish: the wait a person sees. |
| cpu s | Median CPU seconds, user and system, of fables and every ffmpeg it runs. |
| peak MB | The largest resident memory of any one process in the runs. |
| out MB | Bytes written to the output folder, recorded takes left out. |

Wall times on a shared machine drift by about 10% from run to run; compare CPU seconds and
bytes when two runs disagree, and run the old and new binaries back to back
(`--fables <path>`) rather than against an old table.

## Results

*Stinky McStinkbreath* (31 pages, 172 seconds of video), Linux x86-64, 4 cores, ffmpeg 6.1.

| Case | Wall s | CPU s | Peak MB | Out MB |
| --- | --- | --- | --- | --- |
| proofs | 1.03 → 1.33 | 3.1 → 4.5 | 277 → 97 | 105.4 → 67.9 |
| storyboard | 2.26 → 0.97 | 2.2 → 3.3 | 174 → 223 | 24.2 → 17.4 |
| ebook | 1.44 → 1.54 | 4.2 → 4.7 | 174 → 194 | 8.67 → 8.18 |
| print | 5.24 → 4.92 | 16.6 → 15.5 | 428 → 436 | 58.4 → 58.4 |
| animatic | 18.5 → 19.7 | 29.0 → 31.0 | 677 → 641 | 43.2 → 43.2 |
| video --preview | 69.6 → 46.7 | 269 → 175 | 506 → 336 | 108.0 → 56.8 |
| video (1080p), one run | 388 → 307 | 1514 → 1183 | 1647 → 775 | 276.9 → 174.0 |

Before is `main` at a6d87f9; after is this branch. The video's output includes the 33 MB
`soundtrack.wav` written beside it; the MP4 itself went from 74 to 23 MB at preview size. The
animatic is unchanged apart from run-to-run noise. Every page and every video frame is the
same, pixel for pixel: the proofs and ePub pages of all 21 books decode identically, and the
raw frames fables hands ffmpeg hash identically, at preview and at 1080p.

## What changed, and why

- **x264 is tuned for animation.** Half of a video's CPU went to the encoder. Finished videos
  now use the `faster` preset with `-tune animation` (flat colour and hard edges keep detail
  with fewer bits); previews and rig tests use `veryfast` at CRF 23 with 96 kb/s audio. On the
  books' own frames the finished file came out 29–68% smaller with 40% less encoder CPU, at a
  PSNR 0.1–0.25 dB and an SSIM 0.001–0.004 below `medium`.
- **PNGs are deflated properly and drop an alpha channel nobody uses.** The PNG encoder ran at
  its fastest setting, which barely compresses a grainy page, and wrote RGBA for opaque pages.
  Level 2 with adaptive filters and RGB makes them over a third smaller. It costs about 40 ms
  of CPU a proof page, which is the proofs' extra second of CPU above.
- **The paper finish runs row by row.** Grain and grade found each pixel's column with a
  division, and the paper mottling hashed four lattice points for every pixel. Hashing once per
  cell makes the crayon and storybook finish 3–4× faster on a print page; a test holds the new
  loops to the old pixel-at-a-time code.
- **JPEGs skip a copy, and ePub ones fit their Huffman tables.** The encoder reads RGBA
  directly. The ePub's pages and covers, which go to every reader (and KDP charges for ePub
  delivery by the megabyte), use fitted tables: about 3% smaller, the same pixels.
- **The storyboard draws its panels on every core.**
- **The contact sheet takes each page as it is drawn**, so the proofs never hold 31 full pages
  in memory at once.

## Tried and left out

- **A global allocator (mimalloc).** 10–15% faster on the short commands but 1.5–2.4× the peak
  memory, no gain on video, and a C dependency.
- **Building for newer x86 CPUs (`x86-64-v3`).** Identical pixels but only about 5% faster, for
  a binary that no longer runs everywhere; Apple silicon already has its vector unit.
- **Skipping repeated frames.** No two consecutive frames of a book are the same: the camera,
  the idle loops and the grain all move.
- **Progressive JPEGs.** Larger, not smaller, from this encoder.
- **jpeg-encoder's AVX2 path.** Its colour conversion rounds differently, so pixels moved by 1.

## Where the time goes now

- **Video:** rasterising frames with resvg/tiny-skia is about two thirds of the CPU and the
  encoder one third. Fills of large paths, pattern fills (sampled bicubically) and gradients
  dominate the rasteriser.
- **Animatic:** ffmpeg's single-threaded filter graph (a `zoompan` and an `overlay` per shot)
  is 85% of the time. Drawing the frames in fables and piping them in, as the video does,
  should take it from about 19 s to a few.
- **Grain in video:** the speckle is mostly gone after H.264 at 1080p, yet it is the bulk of
  what the encoder spends bits on. Holding it still between frames, or leaving it out of video,
  would shrink files much further; that is a look decision, so it is not made here.
- **ePub fonts:** the full Andika faces are 700 KB of an 8 MB ePub. Subsetting them to the
  book's characters needs a subsetter that keeps the `cmap` table readers use.
- **CI:** the macOS test job (about 3½ minutes, a third of it installing ffmpeg) sets the pace.
