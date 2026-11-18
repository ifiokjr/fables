# fables, first time through: *The Smudge on the Moon*

- **Program tested:** `<scratch>/evalbin/bin/fables`
- **Studio:** `<scratch>/evals/base-deepseek`
- **Drawing style:** `rubber-hose`
- **Result:** a 10-page, 306-word picture book, two characters, a clean check, a review with nothing to change, a storyboard, page proofs and a 16:9 preview video.

## The world, the cast and the story

**Lamplight Cove** is a seaside town stacked up a cliff, where every window is washed by hand and the moon is close enough to lean on the chimney pots. Its drawing style is `rubber-hose`. The world file is `worlds/lamplight-cove/world.toml`.

**Nellie Noodle** is the star: a kid window cleaner with a red top, blue shorts, white gloves, and a big black quiff that points at the moon. Her quiff is a redrawn part (`hair.front` and `hair.3q` in `puppets/nellie.svg`), not the style template’s little cap. Her one-line personality: “Polishes every window in Lamplight Cove until it squeaks.” She also has her own `say` voice, Junior, in `cast.toml`.

**Captain Crumpet** is the grown-up: the lighthouse keeper, in a yellow coat and navy trousers, with a grey fringe, a bald crown and a grey moustache. He is drawn by the parametric grown-up body rather than an authored puppet, because the authored-puppet route broke his body (see below). His one-line personality: “Says everything twice, once for the gulls.” He has his own `say` voice, Grandpa (English (UK)).

The cast sheet is `out/sheets/riso/lamplight-cove-cast.png`; Nellie’s model and part sheets are `out/puppets/nellie-model.png` and `out/puppets/nellie-parts.png`, and her fifteen moods are `out/sheets/riso/nellie-moods.png`. The cast sheet is drawn through the default `riso` art skin, even though the puppets themselves are rubber-hose; that is how `fables sheets` works by default.

**The story in three sentences.** Nellie Noodle wants to scrub a smudge off the moon, but her ladder is too short, her shout is too quiet, and a storm washes her plan away. Captain Crumpet’s telescope shows that the smudge is a little starling stuck to the moon, so Nellie gathers moonlight and coaxes the bird down onto her boot. The starling flies home to become the moon’s freckle, and Nellie now cleans the windows of Lamplight Cove by starlight with a friend.

The book is **10 script pages / 24 book pages / 306 words**, for ages four to six. It uses eight setting-and-light combinations: beach/dusk, hilltop/dusk, town/golden, garden/storm, hilltop/moonlit, beach/moonlit, hilltop/dawn and hilltop/golden. It has a close-up (page 6, the starling’s face, `shot=close on=bird.a`), a walk across the page (page 7, `x=0.18` to `x=0.82` with `walk`), and dialogue for both characters (pages 5, 8 and 10).

## The six steps and the files that prove them

| Step | Status | Evidence |
| --- | --- | --- |
| 1. A world with a name and a sentence | Done | `worlds/lamplight-cove/world.toml`; cast sheet `out/sheets/riso/lamplight-cove-cast.png` |
| 2. Two original characters, own colours, one redrawn part, checked | Done | `worlds/lamplight-cove/cast.toml`, `puppets/nellie.svg`, `out/puppets/nellie-model.png`, `out/puppets/nellie-parts.png`, `out/reviews/puppet-check.txt` |
| 3. An 8–10 page, 250–450 word story with the required staging | Done | `worlds/lamplight-cove/stories/smudge-on-the-moon/story.md` (10 pages, 306 words) |
| 4. Check and review until no `fix` findings | Done | `out/reviews/check-final.txt`, `out/reviews/review-0-first.txt` … `review-8.txt`, `review-final.txt` |
| 5. Storyboard, look, change the script because of it | Done | `out/the-smudge-on-the-moon/riso/storyboard/storyboard.png`, `storyboard-2.png` |
| 6. Page proofs and a landscape preview video | Done | `out/the-smudge-on-the-moon/riso/proofs/contact-sheet.png` and the 26 page PNGs; `out/the-smudge-on-the-moon/riso/video/the-smudge-on-the-moon-16x9-preview.mp4` |

### What the storyboard and the proofs changed

1. The first storyboard showed **page 2 as two identical eight-second wides**. I added a `shot=wide` heading and a `@camera move=push over=4` so the second half of the page moves rather than repeats.
2. The first storyboard showed **page 8 as a run of medium shots for about 16 seconds**, two of them the same two-shot, after I had forced `shot=medium on=nellie,crumpet`. I removed the fixed shot and let the automatic director alternate again. Later I added a close on Nellie for “Ooh, it tickles!”, a medium on Crumpet for his line, and a closing full two-shot so the printed page still shows both of them and the bird on her boot.
3. The first storyboard/proof pass exposed that **the “little star” was never in any picture**. The generic critters are bird, dog, worm, flower, tree and toothbrush; there is no star. I recast the smudge as a **starling**, which the tool can draw, and rewrote pages 6–10 around it.
4. Looking at the proofs removed props the tool does not draw: **bucket**, **ladder**, **squeegee**, **hat**, **kite**, **telescope in hand** and **“tallest doorstep”** were all rewritten or replaced. “The little starling plopped onto her boot” replaced “plopped into her bucket”, because the bird was visibly on her boot in the proof.
5. The first storyboard had page 5 “under the lighthouse” while the page was the hilltop setting; I changed the line to “at the top of the hill”, which the picture can support.

## Every command that failed

**Six failed command invocations, from three distinct mistakes.** In order:

### 1–2. `fables puppet template nellie --world <world>` and `fables puppet template crumpet --world <world>`

I had written the cast with `puppet = "puppets/nellie.svg"` and `puppet = "puppets/crumpet.svg"` before the files existed. Both invocations failed with the same cast-loading error (the first one names `crumpet`, not the `nellie` I asked for, because the whole cast is loaded):

```text
error: in <scratch>/evals/base-deepseek/worlds/lamplight-cove/cast.toml: cast member `crumpet`: could not read part file <scratch>/evals/base-deepseek/worlds/lamplight-cove/puppets/crumpet.svg: No such file or directory (os error 2)
```

**What I did next:** removed both `puppet = "..."` lines from `cast.toml`, ran the two `puppet template` commands successfully, then put the lines back.

### 3–4. `fables check <story>` and `fables proofs <story>` with `[cover] light = "moonlit"`

I wanted a moonlit cover and guessed a `light` key. Both commands failed on the same front matter parse:

```text
error: in <scratch>/evals/base-deepseek/worlds/lamplight-cove/stories/smudge-on-the-moon/story.md: front matter is not valid: TOML parse error at line 15, column 1
   |
15 | light = "moonlit"
   | ^^^^^
unknown field `light`, expected one of `setting`, `pose`, `tint`
```

**What I did next:** removed the line; the cover stays in daylight. There is no documented way to light a cover, so a book about the moon has a sunny beach cover.

### 5–6. `fables puppet check --world <world>` and `fables puppet sheet nellie --world <world>` with an empty `ear` path

I was trying to remove the ring on Nellie’s cheek and replaced the `ear` part with an empty path. Both commands rejected the file:

```text
error: in <scratch>/evals/base-deepseek/worlds/lamplight-cove/cast.toml: cast member `nellie`: in <scratch>/evals/base-deepseek/worlds/lamplight-cove/puppets/nellie.svg: unreadable parts:
  `ear`: it has no curves
```

**What I did next:** replaced the empty path with a small real circle; that passed `puppet check` but made no visible difference (the ring is the style’s own ear, and the stock rubber-hose character Clem has the same ring). I restored the backup, so the final puppet has no `ear` override.

**Footnote:** one of my own file-read tool calls used a placeholder path and failed with “File not found”; that was my typo, not a `fables` command, and is not counted above.

## What I had to guess, could not find, or found described wrongly

1. **There is no CLI command that creates a world.** The readme says, under Worlds, “A world needs only a name to start:” and then shows a `world.toml`. But `fables` has no `world` subcommand. The only creation path in the readme is the MCP tool `world_create` (readme, “Designing characters with an agent (MCP)”); `fables new` is “A new book in the world’s `stories/<title>/story.md`, laid out from its `recipe.md`”, and a new world has no recipe. I wrote `world.toml`, `cast.toml`, `puppets/` and `stories/` by hand after reading `worlds/hoppington`, `worlds/teacup-town` and `worlds/kerfuffle`. The layout is only shown in the readme as a tree, not as a setup procedure.
2. **`puppet template` cannot be used before the cast references the file.** Its help says “Start a cast member’s part file to redraw part by part”; it does not say that the cast must not already have `puppet = "..."` pointing at that file. The whole cast is loaded first, so the command fails on any missing puppet. The error names a different cast member than the one I asked for (`crumpet` when I asked for `nellie`), which sent me looking in the wrong place for a moment.
3. **Authored puppets are kids only, and the help does not say so.** The note “Kids only, for now” appears in a comment in the example `worlds/kerfuffle/cast.toml`; it is not in the `puppet template --help` text and not in the readme’s “New characters” section. I made a grown-up from the template, and `puppet check` passed it even though the body was visibly broken: the head floated above the torso, the arms looped above the shoulders, and the “shocked” pose pulled the head away from the body. The broken model sheet is `out/puppets/crumpet-authored-grownup-broken.png`. I switched Crumpet to a parametric grown-up.
4. **A mid-page `@camera` direction also changes the printed still**, not just the video. The readme’s page-heading table says `shot=… on=…` “frames the still illustration”; the stage-direction table lists `@camera` as camera direction for the video; and the script section says “In the book, each page shows the stage at the end of the page.” It does not say a later `@camera` wins for the still. I added `@camera shot=close on=nellie` to page 2 for the video and the page proof became a close-up of her face; on page 8, `@camera shot=medium on=crumpet` turned the page proof into a Crumpet medium with no bird and no boot. I had to add `@camera shot=full on=nellie,crumpet` at the end of page 8 to win the still back, and use `move=push` rather than `shot=close` on page 2.
5. **The cover cannot be lit.** The readme’s `[cover]` example shows `setting`, `tint` and `pose` only. `light = "moonlit"` is rejected with “unknown field `light`, expected one of `setting`, `pose`, `tint`”. There is no `night`, `weather` or equivalent. The cover is therefore a daytime beach for a book about the moon; the title still works, but the cover does not set up the story’s time of day.
6. **A flyer’s `y=` and `land` are not documented enough.** The readme says “`y=sky` (for flyers)” and lists `land` as an action. It does not say that a numeric `y` is a fraction of the stage or that `land` snaps the flyer to the ground regardless of `y`. I placed the starling at `y=0.33` with `land happy` on page 10; it went to the ground behind the text card and disappeared. Replacing `land` with `happy` kept it in the sky and put it on the moon.
7. **There is no way to draw a star as a character.** The generic critters are `bird`, `dog`, `worm`, `flower`, `tree`, `toothbrush`. My first story had a little star with a face as the smudge; no cast kind, generic critter or prop could draw it. I recast the smudge as a starling, a bird the tool can draw. This was the biggest story change and it came from the tool’s vocabulary, not the story.
8. **“Style” means two different things.** `world.toml` has `style = "rubber-hose"` (a drawing style from `fables style list`), while `--style` on most commands is an art skin (`riso`, `tv`, `bold`, …). `fables check` prints `style: riso` for this rubber-hose world, and `fables sheets` writes the cast sheet to `out/sheets/riso/`; the readme explains that “sheets and rigs use riso”, but a first-time user sees the world’s style apparently ignored. A label such as `art skin: riso / drawing style: rubber-hose` would remove the confusion.
9. **Some world and front-matter fields are only in the examples.** I used `about` on the world and on cast members, and `subtitle`, `series`, `number`, `dedication` and `blurb` in the story front matter. None is in the readme’s first `world.toml` or script example; they are in the example worlds and stories and are accepted, so I copied them. They work, but they are not documented.
10. **I could not change the ear.** The `ear` part in a puppet is accepted by `puppet check` but the ring on the cheek in three-quarter and close views did not change when I replaced it. The stock rubber-hose character Clem has the same ring, so it is the style’s ear, not my drawing. It reads as a monocle on Nellie in close-up. I left it.
11. **`fables new` was not tested.** The command needs the world’s `recipe.md`; my new world had none, and the readme does not say how a new world gets a recipe. I wrote `story.md` by hand from the example stories. A `fables new` on a world without a recipe is an untested first-book path.

## Bad results the tool allowed without warning

- **The grown-up authored puppet.** `puppet template crumpet` produced a body with a floating head and looping arms; `puppet check` printed “87 parts from … over the rubber-hose kit” and returned success, and `puppet sheet` rendered the broken figure. The only warning anywhere is the cast comment “Kids only, for now.” Evidence: `out/puppets/crumpet-authored-grownup-broken.png`.
- **A mid-page camera silently reframes the still.** Adding `@camera shot=close on=nellie` for the video changed the book page to a close-up; neither `check` nor `review` warned. The page proof is the only place it shows.
- **`land` can hide a character.** The bird “landed” at ground level behind the text card even though I had given it a sky `y`. `review` noticed the bird was under the card but did not explain that `land` had overridden `y`; I found it by looking at the render.
- **The moon hides under the default text card.** On beach and hilltop pages, the setting’s moon sits in the top of the sky and the default `text=top` card covers it. I only saw this in the proofs; `review` flags actors/critters under the card, not setting parts. `text=bottom` fixed the final pages, but there is no warning that a key story element is invisible.
- **The 16:9 preview crops the square page.** The final beach page’s moon is partly out of frame in the medium shot; the starling appears above Crumpet rather than on the moon in the last beat. The square proof shows it correctly. This is a preview-shape limitation, not a render error, but it is invisible until you watch frames.
- **The style’s cheek ring.** In close-ups and three-quarter heads, the rubber-hose ear is drawn as a ring on the cheek. I first thought it was my puppet; the shipped example Clem has it too. Overriding the `ear` part did nothing, and `puppet check` accepted the override, so a part can be authored without affecting the figure.
- **No star.** A story that wants a star as a character has no drawing to appear in; the pages simply cannot show it. I only noticed this by looking at the first proofs and the storyboard.

## Final `check` output

```text
  style: riso
✓ The Smudge on the Moon — 10 script pages, 24 book pages, 306 words, 2 cast members
```

## Final `review` output

```text
Nothing a director would change.
— The Smudge on the Moon (10 pages, 306 words): 0 fix, 0 consider.
```

## How long each step took, roughly

| Part | Rough tool calls | Notes |
| --- | --- | --- |
| Reading the documentation and every `--help` | 15 | readme, `docs/directing.md`, help for 14 commands, example worlds and styles |
| 1. World and cast files | 4 | folder, `world.toml`, `cast.toml`, two template runs (both failed first) |
| 2. Characters and sheets | 18 | template, redrawn hair, model/part/mood sheets, the broken grown-up, switching Crumpet to parametric |
| 3. Story draft | 6 | write, check, first review, read line numbers, rewrite the star as a starling |
| 4. Check and review loop | 20 | 9 review runs and 8 proof passes, plus the image reads |
| 5. Storyboard and changes | 10 | 3 storyboard renders, 4 storyboard reads, page 2/8 camera edits |
| 6. Proofs and preview video | 20 | 2 video renders (112.3 s and 139.6 s), frame extraction and frame reads |
| Report and evidence tidy-up | 4 | review-0 reconstruction, final check/review, report |

About **90 tool calls** in total and about **35 minutes** of wall time. The only slow commands were the two preview renders (about four minutes together); checks and proofs were under a second, storyboards up to ten seconds.

## Three changes that would have saved me the most trouble

1. **A real CLI world-and-cast bootstrap.** Add something like `fables world create "Lamplight Cove" --style rubber-hose` that writes `world.toml`, `cast.toml`, `puppets/` and a starter `recipe.md`, and let `puppet template` create the cast entry and its file together. The readme says “A world needs only a name to start”, but the CLI has no way to start one; I had to reverse-engineer the folder layout, and the cast-reference chicken-and-egg made my first two commands fail before I had drawn anything. This is the single change that would have saved the most time and confusion.
2. **Make `@camera` video-only, or document and split its still behaviour.** The readme says `shot=` in a heading frames the still, so a writer expects `@camera` to be a video direction; when a mid-page `@camera shot=close` turned page 2 into a close-up and page 8 into a Crumpet medium, I had to re-stage both pages and re-render. Either ignore mid-page `@camera` when composing the still, or add a `book-shot=`/`still-shot=` direction and say plainly in both tables that the last camera otherwise wins.
3. **Either support authored grown-ups or refuse them loudly.** `puppet template` built a broken grown-up body and `puppet check` passed it. Put “kids only” in `puppet template --help` and in the readme’s “New characters” section, not only in a comment inside one example cast; and have `puppet check` reject a `kind = "grown-up"` authored puppet, or make `puppet template` take the body kind and draw correct parts. I lost a whole character design to this and shipped a visible broken model sheet as evidence.

Two runners-up: a `[cover] light` (or `night`/`weather`) field so a moon book can have a night cover, and a `star` (or generic celestial) critter so a story about a star does not have to change its subject. Also worth renaming one of the two “styles” or printing both on one line.

## What I did not do

- I did not build the tool or run `cargo`; I used only the provided binary.
- I did not modify the checkout: `git status --short` in the checkout returns no lines (0 changed paths).
- I did not read the Rust source; I learned from `readme.md`, `docs/directing.md`, every `--help`, and the example worlds/styles.
- I did not use the MCP server; this was a CLI-only test.
- I could not listen to the audio. I verified the MP4 contains H.264 960×540 at 15 fps and an AAC track, and the tool reported “voiced 35 lines (2 character voices), 126s total”; I inspected frames, but I did not audition the narration or the lip-sync.
- `review-0-first.txt` is reconstructed from the first review’s terminal output after the fact; the tool did not write that first version to disk at the time. All later reviews were saved as they ran.
