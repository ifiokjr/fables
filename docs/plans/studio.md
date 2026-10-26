# Plan: fables as a studio

Ifiok wants fables to grow from one series' toolchain into a studio for many books and cartoons. An LLM designs the characters and worlds over MCP, any art style is a style pack, and characters speak with good AI voices. Written 26 October 2026, after layer 8 of the [authored-puppet plan](authored-puppets.md).

## Decisions

These were made on 26 October 2026.

| Question | Decision |
| --- | --- |
| What draws the art? | **Vector only.** Every drawing is an authored vector part, written by people or LLMs. New looks (anime, manga and more) are style packs, not image models. |
| How do cartoons move? | **Limited animation**, the way TV cut-out shows and most anime work: layered parts on a rig, with eye and mouth swaps, holds, camera moves and parallax. That is the timeline, rig and lip-sync fables already has. |
| Where does it live? | **This repository.** Kerfuffle becomes the first world, and nothing built so far is lost. |
| How do you drive it? | **An MCP server and the CLI.** No visual app yet. |

## What stays, what changes

Most of fables is already generic:
- the script format and staging;
- page layout and text cards;
- the ePub and KDP print presses;
- the video timeline, camera, rig, IK and lip-sync;
- voices and their cache.

Three things are specific to Kerfuffle, and they are what this plan generalises:

1. **One world.** The cast is found by walking up to `world/cast.toml`. Settings are Rust functions, and the palette is a Kerfuffle ink set.
2. **One look.** The authored puppet's finish and its face, eyes, hands and mouths are hard-wired. The six style skins only restyle the old parametric puppet.
3. **Humans do the drawing.** Part files are written by hand, or by an agent with ad hoc scripts. Nothing gives an LLM a clean loop of drawing a part, seeing it and fixing it.

## The shape

The project stays one library crate with one binary. A crate split happens only if something needs it.

```text
worlds/
  kerfuffle/
    world.toml        name, style, page size, narrator voice, series defaults
    cast.toml         characters (look, voice, puppet file)
    puppets/          one part file per character
    settings/         one part file per setting (layers: far, mid, near, front)
    stories/          scripts (story.md), each a book and its videos
styles/
  cartoon/            today's authored look
    style.toml        finish, proportions, eye and mouth rules, palette rules
    kit.svg           the parts every character in this style starts from
  anime/
  manga/
```

### Worlds

A world is a folder with a `world.toml`. Every command takes `--world` (or finds the nearest `world.toml` above the script), so many series can live side by side or in their own repositories. Kerfuffle moves to `worlds/kerfuffle/`, and its book renders byte-identically.

### Styles

A style is data plus a kit. `style.toml` sets:
- the finish: line weights, how lines thicken on the shadow side, cel-shadow tone and offset, and fills or screentones;
- body proportions: `KID` and `GROWN_UP` become per-style and named, such as chibi, kid, teen and adult;
- face rules: the face cylinder, eye construction (layered irises and highlights for anime), and how lids and brows act;
- palette rules: free colour, limited palettes, or black and white.

`kit.svg` holds the style's own hands, eyes, mouth chart, shoes and props. The authored puppet reads all of this instead of constants. The six old skins and the parametric puppet retire once every character in a world has a puppet (authored-puppet layer 12).

The first three styles:

| Style | Look |
| --- | --- |
| `cartoon` | Today's authored look, moved into data. |
| `anime` | Slender proportions; large eyes with layered irises and two highlights; spiky hair locks with swing; two-tone hard cel shading; thin even lines. |
| `manga` | Black ink, white paper and screentone fills (halftone and line tones as SVG patterns). Speed lines and impact frames as effects. Thicker outer lines. |

Video motion takes on the style too. Anime holds poses longer and snaps between them (animating on twos), with speed lines on fast moves. Cartoon keeps today's eased overshoot.

### Characters by LLM, over MCP

`fables mcp` serves the studio over stdio using the official Rust SDK (`rmcp`). Its tools follow the loop that made Stinky, Bogey and the rest: draw a part, render it, look, fix it. The renders come back as images, so the model sees what it drew.

| Tool | What it does |
| --- | --- |
| `world_create`, `world_describe` | Start a world from a brief, or read one back: its style, cast, settings and stories. |
| `style_list`, `style_describe` | The styles, their kits, proportions and part spaces. |
| `character_create` | A new cast entry and part file from a brief, a build and a style, started from the style's template parts. |
| `character_set_parts` | Write or replace parts as SVG path data, then run the part check. Returns the findings. |
| `character_sheet` | Render the model sheet (moods, poses, both views) and the parts sheet, as images. |
| `setting_create`, `setting_set_layers`, `setting_render` | The same loop for backgrounds. |
| `story_write`, `story_check` | Write a script and check it against the cast and settings. |
| `book_render`, `page_render` | Proofs, ePub and print, or one page as an image. |
| `voice_cast`, `voice_audition` | Choose a character's voice and hear a line in it. |
| `video_render` | A narrated, lip-synced cartoon, or a short preview. |

Sign-off stays human. A character's sheet is approved by a person before the character is used in a book, as it has been for every character so far.

### Voices

The provider layer stays as it is: a voice per character, the mood carried into the delivery, per-line caching, and lip-sync from alignment data when a provider returns it. It grows by one provider at a time behind the same `Voice` entry, so a new API is a file, not a redesign. Providers should be judged on the same checklist as [the TV-show guide](../tv-show.md): acting range, timing data for lip-sync, voice consistency over a series, licence and price. Keys come from Monosecret, resolved once per render.

## Order of work

The stack continues from layer 8. Each layer is one reviewable branch and leaves the Kerfuffle book working.

| # | Layer | What it does | Done when |
| --- | --- | --- | --- |
| 9 | `refactor: worlds` | `worlds/kerfuffle/` with `world.toml`; `--world` and discovery; the narrator, the recipe and the book's front and back matter per world. Palette and settings follow in layers 10 and 14. | The Kerfuffle book is byte-identical. A second, empty world renders a blank page. |
| 10 | `refactor(art): styles as data` | `styles/cartoon/` holds today's finish, proportions and kit; the puppet reads the style instead of constants. | Byte-identical book. `fables style check cartoon` passes. |
| 11 | `feat: MCP server` | `fables mcp` with the world, style, character and render tools, and sheets returned as images. | An agent creates a character from a brief and iterates its sheet without touching files. |
| 12 | `feat(styles): anime` | The anime finish, proportions, eyes and kit. A test world with one character designed through MCP. | That character's model sheet is signed off by Ifiok. |
| 13 | `feat(styles): manga` | Screentones, B/W finish, speed lines and impact frames; on-twos timing. | One signed-off manga character and a page. |
| 14 | `feat: settings as parts` | Settings become layered part files with MCP tools; Kerfuffle's settings are repainted (authored-puppet layers 10 and 11). | Every Kerfuffle setting is a part file. |
| 15 | `feat(voice): providers` | More providers behind `Voice`, and `voice_audition`. | A line auditions in each configured provider. |
| 16 | `feat: books and videos over MCP` | The story, book and video tools. | An agent writes, renders and voices a short book end to end in a new world. |

Authored-puppet layer 9 (Biscuit on a four-legged rig, Wormsley, Brushy and the critters) fits after 10, once rigs and styles are data. Layer 12 (retiring the old puppet and skins) comes last.

**Milestone one is layers 9 to 12:** an LLM designs an anime character over MCP, Ifiok signs off its sheet, and it appears on a book page and in a lip-synced clip.

## Risks

- **Vector anime and manga have a ceiling.** Hand-placed Béziers reach clean cel anime and inked manga, not painterly or highly rendered art. The styles aim at what vector does well: flat and cel colour, strong silhouettes, ink and screentone.
- **LLM drawing quality varies.** The sheet loop and the part check catch broken shapes, not ugly ones. Human sign-off stays the gate, and each style's template parts give every new character a good starting point.
- **Render time grows with style effects** (screentones, filters). Measure per-frame time when each style lands.

## Style dials and more packs

Approved by Ifiok on 26 October 2026: "Yes. Add the motion dial, the line and shade options and the new style packs (manga, rubber hose, clear line, mid-century, chibi) to the studio stack after anime."

Layers 13 to 16 had already landed by then. So these layers stack on top of layer 14 rather than being inserted after anime, and manga, built in layer 13 with its own switches, moves onto the dials.

A style becomes four dials: construction (the bodies, features, eyes and kit it already has), line, shade and motion.

| # | Layer | What it does | Done when |
| --- | --- | --- | --- |
| 17 | `feat(styles): dials` | `[motion]`, `[line]` and `[shade]` in style.toml (see the lists below). Today's cartoon values become the defaults. | Kerfuffle and the anime test world render byte-identically, and a short clip shows one character in two motion settings. |
| 18 | `refactor(styles): manga on the dials` | Black and white with screentones, a heavy tapered outer line, speed lines and impact frames, on threes with modulation and long holds. | Ren and his page re-render from the dials. |
| 19 | `feat(styles): rubber hose` | Hose limbs, round heads, pie-cut eyes and four-finger white gloves (a kit part); a uniform thick black line, flat colour and no shading; lots of squash, stretch and overshoot, and a bouncy idle loop on twos; an optional 1930s palette. | One test character designed through MCP. |
| 20 | `feat(styles): clear line` | Lines of equal importance with no hatching; flat vivid colours with at most one tone; cartoony characters on more realistic settings with depth from layered planes; restrained, eased motion. | One test character designed through MCP. |
| 21 | `feat(styles): mid-century` | Angular, geometric shapes; a limited flat palette; broken or absent lines, with colour slightly off-register; abstract settings; held poses that snap between keys, on twos and threes. | One test character designed through MCP. |
| 22 | `feat(styles): chibi` | A body about 2 to 2.5 heads tall with mitten hands, big or dot eyes, and bouncy motion. Mostly proportions and kit. | One test character designed through MCP. |

The dials:

- **Motion:**
  - timing on ones, twos or threes, with modulation (fast moves drop to ones, holds go to threes);
  - easing: anticipation and overshoot amounts, or a hard snap between keys;
  - squash and stretch;
  - moving holds (a slight drift while a pose is held);
  - speed-line smears on fast moves;
  - limbs: jointed (IK) or rubber hose (one smooth curve);
  - follow-through;
  - blink and mouth-flap rates;
  - an optional idle loop, such as a bounce.
- **Line:**
  - weight;
  - taper (brushed thick-and-thin, or uniform);
  - outer and inner weights;
  - colour: ink, or self-coloured (the fill, darkened);
  - broken lines;
  - fills off-register from their lines;
  - no line at all.
- **Shade:**
  - tones (0, 1 or 2) and the cel-shadow offset;
  - highlights;
  - fill: flat, a limited palette, or screentone (dot or line tone).

Every pack's test character is designed through the MCP loop, and their sheets are collected for sign-off together. Designs are original: no named artists or studios, no tracing, and no existing characters. Pixar-style 3D, watercolour, claymation and painted backgrounds are out of scope, because vector art can't reach them.

## Status

Every layer landed on 26 October 2026, as stacked local branches. Layer 14 went last, because 15 and 16 didn't depend on it.

- **Worlds and styles (9, 10).** Kerfuffle lives in `worlds/kerfuffle/`, and the cartoon look is data in `styles/cartoon/`. Both refactors kept the book byte-identical.
- **MCP (11, 16).** `fables mcp` serves the world, style, character, voice, story, book, page, video and setting tools.
  - `worlds/mumbleton/` was written, voiced and rendered end to end by an agent.
- **Anime and manga (12, 13).** `worlds/kiteharbour/` (Hana) and `worlds/hayate-street/` (Ren) were designed through MCP. Both model sheets are awaiting Ifiok's sign-off.
- **Settings (14).** The seven Kerfuffle settings are part files in `settings/`, ported from their code with depth haze added. Worlds add their own, such as Kiteharbour's harbour.
  - A fuller repaint to the [authored-puppet rules](authored-puppets.md#settings) (vanishing points, framing foregrounds) is now an art task on these files, not a code change.
- **Voices (15).** Cartesia and Hume sit behind `Voice`. They are unit-tested on their request shapes but have not been run against the live services.
