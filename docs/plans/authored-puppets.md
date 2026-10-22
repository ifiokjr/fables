# Plan: authored puppets for the whole cast

The art experiment in `experiments/cartoon/` redrew Stinky as hand-drawn cut-out parts on the existing rig, and Ifiok preferred it to the current puppet. This plan moves that approach into `fables` itself and rolls it out to the other ten cast members, the settings and the video.

Written 21 October 2026. No code has changed yet; each phase below waits for Ifiok's go-ahead.

## Where things stand

| | Current `fables` | Experiment |
| --- | --- | --- |
| Characters | One parametric template (`src/art/figure.rs`, 2,400 lines) drawn from `Look` fields in `cast.toml` | Hand-placed Bézier parts per character |
| Angles | Front only; `facing` slides the face sideways | Front and three-quarter drawings, blended by `turn` |
| Face | Concentric-circle eyes, a toothy rectangle grin | Lids, lower lids and brows that act, and nine lip-sync mouths |
| Rig | `art::rig` (stances, gaits, two-bone IK) | The same rig, unchanged |
| Moods and poses | 15 moods, 15 poses, props, teeth states | 4 moods, 5 arm poses, no props |
| Finish | Six interchangeable style skins | One finish: a thick-and-thin line and a multiplied shadow tone |
| Settings | Seven flat backdrops | One park with real depth |

The rig, lip-sync, staging, timeline, voices and video pipeline all carry over as they are. The work is in drawing, loading the drawings, and mapping the existing state onto them.

## Decisions for Ifiok

These change the shape of the work, so they come first.

1. **Who draws the parts?** The experiment's parts are path strings in Rust. That was fine for an experiment, but nobody can open them in a drawing tool.
   - *Recommended:* one SVG file per character under `world/puppets/`, editable in Figma, Inkscape or Affinity, with fables loading it.
   - *Alternative:* keep the drawings as path strings in Rust. That avoids any loader work, but only code can edit them.
2. **Keep the six style skins?** The new look is its own finish, and the skins only restyled the old template.
   - *Recommended:* make the new finish the only style and retire `tv`, `bold`, `cut-paper`, `crayon` and `storybook`. Keep the paper grain as an option.
   - *Alternative:* keep two (the new finish plus the riso grain over it).
3. **Is a profile view needed?** Three-quarter walks read well in the test video. A true side view would need a third drawing of every turnable part.
   - *Recommended:* not for book one.
4. **Who signs off each design?** Recommended: Ifiok approves each character's model sheet before it is wired in.

## The design

### A shared puppet with per-character drawings

The new code goes in a new module, `src/art/puppet.rs`. It splits each character into two halves.

**Shared kit, drawn once and reused by everyone:**
- eyes, upper and lower lids, brows and their mood presets;
- the nine-shape mouth chart, plus the mood mouths;
- hands (relaxed, open, fist, point), socks and shoes;
- the face and body cylinder warps, front-to-three-quarter blending, the far-side tint, and thick-and-thin inking.

Characters can override any shared part. Shouty gets a bigger mouth, and Grubby gets his own muddy hands.

**Each character's own drawings:**
- the head outline, hair, ears, and any face extras (glasses, moustache, bogey);
- the torso, clothes and hats;
- a palette;
- limb lengths (a `rig::Limbs`) and a few measurements: head origin, neck, shoulder line, hip spread, mouth position.

`Shape`, the thick-and-thin line and the multiplied shadow move from the experiment into `src/art/` beside `Path` and `Svg`, so there is one drawing surface rather than two.

### Part files

Each character gets one file, `world/puppets/<id>.svg`. Part names live in the element ids:

```text
head.front   head.3q        hair.front   hair.3q    tuft.front  tuft.3q
torso.front  torso.3q       shorts.front shorts.3q  sleeve      shorts.leg
ear          extra.glasses  ...
```

- **Loading reads the SVG with `roxmltree` and `svgtypes`.** Both are already compiled into fables, because `usvg` depends on them. They resolve relative commands, arcs, `S`, `Q`, `L` and the path's own transform; lines and quadratics become cubics, so `Shape` only ever holds cubics. A part is exactly its own path data plus its own `transform`, and the groups around it only lay out the sheet. (`usvg` itself was the first idea, but it bakes group transforms into every path, so tidying the sheet would move the drawings.)
- **Two drawings of a part blend only when they have the same number of curves.** At load time, `fables puppet check <id>` names any mismatch, such as "`hair.3q` has 18 curves, `hair.front` has 17". A mismatched pair is not an error: it swaps drawings at the half-way point instead of blending, the way hands and shoes already work. The authoring rule is to draw the three-quarter view by duplicating the front and moving its nodes.
- **`fables puppet template <id>` writes a starter file.** It contains the skeleton, pivots, the head origin and the ground line on a locked guide layer, so parts are drawn in the right coordinate frame.
- **`fables puppet sheet <id>` renders the parts sheet:** every part, with each turning part shown front-on, half-turned and three-quarters, plus the check's findings. The posed review sheets (model sheet, turnaround, mouth chart and rig check) join it in PR 4, once the puppet is drawn by the library.

In `cast.toml`, a member gains `puppet = "puppets/stinky.svg"`. Members without one keep drawing with the old `figure.rs` until they are migrated, so every PR in the rollout ships a working book.

### Mapping `FigureState` onto the new puppet

`compose::draw_puppet` picks the new puppet when the member has one. The state maps as follows.

| `FigureState` | New puppet |
| --- | --- |
| `facing` (-1..1, staged at ±0.35 by default) | `turn = (facing / 0.35).clamp(-1, 1)`: the default is a full three-quarter view towards the middle, and `facing=0` is front-on. |
| `mood` (15) | Face presets for brows, lids, mouth, mouth curl and extras. Four are done. The other eleven: **laugh** (eyes shut in arcs, wide mouth); **sad** (inner brows up, lids half down, a small down-curve); **grumpy** (flat lids, a pout); **cross** (brows down hard, teeth clenched); **shocked** (lids wide, tiny pupils, round mouth); **fainted** (crossed-out eyes, tongue out); **dizzy** (spiral pupils, a wobbly mouth); **sleepy** (lids at 0.8, a small round mouth); **dreamy** (lids half closed, gaze up, a soft smile); **worried** (brows up in the middle, teeth showing); **proud** (eyes shut and smiling, chin up). |
| `hands`, `hand_shapes` (15 poses) | Done in layer 4. The poses keep their anchor-based reaches. An authored frame measures reaches in separate across and up units, (50, 80), because its arms are shorter and its body narrower than the parametric puppet's. Two staging rules keep hands readable: a raised hand that would overlap the head moves out past it, and in three-quarter view a far hand waving across the face is handed to the near hand. An open hand hanging at the side is drawn relaxed. The kit has a **point** hand. |
| `mouth_open`, `viseme` | Visemes are done. `mouth_open` without a viseme (shouting, mouth-open stills) picks `Open` or `Wide` and scales them by the amount. |
| `gaze`, `blink`, `brow_lift`, `head_tilt`, `tilt` | Done, or a direct carry-over. `brow_lift` adds to both brows' raise. |
| `legs`, `gait`, `lie`, `squash` | `legs` and `gait` are done. `lie` and `squash` are whole-puppet transforms, as now. |
| `sway` | Done in layer 5. Loose hair (the tuft, a bow) swings about its root with `sway`, bounces with a walk and drifts slightly at rest. |
| `teeth` | Teeth only show where the shape draws them. **Grotty** recolours the band (`GROT`) and adds a gap and a green edge. **Sparkly** adds a glint and the `fx::sparkle`. |
| `prop` | Done in layer 5. The jam jar, toothbrush and parcel are vector parts in the kit, held in the fist under the fingers. A toothbrush near the mouth turns into it and scrubs, with foam. A far hand holding something stays in front in three-quarter view. |
| extras (glasses, moustache, apron, post-cap, satchel, bow, muddy hands, bogey, lashes) | Done in layer 5. The cast's `extras` list stays as the switch, and each extra is a vector part in the kit that a character's own file can redraw. Face extras (glasses, lashes, moustache, bogey) wrap with the face; the bow and cap ride on the head, the satchel and apron on the body; muddy hands tint the hands and forearms. |

`Frame::of(build)` is called 17 times: 11 of them in `compose`, `video::frames`, `video::motion` and `press::sheets`, mostly for the head and mouth positions. It becomes `look.frame()`, which reads the measurements from the puppet file. This is mechanical, and it lands early, as its own PR.

### Grown-ups

Mum, Dad and Parcel use the same kit with their own `Limbs`: longer legs and arms, a higher pelvis, and a smaller head relative to the body. Parcel's post cap and satchel, and Mum's bun and glasses, are parts in their files. Kids and grown-ups should stand in a model-sheet lineup together, so their relative sizes are checked early.

### Biscuit, Wormsley and Brushy

- **Biscuit (dog):** needs a four-legged rig. That is `rig::quadruped`, with four two-bone legs and a diagonal-pair gait phase. His body parts are drawn in profile and three-quarter views, and a long body that sags when he faints is a part blend.
- **Wormsley:** a body drawn along a spline (as now, restyled), plus a bowler-hat part and the shared eyes at a small scale.
- **Brushy (the toothbrush):** a rigid body with the shared face kit. Brows and mouth carry all of its acting.
- **Birds, flowers and trees (generic critters):** restyle them to the new line, shadow and palette rules. They don't need the full puppet treatment.

### Settings

The experiment's park sets the rules: colour fades into a sky-coloured haze with depth; lines thin out and disappear with distance; things overlap; the ground has a vanishing point; and the nearest things frame the page. Each of the seven settings (`plain`, `town`, `street`, `garden`, `park`, `bathroom`, `bedroom`) is repainted to these rules. The existing `Layer` split for video parallax and the `Weather` reactions (wilting, night) stay.

Interiors use different depth cues:
- floorboards and rugs in perspective;
- a wall-to-floor line;
- window light falling across the floor;
- furniture overlapping at the edges of the page.

### Styles

If the recommendation in decision 2 is taken, the new finish lives directly in `Svg`. `StyleName` and the `--style` option are kept for one release as an alias, and then removed along with `figure.rs`.

## Order of work

One `gh stack`, so each layer is reviewable on its own. Every PR that changes how something looks attaches before-and-after sheets with `gh pr create --attach`.

| # | PR | What it does | Rough size |
| --- | --- | --- | --- |
| 1 | `refactor(art): shapes and the inked finish` | Moves `Shape` (parse, blend, map, blob, taper) and the thick-and-thin line and multiplied shadow into `src/art/`. No visual change. | ~400 lines |
| 2 | `refactor: measure figures per look` | `Frame::of(build)` becomes `look.frame()` at all 17 call sites. No visual change. | ~150 lines |
| 3 | `feat(art): puppet part files` | The part-file loader, the part list and check, and blend-or-swap. Adds `fables puppet check`, `template` and `sheet` (the parts sheet). `world/puppets/kit.svg` and `stinky.svg` are generated once from the experiment's strings, and the experiment then draws from them. | ~1,100 lines |
| 4 | `feat(art): authored puppet` | `art::puppet` with the shared kit, the `FigureState` mapping, all 15 moods, the point hand and grotty or sparkly teeth. `figure::draw`, `shadow` and `draw_skeleton` hand authored characters to it, so the books, `fables sheets --moods` and `fables rig` draw Stinky from his part file; the rest stay on `figure.rs`. The experiment poses him through the library. | ~1,500 lines |
| 5 | `feat(art): props and extras` | Jam, toothbrush and parcel; hair follow-through; extras as kit parts. (Teeth landed in layer 4.) *Stinky McStinkbreath* renders fully in the new look. | ~600 lines |
| 6 | `feat(world): Bogey and Grubby` | Two kids, each with a signed-off model sheet. Their part files and the puppet features they need come first (pigtail locks that swing, hair sheen, dot and star motifs, skirts, trousers down the shin, long sleeves, a nose from the kit, ears that follow the head's width, plum shading on cool skin); wiring them into `cast.toml` waits for Ifiok's sign-off. | ~1 session each |
| 7 | `feat(world): Shouty and Fibby` | Two kids. | ~1 session each |
| 8 | `feat(world): Mum, Dad and Parcel` | Grown-up limbs and three designs. | ~1.5 sessions each |
| 9 | `feat(art): critters` | Biscuit on `rig::quadruped`, Wormsley, Brushy, and restyled birds, flowers and trees. | ~1,000 lines |
| 10 | `feat(art): outdoor settings` | Park, garden, street and town. | ~2 sessions |
| 11 | `feat(art): indoor settings` | Bedroom, bathroom and plain. | ~1.5 sessions |
| 12 | `chore: retire the old puppet` | Removes `figure.rs`, the old `Look` fields and the style skins; updates `world/kerfuffle.md`'s visual-style section and the `cast.toml` docs; regenerates proofs and videos. | -3,000 lines |

PRs 6 to 8 depend only on PR 5 and can be designed in parallel, one session per character. Settings (PRs 10 and 11) depend only on PR 1 and can start any time after it.

## Checks along the way

- **Unit tests:**
  - part files load;
  - paired drawings blend or fall back to a swap;
  - hands reach their IK targets;
  - the face warp leaves the front view unwarped;
  - every mood and pose maps to valid parts.
- `fables puppet check` runs in CI over every file in `world/puppets/`.
- **What reviewers look at:** the model sheet, turnaround, mouth chart and rig check for each character. The rendered *Stinky* proofs and a 16:9 video take the place of golden-image tests, which would churn with every art tweak.
- Keep an eye on video render time per frame. The new puppet draws more shapes (clips and multiplied shadows). The experiment's clip (210 frames at 1280×720, with one puppet and the park) took about 50 ms of CPU per frame, or 2.5 s on this machine with every core busy. Scenes with four or five puppets at 1080p will cost several times that, so measure before and after PR 4.

## Known rough edges to fix during the rollout

- Until layers 6 to 11 land, Stinky's finish sits beside the flat parametric cast and settings in the book.
- `mouth_size` and the shape fields (`head`, `body`, `nose`, `hair`) in `cast.toml` are ignored for authored puppets; layer 12 removes them.

- Hands and shoes pop when they swap drawings at the half-way point of a turn. Either add a mid-turn drawing or time the swap to the fastest frame of the turn.
- Turning to the page left mirrors the whole puppet, so asymmetric hair (Stinky's wave) flips. Any character whose asymmetry matters needs left-facing drawings.
- The back of the hair reads as one solid mass in three-quarter view; it needs a second strand group.
- The disgusted "ew" hand sits too close to the face.
- The test video has no audio. Lip-sync with a real ElevenLabs take still needs checking.
