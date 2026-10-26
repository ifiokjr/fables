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

Commands find a script's world by walking up to the nearest `world.toml`. Commands without a script, such as `sheets`, `rig`, `new` and `puppet`, use the world above the current folder, or the only world in `./worlds/`. `--world <folder>` picks one explicitly, and a script can name its own with `world = "…"` in its front matter, relative to the script.

A world needs only a name to start:

```toml
name = "Kerfuffle"
style = "cartoon"           # optional: the drawing style its folk are drawn in (default cartoon)

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
| `manga` | Anime anatomy (it shares the anime kit and template) printed in black ink on white paper: every colour becomes paper, a dot screentone or solid black, on every page, print file and video frame. Heavier outlines, high-contrast eyes, and characters animated on twos (twelve drawings a second) while the camera moves on every frame. |

Every kid stands as tall on the page as a cartoon kid, and every grown-up as tall as the others, so a style's proportions change the drawing, not the page layout. [`worlds/kiteharbour/`](worlds/kiteharbour) is a small anime test world and [`worlds/hayate-street/`](worlds/hayate-street) a manga one; their stars, Hana and Ren, were designed entirely through the MCP tools below.

A style can set `screentone = true` under `[finish]` and `on-twos = true` under `[motion]`.

The built-in styles are compiled into fables, so a world anywhere on disk can name one. They are separate from the `--style` skins (riso, tv, bold…), which restyle the older parametric puppet and the settings.

## Designing characters with an agent (MCP)

`fables mcp` serves the studio over the Model Context Protocol on stdio, so an agent can design characters the way every Kerfuffle character was drawn: start from the style's template, redraw parts as SVG path data, look at the model sheet, fix, repeat. Sheets come back as images, so the model sees what it drew. Register it with any MCP client, for example:

```bash
claude mcp add fables -- fables mcp --root /path/to/your/studio
```

| Tool | What it does |
| --- | --- |
| `world_list`, `world_create`, `world_describe` | The worlds under `<root>/worlds/`: start one from a name, a few sentences and a style, or read one back with its cast and books. |
| `style_list`, `style_describe`, `style_sheet` | The drawing styles; every part a figure is drawn with, its coordinate space and the drawing rules; the kit as an image. |
| `character_create` | A new cast member from who they are and their colours, drawn from the style's template. |
| `character_parts` | A character's parts as path data, their own or the kit's. |
| `character_set_parts` | Writes or replaces parts and checks the figure. Changes that would break it are refused with the findings, so the world always loads. |
| `character_sheet` | The model sheet (five beats, front and three-quarters), the parts sheet, or all fifteen moods, as images. |
| `voice_providers`, `voice_list` | Which voice providers can speak now, and the offline macOS voices. |
| `voice_cast`, `voice_audition` | Give a character their voice, and hear a line in it, delivered in a mood, as audio. |
| `story_write`, `story_read`, `story_check` | Write a book's script into a world, checked against the cast (a script that doesn't rehearse is refused, with the line to fix), or read and check one. |
| `page_render`, `book_render` | One page as an image, or the proofs (with a contact sheet image), ePub and KDP print files under `<root>/out/`. |
| `video_render` | The narrated, lip-synced MP4, 9:16 or 16:9, or a quick preview. |

Worlds and characters are named by slugs, never paths, so the server only reads and writes under its root. A person signs off every model sheet before a character is used in a book; `fables puppet sheet <id>` writes the same model sheet at full size.

## Commands

| Command | What it makes |
| --- | --- |
| `fables new "<Title>" --star <id>` | A new book in the world's `stories/<title>/story.md`, laid out from its `recipe.md` with the star already staged. |
| `fables check <story>` | Validates the script and reports page and word counts. Errors give line numbers and list valid options. |
| `fables proofs <story>` | `out/<book>/<style>/proofs/`: one PNG per page plus `contact-sheet.png`, in about a second. |
| `fables ebook <story>` | `out/<book>/<style>/ebook/<book>.epub` (EPUB 3 fixed layout, passes epubcheck) and `cover.jpg` (2560 px store cover). |
| `fables print <story>` | `out/<book>/<style>/print/`: the interior and cover PDFs, plus `kdp-setup.txt` with the numbers to type into KDP. |
| `fables video <story> [--format landscape\|portrait\|both] [--preview]` | `out/<book>/<style>/video/`: MP4s, `soundtrack.wav`, `.srt` captions, a YouTube thumbnail and a 9:16 cover image. `--preview` renders at half size and 15 fps. |
| `fables book <story>` | Proofs, ePub and print. |
| `fables all <story>` | Everything. |
| `fables sheets [--moods <id>]` | Model sheets in `out/sheets/<style>/`: the whole cast, every setting, and one character in every mood. |
| `fables styles <story>` | `out/<book>/styles.png`: the cover and three pages drawn in every art style, side by side. |
| `fables rig <id>` | `out/rig/<style>/`: a rig sheet (every stance and gait phase with the skeleton drawn over the character) and a motion-test video. |

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

A script is TOML front matter between `+++` lines, then pages. Each page starts with `# setting`. Below the heading, narration paragraphs and `@` stage directions interleave in reading order.

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
| Setting | `plain`, `town`, `street`, `garden`, `park`, `bathroom` or `bedroom`. |
| `night`, `wilt=1`, `stink=0.5`, `mint=1`, `tint=sunflower` | Starting weather (`tint` colours the `plain` blob). |
| `speed=1`, `impact=1` | Manga effects: focus lines rushing in towards the first person on stage, or a jagged burst behind them. They work in every style. |
| `text=top`, `text=bottom`, `text=none` | Where the story text card goes. |
| `size=0.9` | Scales the page's text. |
| `zoom=1.5 look=0.5,0.6` | A close-up on the point (x, y), as fractions of the page. |
| `video=skip`, `book=skip` | Leave the page out of one format. |
| `no-wormsley` | Don't hide Wormsley on this page. |

### Stage directions

`@actor word word key=value…`

**Actors** are cast ids from the world's `cast.toml` (`@stinky`), copies of them (`@stinky.monday`), or generic critters: `bird`, `dog`, `worm`, `flower`, `tree`, `toothbrush` (`@bird.a`, `@flower.3`). `@scene …` changes the weather mid-page, and `@camera zoom=… look=…` the framing.

| Kind | Values |
| --- | --- |
| Moods | `happy` `grin` `laugh` `sad` `grumpy` `cross` `defiant` `shocked` `disgusted` `fainted` `dizzy` `sleepy` `dreamy` `worried` `proud` |
| Poses | `idle` `wave` `point` `arms-crossed` `hips` `cheer` `hold-nose` `cover-mouth` `brush` `shrug` `droop` `hold` `think` `reach-out` `pick` |
| Stances (legs and spine) | `stand` `crouch` `sit` `kneel` `jump` `bow` `tiptoe` |
| Gaits | `walk` `run` (cycles on the spot in video) `still` |
| Actions | `talk` (mouth moves with the next paragraph) `faint` `wake` `fly` `land` `wag` `calm` `hop` `flip` `hide` `show` `quiet` |
| Place | `x=left` (`far-left`, `center`, `right`, `far-right`, or `0`–`1`), `y=sky` (for flyers), `size=1.4`, `z=5` (draw order), `face=left`/`right`/`front` |
| Character | `teeth=clean`/`grotty`/`sparkly`, `stink=0`–`1`, `mint=0`–`1`, `holds=toothbrush`/`parcel`/`jam`/`nothing`, `mouth=0`–`1`, `colour=teal`, `wilt=0`–`1` (flowers) |
| Entrances | `enter=left`/`right`, `exit=left`/`right` (slide on and off in video; just appear or vanish in the book) |

Teeth, stink and mint **carry over from page to page** until changed, so grotty teeth stay grotty. Actors without an `x` are spaced out automatically.

**Timing and sound** (video only):

- `@pause 0.4` adds a beat of silence.
- `@sfx name` plays a built-in effect: `pop`, `plop`, `bonk`, `boing`, `whoosh`, `sparkle`, `tweet`, `blorp`, `page`.
- Fainting bonks, hops boing and entrances pop automatically.

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

These are code-drawn looks. For illustrator-quality drawn art, the same rig can take drawn parts instead; see "Drawn art" in [`docs/tv-show.md`](docs/tv-show.md).

**Every character has a full animation rig:**

- A skeleton: pelvis, spine, neck, head, shoulder–elbow–wrist and hip–knee–ankle.
- Elbows and knees solved by two-bone IK from where the hands and feet should be, the same way Spine, Moho and Harmony rigs work.
- On top of the skeleton:
  - **poses** for the arms;
  - **stances** for the legs and spine;
  - **gaits** that plant and lift the feet;
  - a face rig: eyes with lids and irises, brows, a nine-shape mouth chart for lip-sync, and head tilt.
- In video every move winds up, overshoots and settles.

Run `fables rig <id>` for the rig sheet and motion test.

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
  art/            puppets (figure.rs), the rig (rig.rs), styles (style.rs), critters, settings, effects
  press/          proofs, ePub, KDP print PDFs, model sheets
  video/          voices (TTS providers), lip-sync, timeline, frame renderer, soundtrack, ffmpeg
worlds/           one folder per series: world.toml, bible, cast, puppets, recipe and books
styles/           drawing styles: style.toml, kit and template per look
docs/             voices and turning the books into a show
assets/fonts/     Andika and Lilita One (SIL Open Font License)
```

## Known limits

- **Shapes.** The page is square (8.5" × 8.5") and spreads are single pages. A landscape trim or two-page spreads would need new view boxes in `book.rs` and `press/print.rs`.
- **Voices.** The ElevenLabs and OpenAI providers follow their current API docs and are unit-tested on request and response shapes, but haven't yet been run against the live services. Recorded human takes and speech-to-speech conversion aren't built yet (see `docs/tv-show.md`).
- **Read-along ePub.** Not built yet. The narration timings exist, so EPUB media overlays (read-along highlighting in Apple Books) are a natural next step.
- **New habits need new effects.** Effects live in `src/art/fx.rs`, so a book about a new habit (smouldering trousers, say) means a few dozen lines of drawing code.
