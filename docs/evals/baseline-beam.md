# Report: Pip and the Perfect Pebble (Marigold)

## World, characters, story

- **World:** Marigold, a small sunny village of gardens, lanes and a market square, drawn in the clear-line style (world.toml: `style = "clear-line"`, `light = "golden"`).
- **Characters:** Pip Pebble (kid, authored puppet with a redrawn spiky hair part; red hair, teal top, sunflower shorts, collects pebbles) and Bramble (kid, parametric; green hair in pigtails, leaf top with dots, night trousers, tree climber). Each has a one-line personality in cast.toml.
- **Story (3 sentences):** Pip wants the perfect pebble to complete his tin and spots one glittering at the top of the old oak tree. Bramble, already up there, claims it, and Pip cannot climb high enough to reach it. Bramble sees how much it matters to him and brings it down, and Pip gives her his second-best pebble in return, so both tins end full.

## Failed commands (10 total), in order

1. `fables puppet template pip --world ... --style clear` (before cast.toml existed)
   ```
   error: could not read cast file .../marigold/cast.toml: No such file or directory (os error 2)
   ```
   Next: created cast.toml.

2. `fables puppet template pip --world ... --style clear` (cast entry referenced `puppets/pip.svg` but file missing)
   ```
   error: in .../cast.toml: cast member `pip`: could not read part file .../puppets/pip.svg: No such file or directory (os error 2)
   ```
   Next: removed the `puppet = ...` line and reran.

3. `fables puppet template pip --world ... --style clear` (after I had copied the template into pip.svg as a placeholder)
   ```
   error: .../puppets/pip.svg already exists; edit it, or move it aside to start again
   ```
   Next: deleted the placeholder and reran.

4. `fables puppet check pip --world ... --style clear` (after redrawing hair as two subpaths)
   ```
   error: in .../cast.toml: cast member `pip`: in .../puppets/pip.svg: unreadable parts:
     `hair.front`: it has more than one outline (draw each part as its own path)
     `hair.3q`: it has more than one outline (draw each part as its own path)
   ```
   Next: merged each hair into a single closed path.

5. `fables check <story> --out <studio>/out`
   ```
   error: unexpected argument '--out' found
     tip: to pass '--out' as a value, use '-- --out'
   ```
   Next: dropped `--out` (check takes no output flag).

6. `fables check <story>` (page heading `# hilltop golden`)
   ```
   error: line 18: `golden` is not a scene option; use night, day, light=, wilt=, stink=, mint=, tint=, speed=, impact=, zoom=, look=x,y, shot=, on=, move=, over=, shake=, cut or ease
   ```
   Next: changed to `light=golden`.

7. `fables check <story>` (direction `@pip x=0.35 look talk`)
   ```
   error: line 19: `look` is not a mood, pose, stance or action.
     moods: happy, grin, laugh, sad, grumpy, cross, defiant, shocked, disgusted, fainted, dizzy, sleepy, dreamy, worried, proud
     poses: idle, wave, point, arms-crossed, hips, cheer, hold-nose, cover-mouth, brush, shrug, droop, hold, think, reach-out, pick
     stances: stand, crouch, sit, kneel, jump, bow, tiptoe
     actions: talk, faint, wake, fly, land, wag, calm, hide, show, hop, flip, walk, run, still
   ```
   Next: removed `look`.

8. `fables check <story>` (page heading `# town dusk`)
   ```
   error: line 62: `dusk` is not a scene option; use night, day, light=, wilt=, stink=, mint=, tint=, speed=, impact=, zoom=, look=x,y, shot=, on=, move=, over=, shake=, cut or ease
   ```
   Next: changed to `light=dusk`.

9. `sed -i 's/.../' <story>` (macOS sed in-place without backup extension)
   ```
   sed: 2: "/tmp/.../story.md": undefined label 'mp/…/...'
   ```
   Next: used python3 to edit the file.

10. `python3 - <<'PY'` with an unexpanded `$STUDIO` path (first attempt)
    ```
    FileNotFoundError: [Errno 2] No such file or directory: '$STUDIO/worlds/marigold/cast.toml'
    ```
    Next: used the absolute path.

## Guesses, undocumented or wrongly described behaviour

- The authored-puppet workflow is circular and undocumented: `puppet template` requires the cast member to exist, but a cast member with `puppet = ...` cannot load until that file exists. The command's own success message tells you to add the `puppet = ...` line afterwards, but nothing says to omit it first. I expected `character_create`-style one-step creation (MCP only).
- Scene options that are bare words vs. `key=value`: only `night` and `day` can be bare; light presets such as `golden` and `dusk` must be `light=golden`/`light=dusk`. The readme's page-heading table lists `night, wilt=1, stink=0.5, ...` and `light=golden` separately, but the example `# bathroom night` made bare lights look natural.
- `fables check` accepts no `--out` flag, unlike the render commands. Not stated per command; discovered from the error.
- `look` is a page-level option (`look=x,y`), not a stage-direction mood. I confused the two namespaces.
- I could not view any rendered image (cast sheet, proofs, storyboard) because this model does not accept image input, so visual verification was impossible; I relied on check/review and file existence.
- The storyboard is an image only; no textual panel list, timings or `· short` markers are printed, so I could not act on its labels programmatically.
- `sheets` defaults to the `riso` skin even for a clear-line world; I had to pass `--style clear` myself. The readme mentions this for sheets/rigs but it is easy to miss.
- Character voices were not used (video reported "0 character voices") because `[video] cast-voices = true` is required; the readme covers it but I did not set it. Not a failure, but a silent default.

## Bad results produced without warning

- `puppet template` let me author a hair part with two subpaths and only `puppet check` rejected it; the template itself did not validate on write.
- The video rendered 119 s of narration with all dialogue spoken by the narrator instead of the characters, and the only notice was the post-render line "voiced 22 lines (0 character voices)". No pre-flight warning.
- Rendering sheets without `--style clear` would have produced riso-styled reference images for a clear-line world with no warning about the mismatch.

## Steps completed

1. World — `worlds/marigold/world.toml`
2. Characters — `worlds/marigold/cast.toml`, `worlds/marigold/puppets/pip.svg`; checked with `fables puppet check pip` (passed); cast sheet at `out/sheets/clear/marigold-cast.png`
3. Story — `worlds/marigold/stories/pip-and-the-perfect-pebble/story.md` (10 pages, 288 words)
4. Check & review — `final-check.txt`, `final-review.txt`, `reviews/review-v1.txt` ... `reviews/review-v4.txt` (final: 0 fix, 0 consider)
5. Storyboard — `out/pip-and-the-perfect-pebble/clear/storyboard/storyboard.png` (script change: added `transition=dissolve` to the dusk walk page)
6. Proofs & preview video — `out/pip-and-the-perfect-pebble/clear/proofs/contact-sheet.png`; `out/pip-and-the-perfect-pebble/clear/video/pip-and-the-perfect-pebble-16x9-preview.mp4`

## Final `check` output (exact)

```
  style: riso
✓ Pip and the Perfect Pebble — 10 script pages, 24 book pages, 288 words, 2 cast members
```

## Final `review` output (exact)

```
Nothing a director would change.
— Pip and the Perfect Pebble (10 pages, 288 words): 0 fix, 0 consider.
```

## Time per step (rough tool calls)

- Step 1 (world): ~2
- Step 2 (characters + sheet): ~18
- Step 3 (story draft): ~3
- Step 4 (check & review iterations): ~12
- Step 5 (storyboard + change): ~4
- Step 6 (proofs + preview video): ~4

## Three changes that would have saved the most trouble

1. **Make authored-puppet creation one step.** Either let `puppet template <id>` create the cast entry (and the file) together, or document the exact two-pass order (cast entry without `puppet`, run template, then add `puppet = ...`). The circular load failure cost several retries.
2. **Document bare vs. `key=value` scene options with examples.** State explicitly that only `night` and `day` are bare words and that every light preset must be written `light=golden`/`light=dusk`, with a worked example per light.
3. **Print a textual storyboard summary.** Alongside `storyboard.png`, emit panel labels, durations and `· short` markers to stdout/a text file, so an agent (or a reviewer who cannot view images) can reason about and adjust shots without guessing.