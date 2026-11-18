# fables

Write a children's picture book as one plain-text script, and get all of these from it:

- **page proofs** to review;
- a **fixed-layout ePub** for Kindle and Apple Books;
- a **KDP paperback** (interior and full-wrap cover PDFs);
- **narrated, voice-acted videos** for YouTube (16:9) and for Shorts, TikTok and Reels (9:16). The characters do what the narrator says and can speak their own lines in their own voices, lip-synced.

```bash
cargo run --release -- all worlds/kerfuffle/stories/stinky-mcstinkbreath/story.md
```

## Why it works this way

Every character is a **puppet drawn in code**, not a generated picture. That one decision is what makes the rest possible:

- **Characters stay on-model.** Stinky looks the same on page 3, page 23, the cover, the back cover and frame 4,812 of the video. AI image tools still struggle with that.
- **Stage directions are data.** `@stinky faint` is a still on the printed page and a bouncing pratfall in the video, from the same line of script.
- **It's free, offline and repeatable.** There are no API keys and no per-image costs, and the same script always gives the same book.
- **The world grows by editing text.** A new character is a new table in [`worlds/kerfuffle/cast.toml`](worlds/kerfuffle/cast.toml). A new book is a new `story.md`, and a new series is a new world.
- **It's cut-out animation, the TV technique.** Each character is built once from parts (head shapes, eyes with irises, a 9-shape mouth chart for lip-sync, tapered limbs, hands that point and make fists), the same way Toon Boom Harmony and Moho rigs work. Motion follows the classic principles: anticipation, follow-through, squash and stretch, walk cycles, parallax.

The house style, setting and series rules live in the series bible, [`worlds/kerfuffle/kerfuffle.md`](worlds/kerfuffle/kerfuffle.md). Character voices, voice APIs and turning the books into a show are covered in [`docs/tv-show.md`](docs/tv-show.md).

## Worlds

A world is a series: a folder with a `world.toml`, its cast, its puppets and its books. Kerfuffle is the first one.

```text
worlds/kerfuffle/
  world.toml      the series' name, its drawing style, its narrator, and the words in every book's front and back matter
  cast.toml       who lives there and what they look like
  puppets/        authored puppets' part files
  recipe.md       the 12-beat recipe `fables new` starts a book from
  stories/        one folder per book, each with a story.md
```

Commands find a script's world by walking up to the nearest `world.toml`. Commands without a script, such as `sheets`, `rig`, `new` and `puppet`, use the world above the current folder, or the only world in `./worlds/`. `--world <folder>` picks one explicitly, and a script can name its own with `world = "…"` in its front matter: a path relative to the script, such as `world = "../.."`, never an absolute one. Every build prints the folder it resolved as `world: <folder>`.

A world needs only a name to start:

```toml
name = "Kerfuffle"
style = "cartoon"           # optional: the drawing style its folk are drawn in (default cartoon)
light = "golden"            # optional: the light every page starts in (default day)

[narrator]                  # optional: who reads every book aloud, unless a script names its own [voice]
name = "Daniel"
rate = 158

[book]                      # optional: the words in the front and back matter
note = "Psst! Wormsley the worm is hiding on every page. Can you find him?"
meet = ["Have you met the rest of **Kerfuffle**?"]
draw = ["**Draw your own Kerfuffle folk!**"]
```

## Drawing styles

A drawing style is a folder in `styles/` that authored puppets are drawn with, so a new look is data rather than code:

```text
styles/cartoon/
  style.toml      the finish (line weights, light, cel shadow), the face cylinder, where the features sit, how eyes are built, the kit's inks, and the bodies (kid, grown-up)
  kit.svg         parts every character in the style starts with: hands, eyes, mouths, shoes, props
  template.svg    a plain character's own parts, which `fables puppet template` starts new characters from
```

| Command | What it does |
| --- | --- |
| `fables style list` | The drawing styles, with a sentence on each look. |
| `fables style check <name>` | Checks the template over the kit draws a whole figure, and every body stands up. |
| `fables style sheet <name>` | `out/styles/<name>-model.png` (the template kid on a model sheet) and `<name>-parts.png` (every kit and template part). |

| Style | Look |
| --- | --- |
| `cartoon` | Kerfuffle's look: round kids with big heads, brushed outlines that swell on the shadow side, one tone of cel shadow. |
| `anime` | Slender kids and grown-ups with the head about a quarter of their height; big eyes with a shaded iris, a pupil, a rim and two highlights; a tiny nose and mouth; thin even lines; hard two-tone cel shading. |
| `manga` | Anime anatomy (it shares the anime kit and template) printed in black ink on white paper: every colour becomes paper, a dot screentone or solid black, on every page, print file and video frame. A heavy tapered outer line and high-contrast eyes. Characters move on threes with modulation (fast moves on ones, long holds), trail speed lines on fast moves and flap their mouths, while the camera moves on every frame. |
| `manga-colour`, `manga-shojo`, `manga-shonen` | Manga in full anime colour, on the same kit and template. `manga-colour` keeps manga's heavy brushed line and motion on threes, with hard two-tone cel shading and a rim light. `manga-shojo` is delicate: thin self-coloured lines, very large sparkling eyes with long lashes, soft shading and gentle drifting motion on twos. `manga-shonen` is bold: the heaviest line, hard two-tone shadows, sharp narrow eyes, big wind-ups and speed lines. |
| `rubber-hose` | Early animated cartoons: round heads, pie-cut eyes, a button nose, white four-finger gloves, and arms and legs that are one bendy tube. A thick even black line, flat colour, no shading, big squash, stretch and overshoot, and an idle bounce on twos. `rubber-hose-1930s` is the same in soft blacks, greys and old paper white. |
| `clear-line` | Clear line: one even line weight everywhere, inside and out, with no hatching; flat vivid colour with no shading; simple characters with dot eyes, a short neck and gently tapering limbs with a knee and an elbow, for detailed settings that get their depth from layered planes; restrained, eased motion. Pages read best in the `clear` skin. |
| `mid-century` | Mid-century modern: angular, geometric characters on a wedge of a neck, with limbs cut as wedges and flat, pointed shoes; a small flat palette that every colour (settings included) snaps to; clean, even angular lines with the colour printed off them by one fixed slip, the same on every part; no shading; held poses that snap between keys on twos. Abstract settings suit it, in the riso skin. |
| `chibi` | Tiny characters about two heads tall: the anime face with enormous eyes, mitten hands, stubby limbs, soft self-coloured lines with a rim light, and springy, bouncy motion with a gentle idle bob. |

Every kid stands as tall on the page as a cartoon kid, and every grown-up as tall as the others, so a style's proportions change the drawing, not the page layout. [`worlds/kiteharbour/`](worlds/kiteharbour) is a small anime test world and [`worlds/hayate-street/`](worlds/hayate-street) a manga one, [`worlds/hoppington/`](worlds/hoppington) a rubber-hose one, [`worlds/tramline/`](worlds/tramline) a clear-line one with its own square, [`worlds/sundial-heights/`](worlds/sundial-heights) a mid-century one with an abstract plaza, and [`worlds/teacup-town/`](worlds/teacup-town) a chibi one; their stars, Hana, Ren, Clem, Zia, Otto and Mochi, were designed entirely through the MCP tools below.

A style is four dials: its construction (bodies, features, eyes and kit), and three tables in `style.toml` whose defaults are the cartoon look:

| Dial | Settings |
| --- | --- |
| `[line]` | `stroke` (brush thick-and-thin, uniform, broken or none), `colour` (ink, or `self`: each part's fill darkened), `register` (fills off-register from their lines). Weights are `line` and `detail` in `[finish]`. |
| `[shade]` | `tones` (0, 1 or 2), `highlight` (none or rim), `fill` (flat, a limited `palette`, or screentone), `tone` (dots or lines). |
| `[motion]` | `timing` (ones, twos or threes) and `modulate` (moves on ones, holds on threes), `easing` (eased or snap) with `anticipation` and `overshoot`, `squash`, `moving-hold`, `smears`, `limbs` (jointed or hose), `follow-through`, `blink-every`, `mouth-rate`, an `idle` loop (bounce) with `idle-amount` and `idle-tempo`, and the acting layer: `lead` (eyes first), `overlap` (parts arriving one after another), `arcs`, `takes`, `breath`, `weight-shift`, `gesture`, `listen`, `business`, and `travel` (walk or slide). See [docs/animation.md](docs/animation.md). |

Each body (`[bodies.kid]`, `[bodies.grown-up]`) can also say how it is built. Left out, every one of these draws the body exactly as before (round tube limbs, a plain neck, cartoon shoes):

| Setting | Meaning |
| --- | --- |
| `leg-profile`, `arm-profile` | Seven widths that shape a whole leg or arm as one outline: at the root, a third and two thirds of the way down the upper bone, at the knee or elbow, a third and two thirds of the way down the lower bone (where a calf or forearm swells and narrows), and at the ankle or wrist. A bent limb keeps a round outer curve over the joint and a crease inside it. |
| `foot-scale` | How much bigger than the kit's drawings shoes are drawn; they grow from the sole, so they still stand on the ground (default 1). |
| `knee-splay` | How much of a bent knee's sideways reach shows front-on: 1 (the default) bends knees out across the page; less brings them towards the viewer, so a squat foreshortens the thighs instead of splaying them. Turned, knees bend forwards whatever this is. |
| `floor` | `true` keeps the legs to the floor: a knee that would go through it rests on it, a foot behind a kneeling knee lies along the floor sole up, a foot lifted while standing points its toe down, and a skirt (`legwear` without `legwear-leg`) hangs from the waist under gravity, draping over the lap when the thighs come up, instead of turning with the body. It also fits the floor stances to the body's own legs: a crouch squats with the hips above the heels and the knees apart (deeper and wider on short legs, so a long top doesn't hide them), a kneel puts one knee on the floor and plants the other foot in front, and a sit rests the hips on the floor, or on clothes that hang below them, with both knees in front: raised on long legs, stretched out on short ones. Knees bend the way knees do whichever way the leg points. |

The kit can draw two more parts: `neck` (front and three-quarter, in bone units from the head's hinge down, under the top's neckline; without it the neck is a plain tube) and `shoe-sole` (front and three-quarter, inside the shoe, so a toe cap or heel can rise from the sole; without it the sole is a plain strip along the bottom).

`fables rig <id> --motion <style>` shows a character moving the way another style moves.

The built-in styles are compiled into fables, so a world anywhere on disk can name one. They are separate from the `--style` skins (riso, tv, bold…), which restyle the older parametric puppet and the settings.

## Settings

A setting is an SVG file of named parts on the square stage (2400 units, with 160 of overdraw past every edge). A part's id starts with its layer: `far-` (the sky and hills, which move least as the video camera travels), `near-` (behind the actors) or `front-` (in front of them). Parts are drawn through the page's art style, so the same file is a riso print, a cartoon or crayon, and they say how they react to the scene. A page's `light=` grades every colour the scene paints exactly once, paints `fill="sky"` parts with the light's sky gradient, and washes a haze and a vignette over the art; the story text and captions keep their own colours.

| Attribute | Meaning |
| --- | --- |
| `fill` | A palette name or hex code; `tint` is the scene's tint; `sky` is the scene light's sky gradient (flat daylight paints the plain sky colour); `none` for a line, with `stroke` and `stroke-width` (`data-noodle` for a thick inked tube). |
| `data-line="none"` | A fill without an outline (skies, lawns, stripes). |
| `data-night`, `data-wilt` | The colour on a bare night (a page with no `light=`), and the colour it turns as the scene wilts. Parts without a night colour dim. Under a scene light the light alone makes the night: every part keeps its day colour and the light grades it. |
| `data-time` | `day` or `night`: only drawn then (the sun, the moon). Under a scene light the light's sky body decides: `dusk` shows the sun, `moonlit` the moon and stars, `overcast` and `storm` cover the sun with cloud, and an `indoor` or `candle` light shows neither. |
| `data-lit` | Comma-separated light presets (`dusk,night`): only drawn under one of those lights (lit windows at dusk, stars at night). |
| `data-gradient` | `top colour bottom colour`: a vertical gradient over the part (walls, water, lawns, floors). Both colours wilt, fade with distance, dim on a bare night and grade with the light like a flat fill; on a bare night a part with a `data-night` colour turns that flat colour, and under a scene light both stops keep the gradient. |
| `data-glow` | On a `<circle>`, `radius colour opacity`: a soft radial pool of light fading to transparent (a lamp, a window, the sun's bloom). |
| `data-depth` | 0 to 1: far things fade into the sky and lose line. |
| `data-dots` | A riso dot screen: `colour spacing radius opacity`. |
| `data-drift`, `data-swing` | Clouds drift sideways; a swing swings from its pivot (`x y`). |

`<circle data-sparkle="size colour">` twinkles, and `<circle data-critter="flower|tree" r="height" data-colour data-seed>` places one of the living plants, which have faces and wilt. The root `<svg>` sets `data-ground`, where actors stand.

The fifteen built-in settings (`plain`, `town`, `street`, `garden`, `park`, `bathroom`, `bedroom`, `kitchen`, `classroom`, `forest`, `beach`, `hilltop`, `shop`, `playroom`, `stage`) live in [`settings/`](settings) and every world can use them. A world adds its own, or repaints a built-in one by name, in its `settings/` folder; over MCP, `setting_create`, `setting_parts`, `setting_set_parts` and `setting_render` do the same.

## Designing characters with an agent (MCP)

`fables mcp` serves the studio over the Model Context Protocol on stdio, so an agent can design characters the way every Kerfuffle character was drawn: start from the style's template, redraw parts as SVG path data, look at the model sheet, fix, repeat. Sheets come back as images, so the model sees what it drew. Register it with any MCP client, for example:

```bash
claude mcp add fables -- fables mcp --root /path/to/your/studio
```

| Tool | What it does |
| --- | --- |
| `guide` | The director's guide ([`docs/directing.md`](docs/directing.md)), by topic: overview, story, staging, camera, light, acting, mixing, workflow. |
| `world_list`, `world_create`, `world_describe` | The worlds under `<root>/worlds/`: start one from a name, a few sentences and a style, or read one back with its cast and books — and the other worlds whose characters a script's `guests` could invite. |
| `style_list`, `style_describe`, `style_sheet` | The drawing styles; every part a figure is drawn with, its coordinate space and the drawing rules; the kit as an image. |
| `character_create` | A new cast member from who they are and their colours, drawn from the style's template. |
| `character_parts` | A character's parts as path data, their own or the kit's. |
| `character_set_parts` | Writes or replaces parts and checks the figure. Changes that would break it are refused with the findings, so the world always loads. |
| `character_sheet` | The model sheet (five beats, front and three-quarters), the parts sheet, or all fifteen moods, as images. |
| `voice_providers`, `voice_list` | Which voice providers can speak now, and the offline macOS voices. |
| `voice_cast`, `voice_audition` | Give a character their voice, and hear a line in it, delivered in a mood, as audio. |
| `story_write`, `story_read`, `story_check` | Write a book's script into a world, checked against the cast (a script that doesn't rehearse is refused, with the line to fix), or read and check one. |
| `story_review` | A director's critique of a book: findings with a severity, the script line and a suggestion. Advice, never a refusal. |
| `storyboard_render`, `video_frames` | The storyboard as an image, one labelled panel per camera setup; and sampled frames of the video as one tiled image, rendered offline with estimated timings. |
| `light_list`, `shot_list` | The light presets and the camera vocabulary (shot sizes, moves, arrivals, transitions), with a sentence on each. |
| `page_render`, `book_render` | One page as an image, or the proofs (with a contact sheet image), ePub and KDP print files under `<root>/out/`. |
| `video_render` | The narrated, lip-synced MP4, 9:16 or 16:9, or a quick preview. |
| `setting_list`, `setting_create` | A world's settings, built-in and its own; start one, blank or from a setting to repaint. |
| `setting_parts`, `setting_set_parts`, `setting_render` | Read and redraw a setting's parts as SVG elements, and see it by day or night, wilted or not. |

Worlds, books, settings and characters are named by slugs, never paths, so the server only reads and writes under its root: a name that is absolute or steps out with `..` is refused before any file is touched. A book written over MCP belongs to the world it is written into, so `story_write` refuses a script whose front matter names a `world`; at the command line, name one with `--world` or a relative `world =`. A person signs off every model sheet before a character is used in a book; `fables puppet sheet <id>` writes the same model sheet at full size.

## Commands

| Command | What it makes |
| --- | --- |
| `fables new "<Title>" --star <id>` | A new book in the world's `stories/<title>/story.md`, laid out from its `recipe.md` with the star already staged. |
| `fables check <story>` | Validates the script and reports page and word counts. Errors give line numbers and list valid options. |
| `fables review <story>` | A director's critique of the rehearsed script ([`docs/directing.md`](docs/directing.md)): line-numbered findings and suggestions about staging, mood arcs, pacing, light and shots. Advice, never a refusal; ends with a count of findings. |
| `fables proofs <story>` | `out/<book>/<style>/proofs/`: one PNG per page plus `contact-sheet.png`, in about a second. |
| `fables ebook <story>` | `out/<book>/<style>/ebook/<book>.epub` (EPUB 3 fixed layout, passes epubcheck) and `cover.jpg` (2560 px store cover). |
| `fables print <story>` | `out/<book>/<style>/print/`: the interior and cover PDFs, plus `kdp-setup.txt` with the numbers to type into KDP. |
| `fables video <story> [--format landscape\|portrait\|both] [--preview]` | `out/<book>/<style>/video/`: MP4s, `soundtrack.wav`, `.srt` captions, a YouTube thumbnail and a 9:16 cover image. `--preview` renders at half size and 15 fps. |
| `fables storyboard <story> [--format landscape\|portrait]` | `out/<book>/<style>/storyboard/storyboard.png` (paged as `storyboard-2.png`, … past 24 panels): the video drawn as a comic strip, one labelled panel per camera setup. A hold a script-written cut leaves under 1.2 s is labelled `· short`. Never records a voice, so it runs offline in a second or two. |
| `fables book <story>` | Proofs, ePub and print. |
| `fables all <story>` | Everything. |
| `fables sheets [--moods <id>]` | Model sheets in `out/sheets/<style>/`: the whole cast, every setting, the world's colour script (`<world>-lights.png`: one setting with one cast member under every light), and one character in every mood. |
| `fables styles <story>` | `out/<book>/styles.png`: the cover and three pages drawn in every art style, side by side. |
| `fables rig <id>` | `out/rig/<style>/`: a rig sheet (every stance and gait phase with the skeleton drawn over the character), a motion-test video, and an acting-test video (the ten acting rules, in order, on two characters). |

Every command that takes `<story>` also takes a folder of books or a world: `fables all worlds/kerfuffle` builds every `story.md` in it.
Every command also takes `--style`, so any render can be in any style without editing the script:

- `--style storybook` draws in one style;
- `--style bold,tv` draws in each listed style;
- `--style all` draws in all six.

Without `--style`, a book uses the `style` in its front matter (or `riso`), and sheets and rigs use `riso`. Each style renders to its own folder, `out/<book>/<style>/`, so styles sit side by side and never overwrite each other. The voice cache is shared, so re-rendering a video in another style records nothing new.

Install with `cargo install --path .`, or use `cargo run --release --` in place of `fables`.

### Requirements

- **Rust** (edition 2024).
- **ffmpeg** with libx264, for video.
- **A narrator.** macOS `say` by default, or any TTS command (see [Voices](#voices)).

> On macOS, if linking fails with "You have not agreed to the Xcode license agreements", run `sudo xcodebuild -license` once. You can also use the Nix shell from [`devenv.nix`](devenv.nix): `devenv shell`.

## Writing a script

A script is TOML front matter between `+++` lines, then pages. Each page starts with `# setting`. `title` and `author` are required, and the title needs at least one letter or digit: its slug names the output folder. Below the heading, narration paragraphs and `@` stage directions interleave in reading order.

```text
+++
title = "Stinky McStinkbreath"
author = "Ifiok Jr."
star = "stinky"                 # who goes on the cover
moral = "Brush your teeth, morning and night."

[cover]
tint = "moss"
pose = "x=0.5 grin wave teeth=grotty stink=1 talk"
+++

# bathroom night
@mum x=0.72 point cross talk
@stinky x=0.3 arms-crossed defiant
"Brush your teeth, Stanley!" said Mum.
@stinky talk
"**NOPE**," said Stanley.
```

- **In the book**, each page shows the stage *at the end of the page*. The text sits on a paper card, at the top unless the heading says `text=bottom`.
- **In the video**, each `@` line takes effect *when the narrator reaches it*. Above, Mum points and talks during the first line; Stanley crosses his arms and his mouth moves during the second.

### Text

| Write | Get |
| --- | --- |
| A paragraph | Printed in the book and read in the video. Consecutive lines join into one paragraph; a blank line, `@` line or heading ends it. |
| `~ A paragraph` | **Book only.** Use it to trim the video (Shorts are limited to 3 minutes). |
| `> A paragraph` | **Video only.** A summary line for the narrator. |
| `*word*` | Bold. |
| `**WORD**` | Shouted: big, red, in the display face. |
| `// note` | A comment, ignored. |

### Page headings

`# setting option option…`

| Option | Meaning |
| --- | --- |
| Setting | `plain`, `town`, `street`, `garden`, `park`, `bathroom`, `bedroom`, `kitchen`, `classroom`, `forest`, `beach`, `hilltop`, `shop`, `playroom`, `stage`, or one of the world's own (see Settings). |
| `night`, `wilt=1`, `stink=0.5`, `mint=1`, `tint=sunflower` | Starting weather (`tint` colours the `plain` blob). |
| `light=golden` | The scene's light: `day` (the default), `dawn`, `golden`, `dusk`, `night`, `moonlit`, `overcast`, `storm`, `indoor` or `candle`. It grades every colour in the scene once, paints the sky, aims the shadows and adds haze and a vignette. A world can set a default in `world.toml`, and a book can override it with `light = "…"` in its front matter. A bare `night` keeps today's behaviour exactly unless a `light=` is also given. Under a `light=` the light alone makes the night: parts keep their day colour and the light grades it, so `data-night` colours are for bare nights. |
| `speed=1`, `impact=1` | Manga effects: focus lines rushing in towards the first person on stage, or a jagged burst behind them. They work in every style. |
| `text=top`, `text=bottom`, `text=none` | Where the story text card goes. |
| `size=0.9` | Scales the page's text. |
| `zoom=1.5 look=0.5,0.6` | A close-up on the point (x, y), as fractions of the page. |
| `shot=… on=…` | A shot size that frames the still illustration: `wide`, `full`, `medium`, `close` or `extreme-close` on the actors named by `on=` (comma-separated ids; empty means everyone on stage, or whoever speaks for a close shot). Framing is measured from what the puppets actually draw — hair, raised arms, held props and all — with headroom and lead room, so a page can be a close-up without typing coordinates. A still is framed into the part of the page its text card leaves clear, falling back to a tighter shot when a long card leaves no room, so a head is never hidden under the words. |
| `transition=wipe` | How the page is entered in the video: `wipe` (the paper sheet), `cut`, `dissolve`, `fade`, `iris` or `push`. `[video] transition = "…"` sets a book's default. |
| `video=skip`, `book=skip` | Leave the page out of one format. |
| `no-wormsley` | Don't hide Wormsley on this page. |

### Stage directions

`@actor word word key=value…`

**Actors** are cast ids from the world's `cast.toml` (`@stinky`), copies of them (`@stinky.monday`), or generic critters: `bird`, `dog`, `worm`, `flower`, `tree`, `toothbrush` (`@bird.a`, `@flower.3`). `@scene …` changes the weather mid-page (`wilt=`, `stink=`, `light=…`, and friends; the video eases the change over about a second), and `@camera …` the framing.

| Kind | Values |
| --- | --- |
| Moods | `happy` `grin` `laugh` `sad` `grumpy` `cross` `defiant` `shocked` `disgusted` `fainted` `dizzy` `sleepy` `dreamy` `worried` `proud` |
| Poses | `idle` `wave` `point` `arms-crossed` `hips` `cheer` `hold-nose` `cover-mouth` `brush` `shrug` `droop` `hold` `think` `reach-out` `pick` |
| Stances (legs and spine) | `stand` `crouch` `sit` `kneel` `jump` `bow` `tiptoe` |
| Gaits | `walk` `run` (cycles on the spot in video) `still` |
| Actions | `talk` (mouth moves with the next paragraph) `faint` `wake` `fly` `land` `wag` `calm` `hop` `flip` `hide` `show` `quiet` |
| Place | `x=left` (`far-left`, `center`, `right`, `far-right`, or `0`–`1`), `y=sky` (for flyers), `size=1.4`, `z=5` (draw order), `face=left`/`right`/`front` |
| Character | `teeth=clean`/`grotty`/`sparkly`, `stink=0`–`1`, `mint=0`–`1`, `holds=toothbrush`/`parcel`/`jam`/`nothing`, `mouth=0`–`1`, `colour=teal`, `wilt=0`–`1` (flowers) |
| Entrances | `enter=left`/`right`, `exit=left`/`right` (figures walk on and off in video, critters keep their own entrances; in the book they just appear or vanish) |
| Camera | `shot=wide`/`full`/`medium`/`close`/`extreme-close` framed on `on=<actor>,<actor>` (empty: everyone, or the speaker for a close), `move=push`/`pull`/`pan-left`/`pan-right`/`tilt-up`/`tilt-down`/`track` taking `over=<seconds>` (default the rest of the shot), `shake=0`–`1`, and `cut` or `ease` to choose how the camera arrives (a change of shot cuts unless the direction says `ease`). The older `zoom=` and `look=` still work and win when given. |

### Who points the camera

`[video] direction` chooses between two directors:

- `"auto"` (the default): pages without any `@camera` direction are shot by film rules. Every page opens on an establishing wide or full shot long enough to read; whoever speaks gets a medium (landscape) or close (portrait); a two-way exchange alternates shot and reverse shot; very short lines share a two-shot; a strong mood on somebody who isn't speaking earns a close reaction shot; entrances, exits, walks, runs, hops and faints widen to a full shot so the action stays in frame. No shot runs shorter than 1.2 s, cuts land at most one per 1.2 s and never just after a line begins, and a framing held longer than 8 s gets a slow push.
- `"locked"`: the older camera, kept byte for byte — one auto-framed view per scene key and line, gliding between them.

`[video] acting = "plain"` turns the acting layer off and reproduces the plain motion; every style's `[motion]` dials tune it, and [`docs/animation.md`](docs/animation.md) has the whole story.

A page with any `@camera` direction is shot exactly as written, in both modes' vocabulary. Setups on directed pages follow the same 1.2 s floor: one that would flash by is merged into its neighbour, but a cut the script wrote is honoured — the storyboard marks the hold it leaves under 1.2 s with `· short`. Every shot frames its subject's whole head, hair and all — measured from the puppet's own drawing, so a tall hairdo, a raised hand or a held prop is never guessed at — with headroom above and room beside it (room enough to survive the slow push a long hold rides in on); only an extreme close-up may crop hair above the brow line, and lead room never pushes a head out of frame. Close and extreme-close shots soften the setting behind their subject (depth of field); `[video] depth-of-field = false` turns that off.

Teeth, stink and mint **carry over from page to page** until changed, so grotty teeth stay grotty. Actors without an `x` are spaced out automatically.

**Timing and sound** (video only):

- `@pause 0.4` adds a beat of silence, of at most 60 seconds.
- `@sfx name` plays a built-in effect: `pop`, `plop`, `bonk`, `boing`, `whoosh`, `sparkle`, `tweet`, `blorp`, `page`.
- Fainting bonks, hops boing and entrances pop automatically.
- A video runs at most 30 minutes (1800 seconds); a longer script is refused before any audio or frame is made.

### Guests

A book can invite characters from other worlds under the same root, and each keeps looking like themselves:

```text
guests = ["kiteharbour/hana", "kerfuffle/stinky"]   # front matter
guest-style = "host"                                 # optional
```

A guest is directed by its id (`@hana`), or by `world.id` (`@kiteharbour.hana`) when its id clashes with a local cast member — then the bare id means the local one. A guest keeps their puppet parts, their own style's construction (bodies, features, eyes, kit) and their voice, and by default their own style's line and shading as well, so a cartoon kid and an anime kid can share a page looking like themselves. `guest-style = "host"` inks and shades guests with the host world's `[line]` and `[shade]` dials instead; their construction never changes. Everyone on a page moves with the host story's `[motion]`, and guests stand at the height their own world gives them. A missing world or character is an error naming the ones that do exist, and the "meet the folk" page still lists only the host world's cast.

### New characters

Add a table to the world's cast file, such as [`worlds/kerfuffle/cast.toml`](worlds/kerfuffle/cast.toml), built from puppet parts, then run `fables sheets` to see them:

```toml
[grubby]
name = "Grubby McGrubhands"
born = "Gus"
habit = "Never washes his hands, so everything he touches turns to mud."
skin = "teal"
hair = "curls"            # bald tuft bun curls pigtails bob spikes puff fringe
hair-colour = "sunflower"
top = "tomato"
pattern = "star"          # plain stripes dots star
trim = "sunflower"
legwear = "trousers"      # shorts trousers skirt
bottom = "night"
extras = ["muddy-hands"]  # freckles glasses moustache apron post-cap satchel bow muddy-hands bogey lashes
head = "wide"             # round egg wide
body = "round"            # bell bean pear round
nose = "round"            # button round long none
eyes = "cocoa"            # iris colour

[grubby.voice]            # his own voice for dialogue (see Voices)
name = "Eddy (English (UK))"
style = "A friendly, slightly mumbly six-year-old boy."
```

Colours are palette names (see the bible) or `#rrggbb`. A character with a `habit` appears on the "Have you met the rest of Kerfuffle?" page in every other book.

## Art styles and the rig

**Styles are skins.** Choose one per book with `style = "..."` in the front matter, or render in any others with `--style` (one, a comma list, or `all`). `fables styles <story>` draws the same pages in every style so you can compare.

| Style | Look |
| --- | --- |
| `riso` (default) | Two-colour riso print: flat inks, a wobbly line, fills off register, dot screens |
| `tv` | Modern preschool TV: no outlines, clean shapes, soft gradient shading |
| `bold` | Classic cartoon: heavy outlines, hard cel shadows, glossy highlights |
| `cut-paper` | Collage: paper fibres, torn edges, each shape casting a shadow |
| `crayon` | Waxy scribbled colour that strays over the lines |
| `storybook` | Pencil lines over soft watercolour washes on textured paper |
| `clear` | One even outline everywhere, flat colour, no shading or texture (for the clear-line style) |

These are code-drawn looks. For illustrator-quality drawn art, the same rig can take drawn parts instead; see "Drawn art" in [`docs/tv-show.md`](docs/tv-show.md).

**Every character has a full animation rig:**

- A skeleton: pelvis, spine, neck, head, shoulder–elbow–wrist and hip–knee–ankle.
- Elbows and knees solved by two-bone IK from where the hands and feet should be, the same way Spine, Moho and Harmony rigs work.
- On top of the skeleton:
  - **poses** for the arms;
  - **stances** for the legs and spine;
  - **gaits** that plant and lift the feet;
  - a face rig: eyes with lids and irises, brows, a nine-shape mouth chart for lip-sync, and head tilt.
- In video every move winds up, overshoots and settles, and the acting rules on top of the
  rig make characters perform instead of sliding between poses — see
  [`docs/animation.md`](docs/animation.md).

Run `fables rig <id>` for the rig sheet, the motion test and the acting test.

## Voices

**The narrator** is the world's `[narrator]` table, or the script's own `[voice]` table. **Each character** can have their own `[id.voice]` table in the cast file. With `[video] cast-voices = true`, quoted dialogue is spoken by the character whose `talk` direction introduced it, in their voice, and the narrator reads the rest:

```text
@mum point cross talk
"Brush your teeth, Stanley!" said Mum.     ← Mum's voice, then the narrator's
```

Add `drop-tags = true` to skip short tags like "said Mum.", which makes the video play like a show.

The script's mood (`cross`, `defiant`, `sad`…) becomes the line's delivery, and `**shouted**` words are shouted. Mouths lip-sync to whoever is speaking, and everyone else looks at them.

| `provider` | Needs | Acting | Lip-sync |
| --- | --- | --- | --- |
| `say` (default) | macOS | speaking rate | estimated from the audio |
| `command` | any TTS program; `{text}`, `{out}`, `{mood}`, `{voice}` placeholders, text on stdin if `{text}` is absent | `{mood}` | estimated from the audio |
| `elevenlabs` | `id`, optional `model` (default `eleven_v4`), `ELEVENLABS_API_KEY` | `[defiant]`-style audio tags | exact, per letter |
| `openai` | `name` (a built-in voice), optional `style`, `OPENAI_API_KEY` | `style` plus the mood as an acting instruction | estimated from the audio |
| `cartesia` | `id` (a Sonic voice), optional `model` (default `sonic-3.6`), `CARTESIA_API_KEY` | the mood as one of Sonic's emotions, and a speed | estimated from the audio |
| `hume` | `id` (an Octave voice), optional `style`, `HUME_API_KEY` | `style` plus the mood as short acting directions | estimated from the audio |

```toml
[narrator]                  # in world.toml: the narrator (or [voice] in a script, for one book)
name = "Daniel"             # any voice from `say -v '?'`; Enhanced and Premium voices sound best
rate = 158

[stinky.voice]              # in the world's cast.toml: Stinky, in every book
provider = "elevenlabs"
id = "<voice id>"
```

`fables voice providers` shows which providers can speak right now, `fables voice list` the macOS voices, and `fables voice audition <id> "<line>" --mood cross` speaks a line in a character's voice (or `narrator`) into `out/voices/`. Over MCP, `voice_providers`, `voice_list`, `voice_cast` and `voice_audition` do the same, and auditions come back as audio.

Takes are cached in `out/<book>/cache/voice`, keyed by voice, mood and text, so only edited lines are re-recorded. Paid API calls fail once and are never retried in a loop. Resolve keys with Monosecret once per batch: `msr --reason "voice the books" fables all worlds/kerfuffle`.

Sourcing voices, provider prices and policies, local models, human actors and keeping characters consistent are all in [`docs/tv-show.md`](docs/tv-show.md).

## Publishing checklists

**KDP paperback.** `print/kdp-setup.txt` has the numbers.

- Trim 8.5" × 8.5", bleed on, premium colour (the only colour option under 72 pages). The book is padded to an even page count of at least 24.
- Upload `*-interior.pdf` and `*-cover.pdf`.
- Interior pages are 8.625" × 8.75", with bleed only on the outer edge and trim boxes set. Text stays 0.375" inside the trim.
- The cover's spine is 0.002347" per page and has no text (KDP only allows spine text from 79 pages). KDP's barcode area, at the lower right of the back cover, is kept clear.
- Art is 300 DPI RGB. **Order a printed proof** before publishing: KDP converts to CMYK, and bright pinks and greens shift.

**KDP ebook.**

- Upload the `.epub` directly; KDP accepts fixed-layout EPUB. Upload `ebook/cover.jpg` as the cover.
- The ePub declares `book-type: children` and `original-resolution`. Story text is live text and every illustration has alt text.
- It's about 9 MB. At the 70% royalty rate, KDP charges a delivery fee per megabyte. To shrink it, lower `PAGE_PX` or `QUALITY` in `src/press/epub.rs`.

**Apple Books.** Same ePub. Apple rejects picture books whose text is baked into images, which is why the text is live.

**YouTube.** Upload `*-16x9.mp4`, `*-thumbnail.jpg` and the `.srt` captions, and set the audience to **"Made for kids"** (a legal requirement under COPPA). The soundtrack is mastered to −16 LUFS.

**Shorts, TikTok, Reels.** Upload `*-9x16.mp4` with `*-cover-9x16.jpg`.

- Keep the video under 3 minutes for Shorts; trim with `~` book-only paragraphs.
- Captions sit high on the screen, clear of the app buttons at the bottom and right.

**AI disclosure.** KDP asks whether text and images are AI-generated. The example book's text draft and the drawing code were written with an AI model. Answer honestly when you publish.

## Layout

```text
src/
  story.rs        script parser (front matter, pages, beats, markup)
  cast.rs         cast file → puppet looks
  stage.rs        scene model, stage directions, rehearsal across pages
  compose.rs      scene → positioned puppets (blendable for animation)
  text.rs         page text layout with real font metrics
  book.rs         page assembly: title, copyright, story, back matter, covers
  world.rs        worlds: discovery, world.toml, the narrator and book matter
  art/pack.rs     drawing styles: style.toml, kit and template
  studio.rs       worlds and characters as operations: what the CLI and MCP server share
  mcp.rs          `fables mcp`: the studio's tools over the Model Context Protocol
  publish.rs      a book's outputs: the check, proofs, ePub, print files and videos
  review.rs       the critique: findings from a rehearsed story and its planned video
  art/            puppets (figure.rs), the rig (rig.rs), styles (style.rs), critters, settings, effects
  press/          proofs, ePub, KDP print PDFs, model sheets
  video/          voices (TTS providers), lip-sync, timeline, the director's camera setups, frame renderer, soundtrack, ffmpeg
worlds/           one folder per series: world.toml, bible, cast, puppets, settings, recipe and books
settings/         the built-in settings, as part files
styles/           drawing styles: style.toml, kit and template per look
docs/             voices and turning the books into a show
assets/fonts/     Andika and Lilita One (SIL Open Font License)
```

## Known limits

- **Shapes.** The page is square (8.5" × 8.5") and spreads are single pages. A landscape trim or two-page spreads would need new view boxes in `book.rs` and `press/print.rs`.
- **Voices.** The ElevenLabs and OpenAI providers follow their current API docs and are unit-tested on request and response shapes, but haven't yet been run against the live services. Recorded human takes and speech-to-speech conversion aren't built yet (see `docs/tv-show.md`).
- **Read-along ePub.** Not built yet. The narration timings exist, so EPUB media overlays (read-along highlighting in Apple Books) are a natural next step.
- **New habits need new effects.** Effects live in `src/art/fx.rs`, so a book about a new habit (smouldering trousers, say) means a few dozen lines of drawing code.
