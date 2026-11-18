# fables

[![CI](https://github.com/ifiokjr/fables/actions/workflows/ci.yml/badge.svg)](https://github.com/ifiokjr/fables/actions/workflows/ci.yml)

fables turns a picture-book script written as plain text into page proofs, a fixed-layout ePub,
KDP paperback PDFs and narrated, lip-synced videos.

**Status: early.** The script, the settings and the output formats may still change.

## Quick start

With [Rust](https://rustup.rs) installed, plus `ffmpeg` and a voice for the narration (macOS `say`
works out of the box; see [Voices](#voices)):

```bash
git clone https://github.com/ifiokjr/fables
cd fables
cargo run --release -- proofs worlds/kerfuffle/stories/stinky-mcstinkbreath/story.md
cargo run --release -- storyboard worlds/kerfuffle/stories/stinky-mcstinkbreath/story.md
cargo run --release -- video --preview --format landscape worlds/kerfuffle/stories/stinky-mcstinkbreath/story.md
```

The proofs and contact sheet, the storyboard and the preview video land under `out/`. That renders
an example world; to make your own from nothing, start with
[Your first cartoon in five minutes](docs/first-cartoon.md).

## Why it works this way

Every character is a **puppet drawn in code**, not a generated picture. That one decision is what makes the rest possible:

- **Characters stay on-model.** Stinky looks the same on page 3, page 23, the cover, the back cover and frame 4,812 of the video. AI image tools still struggle with that.
- **Stage directions are data.** `@stinky faint` is a still on the printed page and a bouncing pratfall in the video, from the same line of script.
- **It's free, offline and repeatable.** There are no API keys and no per-image costs, and the same script always gives the same book.
- **The world grows by editing text.** A new character is a new table in [`worlds/kerfuffle/cast.toml`](worlds/kerfuffle/cast.toml). A new book is a new `story.md`, and a new series is a new world.
- **It's cut-out animation, the TV technique.** Each character is built once from parts (head shapes, eyes with irises, a 9-shape mouth chart for lip-sync, tapered limbs, hands that point and make fists), the same way Toon Boom Harmony and Moho rigs work. Motion follows the classic principles: anticipation, follow-through, squash and stretch, walk cycles, parallax.

The house style, setting and series rules live in the series bible, [`worlds/kerfuffle/kerfuffle.md`](worlds/kerfuffle/kerfuffle.md). Character voices, voice APIs and turning the books into a show are covered in [`docs/tv-show.md`](docs/tv-show.md).

## Where to read next

1. **[Your first cartoon in five minutes](docs/first-cartoon.md)**: an empty folder to a rendered book, storyboard and preview video, using only the command line.
2. **[Directing](docs/directing.md)**: the craft rules `fables review` applies.
3. **[Animation](docs/animation.md)**: how the rig, the acting layer and the camera move.
4. **[Designing characters with an agent](#designing-characters-with-an-agent-mcp)**: the MCP server that draws, checks and shows model sheets.
5. **[Voices and the show](docs/tv-show.md)**: real voices, character consistency and turning the books into a series.

## Worlds

A world is a series: a folder with a `world.toml`, its cast, its puppets and its books. Kerfuffle is the first one.

```text
worlds/kerfuffle/
  world.toml      the series' name, its drawing style, its narrator, and the words in every book's front and back matter
  cast.toml       who lives there and what they look like
  puppets/        authored puppets' part files
  props/          the world's own props, as part files (optional)
  recipe.md       the 12-beat recipe `fables new` starts a book from
  flows/          its own story flows, found before the built-in ones
  stories/        one folder per book, each with a story.md
```

Commands find a script's world by walking up to the nearest `world.toml`. Commands without a script, such as `sheets`, `rig`, `new`, `character` and `puppet`, use the world above the current folder, or the only world in `./worlds/`. `--world <folder>` picks one explicitly, `--root <folder>` says which folder holds `worlds/`, and a script can name its own with `world = "…"` in its front matter: a path relative to the script, such as `world = "../.."`, never an absolute one. Every build prints the folder it resolved as `world: <folder>`.

A world needs only a name to start, and `fables world new "Kerfuffle" --style cartoon --author "Ifiok Jr."` writes one for you:

```toml
name = "Kerfuffle"
author = "Ifiok Jr."        # optional: the author of its books, and the default for `fables new`
style = "cartoon"           # optional: the drawing style its folk are drawn in (default cartoon)
light = "golden"            # optional: the light every page starts in (default day)
age = "3-5"                 # optional: who its books are read aloud to (under-3, 3-5, 5-7)

[narrator]                  # optional: who reads every book aloud, unless a script names its own [voice]
name = "Daniel"
rate = 158

[book]                      # optional: the words in the front and back matter
note = "Psst! Wormsley the worm is hiding on every page. Can you find him?"
meet = ["Have you met the rest of **Kerfuffle**?"]
draw = ["**Draw your own Kerfuffle folk!**"]
```

`fables new` signs a new book with the `--author` you pass, else the world's `author` here. With
neither it stops and says to pass one or set the field. A book without an `author` in its own front
matter is also credited to the world's author.

`fables world new` writes `worlds/<slug>/` with that file, a `cast.toml` holding a commented example and nothing else, a `puppets/` folder and a `stories/` folder, then prints the next command. `fables character new <id> --name "…"` adds a cast member; `fables setting new <name>` starts a place.

## Story flows

A story flow is a shape a book can take: a laid-out recipe with a beat table, page and word budgets, and its pages already staged, so a new book starts from a sound page plan instead of a blank file. Nine are built into fables; `fables flow list` prints this table.

| Flow | What it is good for | Characters | Pages | Words |
| --- | --- | --- | --- | --- |
| `three-tries` | A want a child can hold and an ending you can already see, where the tries must earn it. | 1 | 13–17 | 300–500 |
| `pile-up` | A book built to be read again, where a child can join the chain and shout the last line. | a crowd | 9–13 | 250–450 |
| `there-and-back` | A search with a clear reason to leave, and a home the reader is glad to come back to. | 1 | 13–17 | 280–480 |
| `round-we-go` | A chain a child can follow, where the fun is seeing the next link coming. | 1 | 8–12 | 180–320 |
| `odd-couple` | A funny two-hander where the joke is character, and the ending is a swap rather than a lesson. | 2 | 13–17 | 280–480 |
| `wrong-end-of-the-stick` | A joke the pictures tell before the words do, where being wrong is funny and easily put right. | 2 | 11–13 | 260–450 |
| `goodnight-tour` | A bedtime book where the falling light and the falling word count do the work. | 1 | 11–15 | 200–380 |
| `gag-short` | A sixty-to-ninety-second cartoon where the timing is the story and the words are almost nothing. | 1 | 6–9 | 40–120 |
| `habit-tale` | A character-led book where the change is a choice the star makes, not a lesson the narrator gives. | 2 | 13–17 | 280–480 |

`fables new "<Title>" --star <id>` starts from the world's own `recipe.md` when it has one, and from `three-tries` when it has none. `--flow <name>` picks a flow instead, and `--friend <id>` names the second character for a flow that stages two or a crowd. A flow that needs a friend refuses without one, naming the flow and the world's cast.

```bash
fables new "The Whole Street Hums" --star mo --friend nan --flow pile-up
fables flow show pile-up
```

`fables flow list` also shows each flow's one-sentence about; `fables flow show <name>` prints its beats as a table, with the craft note for each beat. A world may add its own flows in its `flows/<name>.md`; they are found before the built-ins, so a world can replace `three-tries` with its own shape, and its `recipe.md` stays its default.

A flow file is a recipe: TOML front matter with a `[flow]` table, then the laid-out pages.

```text
+++
title = "{{title}}"
author = "{{author}}"
year = {{year}}
star = "{{star}}"
age = "3-5"              # the read-aloud band; a flow that suits a younger book says "under-3"

[flow]
name = "The Pile-Up"
about = "One sentence a parent would understand."
good-for = "When to pick this shape."
pages = [9, 13]          # the finished book's story pages
words = [250, 450]       # the finished book's words
needs = "many"           # 1, 2 or many

[[flow.beats]]
name = "First link"
pages = 1
words = [15, 25]
note = "Craft advice a first-time writer can act on in one sentence."
+++

# street day shot=wide
@{{star}} x=0.16 face=right happy
// 15–20 words: one actor, one small sound, and a lot of empty street.
[One small sound in an empty street.]
```

`pages` and `words` take one number or a `[min, max]` pair. `{{title}}`, `{{author}}`, `{{year}}` and `{{star}}` are filled in from the command; a flow whose `needs` is `2` or `many` writes `{{friend}}` where the second character goes. Every flow file names its `age` band; `fables new` writes `age = "3-5"` into a world recipe or a world flow that leaves one out, so every new book is read aloud at the same pace unless it says otherwise. A flow may also write `flow = "{{flow}}"` in its front matter; without it, `fables new` adds `flow = "<name>"` after the `star` line, so a book always remembers the shape it started from. The beats' page counts must add up to the pages below them, and every `[bracketed]` line is a line for the writer: `fables review` says plainly how many are still unwritten instead of critiquing them.

## Drawing styles

A drawing style is a folder in `styles/` that characters are drawn from, so a new look is data rather than code:

```text
styles/cartoon/
  style.toml      the finish (line weights, light, cel shadow), the face cylinder, where the features sit, how eyes are built, the kit's inks, the bodies (kid, grown-up), and the page skin that suits the look
  kit.svg         parts every character in the style starts with: hands, eyes, mouths, shoes
  template.svg    a plain kid's own parts, which new kids are drawn from
  template-grown-up.svg
                  a plain grown-up's own parts, cut for the grown-up body, which new grown-ups are drawn from
```

Every style has two bodies, `kid` and `grown-up`, and a template for each, so a grown-up is drawn with the style's adult proportions instead of a kid's torso, sleeves and legwear stretched over a taller skeleton.

| Command | What it does |
| --- | --- |
| `fables style list` | The drawing styles, with a sentence on each look, the bodies it draws (kid, grown-up) and the page skin it prints in. |
| `fables style check <name>` | Checks each body's template over the kit draws a whole figure whose parts join, and every body stands up. Joins are measured standing, cheering and sitting, front-on and three-quarters: the head or its neck must touch the torso, each arm must start inside the torso or its sleeve, each leg inside the torso or the legwear, and a sleeve or trouser leg drawn in two pieces must meet at the elbow or knee. |
| `fables style describe <name>` | The bodies the style draws and the template each starts from, every part a figure is drawn with (which are turning pairs, which are required), its coordinate space, and the rules for drawing parts. |
| `fables style sheet <name>` | `out/styles/<name>-model.png` (the template kid on a model sheet) and `<name>-parts.png` (every kit and template part). |

| Drawing style | Look |
| --- | --- |
| `cartoon` | Kerfuffle's look: round kids with big heads, brushed outlines that swell on the shadow side, one tone of cel shadow. |
| `anime` | Slender kids and grown-ups with the head about a quarter of their height; big eyes with a shaded iris, a pupil, a rim and two highlights; a tiny nose and mouth; thin even lines; hard two-tone cel shading. |
| `manga` | Anime anatomy (it shares the anime kit and template) printed in black ink on white paper: every colour becomes paper, a dot screentone or solid black, on every page, print file and video frame. A heavy tapered outer line and high-contrast eyes. Characters move on threes with modulation (fast moves on ones, long holds), trail speed lines on fast moves and flap their mouths, while the camera moves on every frame. |
| `manga-colour`, `manga-shojo`, `manga-shonen` | Manga in full anime colour, on the same kit and template. `manga-colour` keeps manga's heavy brushed line and motion on threes, with hard two-tone cel shading and a rim light. `manga-shojo` is delicate: thin self-coloured lines, very large sparkling eyes with long lashes, soft shading and gentle drifting motion on twos. `manga-shonen` is bold: the heaviest line, hard two-tone shadows, sharp narrow eyes, big wind-ups and speed lines. |
| `rubber-hose` | Early animated cartoons: round heads, pie-cut eyes, a button nose, white four-finger gloves, and arms and legs that are one bendy tube. A thick even black line, flat colour, no shading, big squash, stretch and overshoot, and an idle bounce on twos. `rubber-hose-1930s` is the same in soft blacks, greys and old paper white. |
| `clear-line` | Clear line: one even line weight everywhere, inside and out, with no hatching; flat vivid colour with no shading; simple characters with dot eyes, a short neck and gently tapering limbs with a knee and an elbow, for detailed settings that get their depth from layered planes; restrained, eased motion. Pages read best in the `clear` skin. |
| `mid-century` | Mid-century modern: angular, geometric characters on a wedge of a neck, with limbs cut as wedges and flat, pointed shoes; a small flat palette that every colour (settings included) snaps to; clean, even angular lines with the colour printed off them by one fixed slip, the same on every part; no shading; held poses that snap between keys on twos. Abstract settings suit it, in the riso skin. |
| `chibi` | Tiny characters about two heads tall: the anime face with enormous eyes, mitten hands, stubby limbs, soft self-coloured lines with a rim light, and springy, bouncy motion with a gentle idle bob. |

Every kid stands as tall on the page as a cartoon kid, and every grown-up as tall as the others, so a style's proportions change the drawing, not the page layout. Every style also names the **page skin** that suits it (`skin` in its `style.toml`): `clear-line` prints in `clear`, and the others print in `riso`. A book uses that skin unless its script or `--style` names another (see [Art styles](#art-styles)). [`worlds/kiteharbour/`](worlds/kiteharbour) is a small anime test world and [`worlds/hayate-street/`](worlds/hayate-street) a manga one, [`worlds/hoppington/`](worlds/hoppington) a rubber-hose one, [`worlds/tramline/`](worlds/tramline) a clear-line one with its own square, [`worlds/sundial-heights/`](worlds/sundial-heights) a mid-century one with an abstract plaza, and [`worlds/teacup-town/`](worlds/teacup-town) a chibi one; their stars, Hana, Ren, Clem, Zia, Otto and Mochi, were designed entirely through the MCP tools below.

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

The built-in drawing styles are compiled into fables, so a world anywhere on disk can name one. A drawing style is not a page skin: the page skins (`riso`, `tv`, `bold`, `cut-paper`, `crayon`, `storybook`, `clear`) print a page, and each drawing style names the skin that suits its look (`skin` in its `style.toml`).

## Settings

A setting is an SVG file of named parts on the square stage (2400 units, with 160 of overdraw past every edge). A part's id starts with its layer: `far-` (the sky and hills, which move least as the video camera travels), `near-` (behind the actors) or `front-` (in front of them). Parts are drawn through the page skin, so the same file is a riso print, a cartoon or crayon, and they say how they react to the scene. A page's `light=` grades every colour the scene paints exactly once (a part takes the full grade, losing a little more colour and contrast the higher its `data-depth`, while the characters in front take a gentler share), paints `fill="sky"` parts with the light's sky gradient, and washes a haze and a vignette over the art; the story text and captions keep their own colours.

| Attribute | Meaning |
| --- | --- |
| `fill` | A palette name or hex code; `tint` is the scene's tint; `sky` is the scene light's sky gradient (flat daylight paints the plain sky colour); `none` for a line, with `stroke` and `stroke-width` (`data-noodle` for a thick inked tube). |
| `data-line="none"` | A fill without an outline (skies, lawns, stripes). |
| `data-night`, `data-wilt` | The colour on a bare night (a page with no `light=`), and the colour it turns as the scene wilts. Parts without a night colour dim. Under a scene light the light alone makes the night: every part keeps its day colour and the light grades it. |
| `data-time` | `day` or `night`: only drawn then (the sun, the moon). Under a scene light the light's sky body decides: `dusk` shows the sun, `moonlit` the moon and stars, `overcast` and `storm` cover the sun with cloud, and an `indoor` or `candle` light shows neither. |
| `data-lit` | Comma-separated light presets (`dusk,night`): only drawn under one of those lights (lit windows at dusk, stars at night). |
| `data-gradient` | `top colour bottom colour`: a vertical gradient over the part (walls, water, lawns, floors). Both colours wilt, fade with distance, dim on a bare night and grade with the light like a flat fill; on a bare night a part with a `data-night` colour turns that flat colour, and under a scene light both stops keep the gradient. |
| `data-glow` | On a `<circle>`, `radius colour opacity`: a soft radial pool of light fading to transparent (a lamp, a window, the sun's bloom). A `light=` colours it but does not dim it, so a lamp still glows at night. |
| `data-depth` | 0 to 1: far things fade into the sky and lose line, and under a `light=` lose a little more colour and contrast. |
| `data-dots` | A riso dot screen: `colour spacing radius opacity`. |
| `data-drift`, `data-swing` | Clouds drift sideways; a swing swings from its pivot (`x y`). |
| `data-ambience` | Sound beds the place adds to the video (`birds,breeze`): `birds`, `breeze`, `waves`, `rain`, `wind`, `crickets`, `room`, `crowd`. The scene's light adds its own (`storm` brings rain and wind, `night` and `moonlit` bring crickets). |
| `data-indoors` | On the root: the place is inside, so night's crickets stay outside. |

`<circle data-sparkle="size colour">` twinkles, and `<circle data-critter="flower|tree" r="height" data-colour data-seed>` places one of the living plants, which have faces and wilt. The root `<svg>` sets `data-ground`, where actors stand.

The fifteen built-in settings (`plain`, `town`, `street`, `garden`, `park`, `bathroom`, `bedroom`, `kitchen`, `classroom`, `forest`, `beach`, `hilltop`, `shop`, `playroom`, `stage`) live in [`settings/`](settings) and every world can use them. A world adds its own, or repaints a built-in one by name, in its `settings/` folder; over MCP, `setting_create`, `setting_parts`, `setting_set_parts` and `setting_render` do the same.

**Composing for the stage.** Every page puts a text card and actors on top of a setting, so give each landmark a home neither covers (`fables sheets --zones` outlines these zones on every setting). In stage units, measured from the top: the built-in grounds sit at y 2060–2100; a typical top card covers x 150–2250 down to y ≈ 880 (a short line stops near 375, a long page near 970), and with `text=bottom` it covers y ≈ 1520–2280 (1470 for a long page). Heads reach 1000–1170 above the ground for a kid and 1160–1270 for a grown-up across the drawing styles, so they top out at y ≈ 800–1100. Actors without an `x=` stand at ½ alone, at ⅓ and ⅔ as a pair (x 800 and 1600, each covering about 480 across) and at ¼, ½ and ¾ as three; `x=far-left`, `left`, `center`, `right` and `far-right` are 0.12, 0.28, 0.5, 0.72 and 0.88. That leaves the wings (x below about 540 and above about 1860) from y 880 down to the ground for landmarks, a sun or a moon low in a wing at y ≈ 950–1120, the gutter between a pair (x 1040–1360) for something a lone actor may hide, and the card band for detail a story can lose. Keep the band behind the actors calm and mid-toned, with no straight edge across the heads.

## Props

A prop is a thing the story is about: a kite, a jar, a parcel. Each one is an SVG part file in [`props/`](props), found by name the way a setting is: the world's `props/<name>.svg` first, then the built-in one. `holds=<name>` puts one in a character's hand; `@kite x=0.7 y=sky` stands one on the stage as an actor.

A prop is drawn in its own coordinate space, and its root `<svg>` says how a hand holds it:

| Attribute | Meaning |
| --- | --- |
| `data-grip="x y"` | The point that sits in the fist. |
| `data-hands` | `one`, or `two` to bring both hands to the grip (a crate, a parcel). |
| `data-angle` | Degrees the prop leans in a relaxed hand. |
| `data-carry` | How somebody standing at ease carries it: `hand` (in the fist at the side, the default), `up` (held up beside the head, clear of the face: a torch, a flower), `front` (in both hands before the chest: a cake, a drum), `string` (the grip is a string's end and the prop floats above: a balloon) or `head` (worn: a hat). A pose that uses the hands (`hold`, `reach-out`, `wave`…) overrides it. |
| `data-held-scale` | A factor on the held-scale rule, for a prop that should be bigger or smaller in a hand than the rule makes it. |
| `data-about` | A sentence on what it is. |
| `data-use="brush"` | A toothbrush's behaviour: it points at the mouth when it comes near it, and scrubs it with foam. |

Each part is a `<path>` whose `id` is its name, drawn in file order. `fill` is a palette name or hex code; `data-shade="1"` gives the part the style's cel shadow, `data-line="detail"` or `"none"` changes its outline, and a line part has `fill="none"` with a `stroke`, a `stroke-width` and optionally `data-opacity`. Parts take the scene's light and the style's line like everything else. A prop grows with the hand that holds it, so a grown-up holds the same ball bigger than a kid, and it sits in the fist under the fingers at the grip. Small things are drawn larger than life in a hand, the way picture books draw them: the held-scale rule grows a key, a letter or a pebble towards the size of the hand that holds it, so a four-year-old sees what it is, and leaves things already that big as they are. However big a style draws its hands, a held thing's longest side is at least 1.7 times the fist's, so a rubber-hose glove never swallows a pebble.

The thirty-five built-in props are `apple`, `ball`, `balloon`, `basket`, `bell`, `book`, `box`, `bucket`, `cake`, `cup`, `drum`, `flower`, `hat`, `jam`, `jar`, `key`, `kite`, `kite-spool`, `ladder`, `lantern`, `leaf`, `letter`, `map`, `paintbrush`, `parcel`, `pebble`, `ruler`, `sandwich`, `spoon`, `star`, `teddy`, `telescope`, `toothbrush`, `torch` and `umbrella`. `fables sheets` writes `<world>-props.png` with each one alone and in the hand of the world's first figure. Over MCP, `prop_list`, `prop_create`, `prop_parts`, `prop_set_parts` and `prop_render` do the same as the setting tools.

A prop can stand on the stage as an actor, with `size=`, `z=`, `hide`/`show` and `enter=`/`exit=` like anyone:

```text
@kite x=0.7 y=sky tether=hana
```

At `size=1` a prop on the stage is the size a kid's hand holds it at, so a kite let go of is the kite that was held; `size=` scales from there. `y=sky` flies it over the cast's heads, low enough that a wide landscape shot, which keeps the ground in view, still shows it whole; on a book page one the card would cover moves beside the card, or below it, never under it and never over a face. In video the full landscape frame shows little sky above the heads, so stand a sky prop in the gap between the actors (or in a wing), not straight above one of them. A number (`y=0.3`, the prop's base that far down the stage) is never moved. A flyer (a bird) placed `y=sky`, or flying with no height given, keeps clear of the card the same way, and `land` puts it on the ground out from under the card.

`tether=<actor>` draws a string from the prop to that actor's hand, in the style's line, which hangs like string: it bows down from the straight line, more the longer and more level it runs (a string straight up hardly bows), and swings gently in video; `tether=nothing` lets it go. Giving and taking are two directions written together:

```text
@hana holds=nothing
@mo holds=kite-spool
```

In video a give plays as one: the two reach towards each other, the hands meet at the smaller one's chest, and the thing passes there. Two who stand too far apart for their hands to meet (a pair at ⅓ and ⅔ do) toss it: it flies underarm in a low arc from the giver's hand into the taker's. A string that changes hands is tied to the new hand when it gets there, so `@mo jump catch` with `@kite tether=mo` reads as a leap and a catch: the catching hand reaches up and out on the side of what it catches, clear of the face, and the string is caught at the top of the leap. Eyes on a stage prop (`see=kite`) go to the thing itself, a little above its middle, not to its tail or string.

## Designing characters with an agent (MCP)

`fables mcp` serves the studio over the Model Context Protocol on stdio, so an agent can design characters the way every Kerfuffle character was drawn: start from the style's template, redraw parts as SVG path data, look at the model sheet, fix, repeat. Sheets come back as images, so the model sees what it drew. Register it with any MCP client, for example:

```bash
claude mcp add fables -- fables mcp --root /path/to/your/studio
```

| Tool | What it does |
| --- | --- |
| `guide` | The director's guide ([`docs/directing.md`](docs/directing.md)), by topic: overview, story, staging, camera, light, acting, mixing, sound, workflow. |
| `world_list`, `world_create`, `world_describe` | The worlds under `<root>/worlds/`: start one from a name, a few sentences, a drawing style and an author, or read one back with its cast and books — and the other worlds whose characters a script's `guests` could invite. |
| `style_list`, `style_describe`, `style_sheet` | The drawing styles; every part a figure is drawn with, its coordinate space, the drawing rules and the page skin the style prints in; the kit as an image. |
| `character_create` | A new cast member from who they are and their colours, drawn from the style's template. `draw` also writes their part file. |
| `character_parts` | A character's parts as path data: their own, their style's template's, or the kit's. |
| `character_set_parts` | Writes or replaces parts and checks the figure. A character still drawn from the template is given a file of their own first. Changes that would break it are refused with the findings, so the world always loads. |
| `character_sheet` | The model sheet (five beats, front and three-quarters), the parts sheet, or all fifteen moods, as images. |
| `voice_providers`, `voice_list` | Which voice providers can speak now, and the offline macOS voices. |
| `voice_cast`, `voice_audition` | Give a character their voice, and hear a line in it, delivered in a mood, as audio. |
| `story_write`, `story_read`, `story_check` | Write a book's script into a world, checked against the cast (a script that doesn't rehearse is refused, with the line to fix), or read and check one. |
| `flow_list`, `flow_describe`, `story_start` | The story flows a world's books can start from and one flow's beats; and the whole `story.md` scaffold for a flow, with the star and friend staged, ready to fill in and `story_write`. |
| `story_review` | A director's critique of a book: findings with a severity, the script line and a suggestion. Advice, never a refusal. |
| `storyboard_render`, `storyboard_pin` | The storyboard as images and as a text shot list (page, script line, timing, shot size, subjects, move, entry, source and words); and the same plan pinned into the script as editable `@camera` directions marked `// pinned`, or removed again. |
| `animatic_render`, `video_frames` | An animatic that holds each shot for its real duration over the soundtrack, with each shot's caption and a progress bar, and the shot list; `silent: true` records no voice; and sampled frames of the video as one tiled image, offline. |
| `light_list`, `shot_list` | The light presets and the camera vocabulary (shot sizes, moves, arrivals, transitions), with a sentence on each. |
| `page_render`, `book_render` | One page as an image, or the proofs (with a contact sheet image), ePub and KDP print files under `<root>/out/`. |
| `video_render` | The narrated, lip-synced MP4, 9:16 or 16:9, or a quick preview. |
| `setting_list`, `setting_create` | A world's settings, built-in and its own; start one, blank or from a setting to repaint. |
| `setting_parts`, `setting_set_parts`, `setting_render` | Read and redraw a setting's parts as SVG elements, and see it by day or night, wilted or not. |
| `prop_list`, `prop_create` | A world's props, built-in and its own; start one, blank or from a prop to redraw. |
| `prop_parts`, `prop_set_parts`, `prop_render` | Read and redraw a prop's parts, its grip and its behaviour, and see the prop as an image. |

Worlds, books, settings and characters are named by slugs, never paths, so the server only reads and writes under its root: a name that is absolute or steps out with `..` is refused before any file is touched. A book written over MCP belongs to the world it is written into, so `story_write` refuses a script whose front matter names a `world`; at the command line, name one with `--world` or a relative `world =`. A person signs off every model sheet before a character is used in a book; `fables puppet sheet <id>` writes the same model sheet at full size.

## Commands

| Command | What it makes |
| --- | --- |
| `fables world new "<Name>" --style <drawing style> [--about "…"] [--author "…"]` | `worlds/<slug>/` with `world.toml`, a commented cast example, a `puppets/` folder and a `stories/` folder. |
| `fables character new <id> --name "…" [--kind kid\|grown-up] [--about "…"] [--skin … --hair-colour … --top … --trim … --bottom … --shoes … --eyes … --pattern …] [--draw]` | A new cast member, drawn from the world's drawing style's template; `--draw` also writes their part file. |
| `fables setting new <name> [--from <setting>] [--about "…"]` | A world's own setting, blank or repainted from one it already uses. |
| `fables new "<Title>" --star <id> [--friend <id>] [--flow <name>]` | A new book in the world's `stories/<title>/story.md`, laid out from the named flow, else the world's `recipe.md`, else `three-tries`, with the star already staged. |
| `fables flow list` | The story flows: each one's about, what it is good for, how many characters it needs and its length. |
| `fables flow show <name>` | One flow's beats as a table, with its page and word budgets and its craft notes. |
| `fables prop list` | Every prop the world can use, with its source, hands, grip and a sentence on it. |
| `fables prop new <name> [--from <prop>]` | A world's own `props/<name>.svg`, blank or copied from a prop it can use. |
| `fables check <story>` | Validates the script and reports the drawing style and page skin, page and word counts, and any cast fields a drawn character ignores. Errors give line numbers and list valid options. |
| `fables review <story>` | A director's critique of the rehearsed script ([`docs/directing.md`](docs/directing.md)): line-numbered findings and suggestions about staging, mood arcs, pacing, light and shots. Advice, never a refusal; ends with a count of findings. |
| `fables puppet parts <id> [part…]` | A character's parts as SVG path data: their own (or their style's template) and the kit's, each marked. |
| `fables puppet set <id> <part> --d "…"` / `--file <svg>` | Writes parts through the same checked operation as MCP `character_set_parts`; a change that would break the figure is refused with the findings. |
| `fables puppet template <id> [--from <id>]` | Starts the part file from the template (or another member's drawings) and adds the `puppet = …` line. |
| `fables style describe <drawing style>` | Every part a figure is drawn with, which are turning pairs, the coordinate space and the drawing rules. |
| `fables proofs <story>` | `out/<book>/<skin>/proofs/`: one PNG per page plus `contact-sheet.png`, in about a second. |
| `fables ebook <story>` | `out/<book>/<skin>/ebook/<book>.epub` (EPUB 3 fixed layout, passes epubcheck) and `cover.jpg` (2560 px store cover). |
| `fables print <story>` | `out/<book>/<skin>/print/`: the interior and cover PDFs, plus `kdp-setup.txt` with the numbers to type into KDP. |
| `fables video <story> [--format landscape\|portrait\|both] [--preview]` | `out/<book>/<skin>/video/`: MP4s, `soundtrack.wav`, `.srt` captions, a YouTube thumbnail and a 9:16 cover image. `--preview` renders at half size and 15 fps. |
| `fables storyboard <story> [--format landscape\|portrait] [--pin [page=3,5]] [--unpin]` | `out/<book>/<skin>/storyboard/storyboard.png` (paged as `storyboard-2.png`, … past 24 panels), plus `storyboard.md`: the shot list, one row per shot with its page, script line, start and duration, shot size, subjects, move, how it is entered, who chose it, the music cue that comes in, any dead stop (`@hold`, `hold=`) and the words spoken. The board prints the list too. `--pin` writes the director's plan for the `--format` shape (landscape unless you say) into the script as `@camera` directions marked `// pinned` for you to edit, delete or reorder, and names any pages where the other shape plans other shots; `--unpin` takes them out again. Never records a voice, so it runs offline in a second or two. |
| `fables animatic <story> [--format landscape\|portrait] [--silent]` | `out/<book>/<skin>/animatic/<book>-animatic.mp4`: each storyboard panel held for its real duration over the soundtrack, captioned with its number, page, line, start, length and first words, under a progress bar that marks each cut, so timing reads in seconds; a push or pan moves by cropping one wide plate. Half size at 24 fps, with `soundtrack.wav` beside it, mixed exactly as the video's. A line whose voice cannot speak here (no `say`, no API key, no command) is silent and timed by its estimate while music, effects and ambience play, and the output says how many; `--silent` does that for every line and records nothing. `fables video` never falls back: it stops with the voice's error. |
| `fables book <story>` | Proofs, ePub and print. |
| `fables all <story>` | Everything. |
| `fables sheets [--moods <id>] [--zones]` | Model sheets in `out/sheets/<skin>/`: the whole cast, every setting, the world's props (`<world>-props.png`: each prop alone and in the first figure's hand), the world's colour script (`<world>-lights.png`: one setting with one cast member under every light), and one character in every mood. `--zones` outlines, on every setting, where a typical text card sits at the top and with `text=bottom`, how high a kid's and a grown-up's heads reach, and the spots actors stand on without an `x=` (see [Settings](#settings)). |
| `fables styles <story>` | `out/<book>/styles.png`: the cover and three pages drawn in every page skin, side by side. |
| `fables rig <id>` | `out/rig/<skin>/`: a rig sheet (every stance and gait phase with the skeleton drawn over the character), a motion-test video, and an acting-test video (the ten acting rules, in order, on two characters). |

Every command that takes `<story>` also takes a folder of books or a world: `fables all worlds/kerfuffle` builds every `story.md` in it.
Every command also takes `--style`, the **page skin**, so any render can print in any skin without editing the script. (For drawing styles such as `chibi` or `clear-line`, run `fables style list`.)

- `--style storybook` prints in one skin;
- `--style bold,tv` prints in each listed skin;
- `--style all` prints in all seven.

A book's skin is, in order: `--style` on the command line, `style` in the script's front matter, the page skin its world's drawing style names (`skin` in `styles/<style>/style.toml`), then `riso`. Without `--style` and a front-matter `style`, a chibi world prints in `riso` and a clear-line one in `clear`. Each skin renders to its own folder, `out/<book>/<skin>/`, so skins sit side by side and never overwrite each other. The voice cache is shared, so re-rendering a video in another skin records nothing new.

Install with `cargo install --path .`, or use `cargo run --release --` in place of `fables`.

### Requirements

- **Rust**, the pinned nightly in [`rust-toolchain.toml`](rust-toolchain.toml): `rustfmt.toml` uses
  unstable options. `rustup` installs it, with `rustfmt` and `clippy`, the first time a command
  runs in the checkout. The Nix shell in [`devenv.nix`](devenv.nix) uses the same rustup
  toolchain, so it builds with the same compiler.
- **ffmpeg** with the `libx264` encoder, for video. Homebrew's, Debian's and Ubuntu's builds have
  it; Fedora's `ffmpeg-free` does not, and fables stops with a message naming the missing encoder
  before it draws a frame. Beyond that it uses only ffmpeg's own `aac` encoder and filters built
  into every ffmpeg (`zoompan`, `overlay`, `concat`, `color`, `format`, `setsar`, `loudnorm`), so a
  build without freetype (`drawtext`) is fine.
- **A narrator.** macOS `say` by default, or any TTS command (see [Voices](#voices)).

> On macOS, if linking fails with "You have not agreed to the Xcode license agreements", run `sudo xcodebuild -license` once. You can also use the Nix shell from [`devenv.nix`](devenv.nix): `devenv shell`.

## Writing a script

A script is TOML front matter between `+++` lines, then pages. Each page starts with `# setting`. `title` and `author` are required, and the title needs at least one letter or digit: its slug names the output folder. Below the heading, narration paragraphs and `@` stage directions interleave in reading order.

```text
+++
title = "Stinky McStinkbreath"
author = "Ifiok Jr."
star = "stinky"                 # who goes on the cover
age = "3-5"                     # who it is read aloud to: under-3, 3-5 or 5-7
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

`age` sets how the book is read aloud: the narration rate (about 125, 135 and 150 words a minute for `under-3`, `3-5` and `5-7`), the gaps between lines and the tail after a page, and how long a page in a new place is seen before it is talked over. A world sets a default in its `world.toml`, a book's own `age` wins, and a narrator with an explicit `rate` keeps it. A book with no `age` anywhere keeps the older, brisker timing (165 words a minute), so its video does not change; `fables check` suggests one.

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
| `// note` | A comment, ignored. It may also end a stage direction, as a pinned line does: `@camera shot=wide move=push after=7.19 // pinned`. |

### Page headings

`# setting option option…`

| Option | Meaning |
| --- | --- |
| Setting | `plain`, `town`, `street`, `garden`, `park`, `bathroom`, `bedroom`, `kitchen`, `classroom`, `forest`, `beach`, `hilltop`, `shop`, `playroom`, `stage`, or one of the world's own (see Settings). |
| `night`, `wilt=1`, `stink=0.5`, `mint=1`, `tint=sunflower` | Starting weather (`tint` colours the `plain` blob). |
| `light=golden`, `golden` | The scene's light: `day` (the default), `dawn`, `golden`, `dusk`, `night`, `moonlit`, `overcast`, `storm`, `indoor` or `candle`. A bare preset word means `light=<preset>` in a heading (`# street golden`) or an `@scene` direction (`@scene dusk`). It grades every colour in the scene once, paints the sky, aims the shadows and adds haze and a vignette. Every colour stays recognisably itself: a hue turns only a little towards the light, lines stay ink, skin keeps its warmth in every light, and the characters sit a little forward of the setting. A world can set a default in `world.toml`, and a book can override it with `light = "…"` in its front matter. Bare `night` and `day` keep their weather meaning exactly: `night` turns the sky night without naming a light, so the book's default light still applies. Under a `light=` the light alone makes the night: parts keep their day colour and the light grades it, so `data-night` colours are for bare nights. |
| `speed=1`, `impact=1` | Manga effects: focus lines rushing in towards the first person on stage, or a jagged burst behind them. They work in every style. |
| `text=top`, `text=bottom`, `text=none` | Where the story text card goes. |
| `size=0.9` | Scales the page's text. |
| `zoom=1.5 look=0.5,0.6` | A close-up on the point (x, y), as fractions of the page. |
| `shot=… on=…` | A shot size that frames the still illustration: `wide`, `full`, `medium`, `close` or `extreme-close` on the actors named by `on=` (comma-separated ids; empty means everyone on stage, or whoever speaks for a close shot). Framing is measured from what the puppets actually draw — hair, raised arms, held props and all — with headroom and lead room, so a page can be a close-up without typing coordinates. A still is framed into the part of the page its text card leaves clear, falling back to a tighter shot when a long card leaves no room, so a head is never hidden under the words. |
| `transition=wipe` | How the page is entered in the video: `wipe` (the paper sheet), `cut`, `dissolve`, `fade`, `iris` or `push`. `[video] transition = "…"` sets a book's default. |
| `music=calm` | The page's music cue: `play`, `calm`, `tense`, `sad`, `sleepy`, `bright` or `none`. Without one, its `light=` picks a cue. |
| `hold=1.2` | A silent hold at the start of the page, before its first line, for a reveal. |
| `video=skip`, `book=skip` | Leave the page out of one format. |
| `no-wormsley` | Don't hide Wormsley on this page. |

### Stage directions

`@actor word word key=value…`

**Actors** are cast ids from the world's `cast.toml` (`@stinky`), copies of them (`@stinky.monday`), generic critters: `bird`, `dog`, `worm`, `flower`, `tree`, `toothbrush` (`@bird.a`, `@flower.3`), or a prop's name, which stands that prop on the stage (`@kite`). `@scene …` changes the weather mid-page (`wilt=`, `stink=`, a `light=` or a bare preset such as `@scene dusk`, `music=…`, `ambience=…`, and friends; the video eases the change over about a second), and `@camera …` the framing. `@music <cue>` changes the cue from that line on, and `@hold <seconds>` is a dead stop (below).

| Kind | Values |
| --- | --- |
| Moods | `happy` `grin` `laugh` `sad` `grumpy` `cross` `defiant` `shocked` `disgusted` `fainted` `dizzy` `sleepy` `dreamy` `worried` `proud` |
| Poses | `idle` `wave` `point` `arms-crossed` `hips` `cheer` `hold-nose` `cover-mouth` `brush` `shrug` `droop` `hold` `think` `reach-out` `pick` `catch` |
| Stances (legs and spine) | `stand` `crouch` `sit` `kneel` `jump` `bow` `tiptoe` (in video a `jump` is a leap that lands) |
| Gaits | `walk` `run` (cycles on the spot in video) `still` |
| Actions | `talk` (mouth moves with the next paragraph) `faint` `wake` `fly` `land` `wag` `calm` `hop` `flip` `hide` `show` `notice` `quiet` |
| Place | `x=left` (`far-left`, `center`, `right`, `far-right`, or `0`–`1`), `y=sky` (a flyer high over the cast, a prop flying over their heads; on a book page, clear of the text card), `y=0.3` (a bird's body, or a prop's base, that far down the stage: 0 is the top, 1 the bottom; a number is never moved), `size=1.4`, `z=5` (draw order), `face=left`/`right`/`front` |
| Character | `teeth=clean`/`grotty`/`sparkly`, `stink=0`–`1`, `mint=0`–`1`, `holds=<prop>`/`nothing` (any world or built-in prop, or empty hands), `mouth=0`–`1`, `colour=teal`, `wilt=0`–`1` (flowers), `tether=<actor>`/`nothing` (a stage prop's string, or letting it go) |
| Eyes | `see=<actor>`/`<prop>`/`mine`/`up`/`down`/`left`/`right`/`front`/`away` (`mine` is what the actor holds): it holds until it is changed, `front` releases it, and the eyes lead the head. `notice` plays a small beat first: the eyes flick to the `see=` target (or whoever spoke last), hold for about half a second, and then the rest of the direction lands. |
| Entrances | `enter=left`/`right`, `exit=left`/`right` (figures walk on and off in video, critters keep their own entrances; in the book they just appear or vanish) |
| Scene | `light=golden` or a bare preset (`dusk`), `wilt=1`, `stink=1`, `mint=1`, `tint=sunflower`, `music=calm`, `ambience=birds` (or `none`), `speed=1`, `impact=1` |
| Directions (no actor) | `@pause 1` `@hold 1.5` `@music calm` `@sfx pop` |
| Effects | `pop` `plop` `bonk` `boing` `whoosh` `sparkle` `tweet` `blorp` `page` `sting` `tada` `twinkle` `thud` `splash` `gasp-whoosh` `ding` `drum-roll` `heartbeat` |
| Camera | `shot=wide`/`full`/`medium`/`close`/`extreme-close` framed on `on=<actor>,<actor>` (empty: everyone, or the speaker for a close), `move=push`/`pull`/`pan-left`/`pan-right`/`tilt-up`/`tilt-down`/`track` taking `over=<seconds>` (default the rest of the shot), `shake=0`–`1`, `after=<seconds>` to land the direction that long after the page opens (a pinned slow push starts inside a long hold), and `cut` or `ease` to choose how the camera arrives (a change of shot cuts unless the direction says `ease`). The older `zoom=` and `look=` still work and win when given. |

### Who points the camera

`[video] direction` chooses between two directors:

- `"auto"` (the default): pages without any `@camera` direction are shot by film rules. Every page opens on an establishing wide or full shot long enough to read; whoever speaks gets a medium (landscape) or close (portrait); a two-way exchange alternates shot and reverse shot; very short lines share a two-shot; a strong mood on somebody who isn't speaking earns a close reaction shot; entrances, exits, walks, runs, hops and faints widen to a full shot so the action stays in frame. No shot runs shorter than 1.2 s, cuts land at most one per 1.2 s and never just after a line begins, and a framing held longer than 8 s gets a slow push.
- `"locked"`: the older camera, kept byte for byte — one auto-framed view per scene key and line, gliding between them.

`[video] acting = "plain"` turns the acting layer off and reproduces the plain motion; every style's `[motion]` dials tune it, and [`docs/animation.md`](docs/animation.md) has the whole story.

`[video]` sets how the book is performed:

| Option | Meaning |
| --- | --- |
| `cast-voices = true` | Quoted words are spoken by the character whose `talk` direction introduced them; the narrator reads the rest. |
| `drop-tags = true` | With cast voices, skip short tags like "said Mum." so the scene plays like a show. |
| `direction = "auto"` | `auto` shots pages by film rules; `locked` is the older always-gliding camera. |
| `acting = "full"` | `full` performs by the acting rules; `plain` turns the layer off. |
| `depth-of-field = false` | Stop softening the setting behind close shots. |
| `transition = "wipe"` | How pages are entered unless the heading says otherwise. |
| `music = "auto"` | `auto` lets each page's `light=` pick the cue; a cue name (`play`, `calm`, `tense`, `sad`, `sleepy`, `bright`, `none`) is the book's default; `play` holds the old loop throughout. |
| `ambience = true` | Procedural sound beds from each setting and light. On already for a book with an `age`. |

A page with any `@camera` direction is shot exactly as written, in both modes' vocabulary. Setups on directed pages follow the same 1.2 s floor: one that would flash by is merged into its neighbour, but a cut the script wrote is honoured — the storyboard marks the hold it leaves under 1.2 s with `· short`. Every shot frames its subject's whole head, hair and all — measured from the puppet's own drawing, so a tall hairdo, a raised hand or a held prop is never guessed at — with headroom above and room beside it (room enough to survive the slow push a long hold rides in on); only an extreme close-up may crop hair above the brow line, and lead room never pushes a head out of frame. Close and extreme-close shots soften the setting behind their subject (depth of field); `[video] depth-of-field = false` turns that off.

**The storyboard is where a cartoon is fixed.** `fables storyboard` draws one panel per shot, with a move drawn as its end framing and an arrow rather than split into two near-identical panels, and prints the shot list. `--pin` writes the automatic director's shots into the script as `@camera` directions marked `// pinned`, so you can edit, delete or reorder them; rendering the pinned script reproduces the plan, and `--unpin` removes exactly those lines. `fables animatic` then holds each panel for its real duration over the soundtrack: judge the timing there, in seconds, and render the full video only once it is right.

Teeth, stink and mint **carry over from page to page** until changed, so grotty teeth stay grotty. Actors without an `x` are spaced out automatically.

**Timing and sound** (video only):

- `@pause 0.4` adds a beat of silence, of at most 60 seconds; `@hold 1.2` is a dead stop, where the voice, the music, the ambience and the performance's idle movement all stop. `hold=1.2` on a heading does the same at the start of a page. Both take 0 to 60 seconds and count towards the video's length.
- `@sfx name` plays a built-in effect: `pop`, `plop`, `bonk`, `boing`, `whoosh`, `sparkle`, `tweet`, `blorp`, `page`, `sting`, `tada`, `twinkle`, `thud`, `splash`, `gasp-whoosh`, `ding`, `drum-roll`, `heartbeat`. An unknown name is an error listing these.
- Fainting bonks, hops boing and entrances pop automatically. In a book with an `age`, an action's sound lands on its contact frame plus about three frames, never before it.
- A video runs at most 30 minutes (1800 seconds); a longer script is refused before any audio or frame is made.
- The music follows the story. A page's `light=` picks the cue (`storm` and `overcast` → `tense`, `dusk` and `golden` → `calm`, `night`, `moonlit` and `candle` → `sleepy`, otherwise `play`); `music=` on a heading, `@music <cue>` mid-page or `[video] music = "<cue>"` says otherwise, and `[video] music = "auto"` is each page's light. Cues cross-fade over about a second.
- A `@sfx` effect is normalised to the same loudness as the rest, so a new one never jumps out of the mix.
- When a `**SHOUTED**` line is spoken by a character whose mood has just turned cross, defiant, shocked or laughing, the music drops out half a second before the line and returns after it.
- Ambience comes from the place: a setting's `data-ambience` and its light's own beds (`storm` adds rain and wind; `night` and `moonlit` add crickets outdoors). It sits about 25 dB under the narration and ducks like the music. A book with an `age` gets it; a book with neither an `age` nor `[video] ambience = true` keeps the soundtrack it had.
- A page in a new place is seen for at least a second and a half before the first line is spoken, so the place lands before it is talked over; the age band scales it.


### Guests

A book can invite characters from other worlds under the same root, and each keeps looking like themselves:

```text
guests = ["kiteharbour/hana", "kerfuffle/stinky"]   # front matter
guest-style = "host"                                 # optional
```

A guest is directed by its id (`@hana`), or by `world.id` (`@kiteharbour.hana`) when its id clashes with a local cast member — then the bare id means the local one. A guest keeps their puppet parts, their own style's construction (bodies, features, eyes, kit) and their voice, and by default their own style's line and shading as well, so a cartoon kid and an anime kid can share a page looking like themselves. `guest-style = "host"` inks and shades guests with the host world's `[line]` and `[shade]` dials instead; their construction never changes. Everyone on a page moves with the host story's `[motion]`, and guests stand at the height their own world gives them. A missing world or character is an error naming the ones that do exist, and the "meet the folk" page still lists only the host world's cast.

### New characters

Add a table to the world's cast file, such as [`worlds/kerfuffle/cast.toml`](worlds/kerfuffle/cast.toml), built from puppet parts, then run `fables sheets` to see them. Or let the command line write the table:

```bash
fables character new grubby --name "Grubby McGrubhands" --about "Never washes his hands." \
  --skin teal --hair-colour sunflower --top tomato --trim sunflower --bottom night \
  --shoes cocoa --eyes cocoa --pattern star
```

```toml
[grubby]
name = "Grubby McGrubhands"
born = "Gus"
habit = "Never washes his hands, so everything he touches turns to mud."
kind = "kid"               # kid or grown-up
skin = "teal"
hair-colour = "sunflower"
top = "tomato"
pattern = "star"           # plain stripes dots star
trim = "sunflower"
legwear = "trousers"       # shorts trousers skirt
bottom = "night"
shoes = "cocoa"
extras = ["muddy-hands"]   # freckles glasses moustache apron post-cap satchel bow muddy-hands bogey lashes
eyes = "cocoa"             # iris colour

[grubby.voice]             # his own voice for dialogue (see Voices)
name = "Eddy (English (UK))"
style = "A friendly, slightly mumbly six-year-old boy."
```

A person with **no `puppet` line** is drawn from their world's drawing style's template for their body, in their own colours, exactly as if `fables puppet template` had written the file and it had never been touched. That is how a new character is ready in one command.

- `fables puppet template <id>` writes `puppets/<id>.svg` from the template and adds the `puppet = "puppets/<id>.svg"` line itself, so a part can be redrawn with `fables puppet set <id> <part> --d "M…"`.
- `fables character new <id> --name "…" --draw` writes both the table and the part file in one step.
- `puppet = "parametric"` draws the older parametric puppet instead, the one that reads `hair`, `head`, `body`, `nose` and `mouth` (`mouth_size` in code). Those five fields do nothing on a character drawn from parts, and `fables check` says so once per member.

A person is `kind = "kid"` (the default) or `kind = "grown-up"`, and the kind picks the body they stand on and the template they are drawn from. Every drawing style has a kid template and a grown-up one cut for adult proportions, so a grown-up with no `puppet` line, or one started with `puppet template` or `character new --draw`, is a grown-up of that style rather than a kid stretched tall. `fables puppet check <id>` fails a figure whose parts come apart on its body, naming the part, the gap and the pose, and `fables puppet set` refuses a change that opens a join.

Colours are palette names (see the bible) or `#rrggbb`. A character with a `habit` appears on the "Have you met the rest of Kerfuffle?" page in every other book. The full command-line vocabulary for a character is `fables puppet parts <id>` (their parts as path data) and `fables style describe <drawing style>` (every part, its space and the drawing rules).

## Art styles

**A page skin is how the page prints.** Choose one per book with `style = "..."` in the front matter, or render in any others with `--style` (one, a comma list, or `all`). `fables styles <story>` draws the same pages in every skin so you can compare. A book that names neither uses the page skin its world's **drawing style** recommends (`skin` in `styles/<style>/style.toml`: `clear` for clear-line, `riso` for the others).

| Page skin | Look |
| --- | --- |
| `riso` (default) | Two-colour riso print: flat inks, a wobbly line, fills off register, dot screens |
| `tv` | Modern preschool TV: no outlines, clean shapes, soft gradient shading |
| `bold` | Classic cartoon: heavy outlines, hard cel shadows, glossy highlights |
| `cut-paper` | Collage: paper fibres, torn edges, each shape casting a shadow |
| `crayon` | Waxy scribbled colour that strays over the lines |
| `storybook` | Pencil lines over soft watercolour washes on textured paper |
| `clear` | One even outline everywhere, flat colour, no shading or texture (for the clear-line drawing style) |

The drawing style decides how a figure is built (chibi, clear-line, rubber-hose…); the page skin decides how every page prints. `fables check` and every render say which is which: `drawing: chibi · skin: riso`.

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

The script's mood (`cross`, `defiant`, `sad`…) becomes the line's delivery, and `**shouted**` words are shouted. Mouths lip-sync to whoever is speaking; everyone else looks at them unless a `see=` holds their eyes, and a `notice` makes them look before they react.

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

Takes are cached in `out/<book>/cache/voice`, keyed by voice, mood and text, so only edited lines are re-recorded. Paid API calls fail once and are never retried in a loop. Give the run the provider's key in its environment, for one batch: `ELEVENLABS_API_KEY=<key> fables all worlds/kerfuffle`.

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
  flow.rs         story flows: the `[flow]` table, the built-in recipes and starting a book
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
worlds/           one folder per series: world.toml, bible, cast, puppets, settings, recipe, flows and books
settings/         the built-in settings, as part files
props/            the built-in props, as part files
styles/           drawing styles: style.toml, kit and template per look
flows/            the built-in story flows, one laid-out recipe per shape
docs/             the five-minute guide, voices and turning the books into a show
assets/fonts/     Andika and Lilita One (SIL Open Font License)
```

## Licence

The code is released into the public domain under the [Unlicense](LICENSE). The Kerfuffle series'
story text, character names and designs, the series bible and the puppet drawings under
[`worlds/kerfuffle/`](worlds/kerfuffle) are copyright Ifiok Jr., all rights reserved; they are
included so the tool can be seen working on a real series, and
[`worlds/kerfuffle/LICENSE`](worlds/kerfuffle/LICENSE) says what that means. Every other world under
[`worlds/`](worlds) is an example released with the code. The fonts in [`assets/fonts/`](assets/fonts)
are under the SIL Open Font License, with their licence files beside them.

## Known limits

- **Shapes.** The page is square (8.5" × 8.5") and spreads are single pages. A landscape trim or two-page spreads would need new view boxes in `book.rs` and `press/print.rs`.
- **Voices.** The ElevenLabs and OpenAI providers follow their current API docs and are unit-tested on request and response shapes, but haven't yet been run against the live services. Recorded human takes and speech-to-speech conversion aren't built yet (see `docs/tv-show.md`).
- **Read-along ePub.** Not built yet. The narration timings exist, so EPUB media overlays (read-along highlighting in Apple Books) are a natural next step.
- **New habits need new effects.** Effects live in `src/art/fx.rs`, so a book about a new habit (smouldering trousers, say) means a few dozen lines of drawing code.
