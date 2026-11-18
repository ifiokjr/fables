# REPORT — fables first-time-user test (style: chibi)

## The world, the characters, the story

**World:** Moonmallow — a marshmallow-soft village on a hill, where the street lamps are
fireflies in jars and nobody ever locks a door. Drawing style: chibi.
(`worlds/moonmallow/world.toml`)

**Characters** (`worlds/moonmallow/cast.toml`):
- **Pip Thistledown** — the smallest lamplighter in Moonmallow, brave at exactly
  sunset-sized problems. Own colours (orange spiky hair, blue striped top, navy shorts) and
  a fully redrawn hair part (tall messy spikes, front + three-quarter views) in
  `worlds/moonmallow/puppets/pip.svg`, started from the chibi template.
- **Plum Ferrow** — the jam-seller at the end of the lane, grumbly on the outside, gooey in
  the middle. Own colours, purple bun hair and glasses (cast-file options only).

**Story (3 sentences):** Pip the lamplighter cannot wake the fireflies in her jar, and
sunset — and the big hilltop lantern — is coming. Nothing works until Plum the jam-seller
hands over a sticky old jar, because sweet things wake for sweetness. The lantern glows
brighter than ever, and Pip decides that "together" is even better than "by myself".

## Failed commands, in order (4 total)

1. `fables puppet template pip --world …/moonmallow`
   → `error: in …/cast.toml: cast member 'pip': could not read part file …/puppets/pip.svg: No such file or directory (os error 2)`
   The cast entry must not name a puppet file until the file exists, but `puppet template`
   (the thing that creates the file) requires the cast entry. Catch-22. Fix: temporarily
   commented out `puppet = "puppets/pip.svg"`, ran `template`, restored the line. The
   template command's own output then told me to add the line back.
2. `fables check …/story.md`
   → `error: line 19: 'golden' is not a scene option; use night, day, light=, wilt=, …`
   I had written `# street golden`. Fix: `light=golden` (the readme's heading table lists
   `light=golden` but my prose reading missed it — error message was excellent).
3. `fables check …/story.md`
   → `error: line 65: holds 'jar' should be toothbrush, parcel, jam or nothing`
   Fix: `holds=jam`.
4. (Not a failure but a fix) review v1 reported 2 `fix` findings (missing `talk` on a quoted
   line; a 47-word page), which I fixed before rendering.

## What I had to guess, or found missing/wrong

- **The puppet-file catch-22** (failure 1 above). Nothing in readme.md ("A new character is
  a new table in cast.toml") or `puppet template --help` says the cast table must omit
  `puppet =` until after templating, or that `template` reads the cast at all. One sentence
  in the readme would save every newcomer this stumble.
- **Redrawing parts is documentation-by-example.** The readme says `puppet template`
  starts a file "to redraw part by part", but no doc lists which part ids a chibi figure
  needs, their coordinate space, or which ids are turning pairs (`.front`/`.3q`). I copied
  path shapes from `worlds/teacup-town/puppets/mochi.svg` and edited coordinates blind;
  `puppet sheet` was my only feedback loop. The MCP tool docs promise `character_parts` /
  `style_describe` "coordinate space and drawing rules" — the CLI has no equivalent read
  command, so CLI-only users get nothing.
- **`sheets` renders into a `riso/` art-style folder** even though the world's drawing style
  is chibi. The readme does say "sheets and rigs use riso", but the art-style/drawing-style
  split is confusing on first contact — my cast sheet's filename said `riso` while the
  characters were unmistakably chibi.
- **Whether bare `night` vs `light=night` differ.** The readme paragraph on scene lights is
  dense ("A bare `night` keeps today's behaviour exactly unless a `light=` is also given").
  I guessed and used `light=` everywhere; I still don't know what I gave up.
- **`check` prints "style: riso"** for a chibi-world book with no front-matter style. Minor,
  but it made me wonder whether my world's style had been picked up at all until I saw the
  chibi drawings.

## Bad results without warning

- A stray closing `+++` after the last page was silently rendered as literal "+++" text on
  a story page. `check` passed it; I only caught it on the contact sheet. A check finding
  ("stray front-matter marker on line N") would be cheap and valuable.
- Nothing else. Errors were consistently line-numbered and listed valid options, which is
  genuinely good.

## Steps completed, with proof files

All six steps completed. Studio root:
`<scratch>/evals/base-glm-flash/` (paths below relative to it)

1. **World** — `worlds/moonmallow/world.toml`
2. **Characters** — `worlds/moonmallow/cast.toml`, redrawn puppet
   `worlds/moonmallow/puppets/pip.svg`, cast sheet `out/sheets/riso/moonmallow-cast.png`
   (plus `moonmallow-places.png`, `moonmallow-lights.png`)
3. **Story** — `worlds/moonmallow/stories/pip-and-the-sleepy-lights/story.md`
   (8 pages, 259 words; settings town/golden/dusk/overcast/night/moonlit; close-up on
   page 5 via `shot=close on=pip`; walks across pages 2 and 7; dialogue for both)
4. **Check & review** — all four review outputs kept: `out/review-v1.txt` (2 fix, 2
   consider), `out/review-v2.txt` (0 fix), `out/review-v3.txt` and `out/review-final.txt`
   (0 fix, 1 consider)
5. **Storyboard + change** — `out/pip-and-the-sleepy-lights/riso/storyboard/storyboard.png`
   and `storyboard-2.png`. Changes made because of it: lengthened the 1.2 s "Wakey-wakey!"
   shot and changed the ending mood from `talk` to `proud` (the storyboard showed Pip
   still downcast in the final panels, matching review's "arc ends where it began").
6. **Render** — proofs + contact sheet:
   `out/pip-and-the-sleepy-lights/riso/proofs/` (`contact-sheet.png`, 24 pages); landscape
   preview video:
   `out/pip-and-the-sleepy-lights/riso/video/pip-and-the-sleepy-lights-16x9-preview.mp4`
   (119 s, 960×540, 15 fps) with `soundtrack.wav` and `.srt`.

## Final check and review output (copied exactly)

```
  style: riso
✓ Pip and the Sleepy Lights — 8 script pages, 24 book pages, 259 words, 2 cast members
```

```
consider line 73: The page with the strongest mood change stays at a distance. Frame it: `shot=close on=…` in the heading, or an `@camera` direction mid-page.
— Pip and the Sleepy Lights (8 pages, 259 words): 0 fix, 1 consider.
```

## Rough time per step, in tool calls

| Step | Calls |
| --- | --- |
| Read docs/help, example worlds | ~10 |
| World + characters (incl. catch-22 retry, hair redraw, sheets) | ~12 |
| Story + check/review loop to 0 fix | ~8 |
| Storyboard + script changes + re-review | ~6 |
| Proofs + preview video (+ the `+++` fix re-render) | ~6 |
| Report | 1 |

## Three changes that would have saved me the most trouble

1. **Fix the `puppet template` / cast.toml ordering trap** — either let `template` create
   the cast table too, or let it run without a cast entry and document the exact
   two-step recipe (`template` first, then add `puppet =` to the cast).
2. **Document the part-id vocabulary and coordinate space for CLI users** — a table of the
   parts a style's figure is built from (ids, views, units) in `docs/`, or a `puppet parts
   <id>` read command mirroring the MCP `character_parts`. Redrawing blind is the scariest
   part of the whole flow.
3. **Make `check` catch stray `+++` and other silently-swallowed markup** — the one place
   the tool produced a visibly broken page while reporting success.
