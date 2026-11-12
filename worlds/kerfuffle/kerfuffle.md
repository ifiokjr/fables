# Tales from Kerfuffle: series bible

> **In the town of Kerfuffle, whatever you do shows on the outside.**

Every book in the series opens with that line. It is the whole world in one sentence.

## The idea

Kerfuffle is a small, crooked, brightly painted town where habits have visible, literal, slapstick consequences.

- Skip brushing and your breath becomes a green cloud that knocks birds out of the sky.
- Never wash your hands and everything you touch turns brown and gloopy.
- Tell a fib and your trousers start to smoulder.

And whatever shows on the outside **becomes your name**. Nobody in Kerfuffle keeps their birth name for long:

- Stanley who won't brush becomes **Stinky McStinkbreath**.
- Bonnie who picks her nose becomes **Bogey McBogeyface**.

The names are the hook, a schoolyard cousin of Boaty McBoatface. The rule underneath is the twist: **names can change back**. Fix the habit, and the town gives you a new name. Stinky McStinkbreath ends his book as **Minty McFreshbreath**.

That turns every book into a before-and-after with a title-page punchline. Children get two things:

- a name to shout;
- a reason to want the second one.

## The rules of the world

1. **Habits show.** Every consequence is visible, physical and funny. Never use a lecture where a picture would do.
2. **Consequences are slapstick, never cruel.** Things faint, wilt, droop, smoulder and gloop. Nothing is hurt for good. Every knocked-out bird wakes up by the last page.
3. **Your name is what you do.** The town names you, and the town renames you when you change.
4. **Kids fix it themselves.** Grown-ups ask, nag and sigh, but the turn always comes from the child's own choice, made alone, often at night.
5. **The fix is small and doable tonight.** Two minutes of brushing, one wash of the hands. Readers must be able to copy the turn at bedtime.
6. **The good habit is gorgeous.** The reward is the most beautiful spread in the book: mint mist, sparkle, bloom. Doing the right thing should look like magic.
7. **Wormsley hides on every page.** A worm in a bowler hat, somewhere on every page of every book. The tool hides him automatically.
8. **Folk come in every colour of the paint box.** Kerfuffle folk have tangerine, teal and bubblegum skin, so every child can see themselves without any book being about skin.

## The Kerfuffle recipe

Every tale follows the same shape, so a new book is a new habit poured into a known mould. The page counts are a guide for a 32-page paperback.

| Beat | Pages | What happens | In *Stinky* |
| --- | --- | --- | --- |
| The town | 1–2 | "In the town of Kerfuffle, whatever you do shows on the outside." Cameos from other folk showing their habits. | Bogey picks; Grubby is muddy. |
| The kid | 1 | Meet them by their birth name, being lovable. | Stanley, jam jar, bouncing. |
| NOPE | 2 | A grown-up asks. The kid refuses, twice, with a catchphrase. | "**NOPE**." |
| It shows | 3–4 | The habit becomes visible and escalates in threes. | Teeth go yellow, green, nameless. |
| Rule of three | 3 | Three consequences, each bigger than the last. | Flowers wilt, birds drop, the postman faints. |
| The name | 1 | The town gives them their new McName. | **Stinky McStinkbreath** |
| The cost | 2–3 | Friends won't play. A low point: gentle, a little funny, never mean. | Hide-and-seek by smell. A pigeon faints. |
| The choice | 2 | Alone, the kid decides to try, just once, on their own terms. | "Fine. Just *one* brush." |
| The magic | 1–2 | Doing the good thing, drawn as the most beautiful moment in the book. | **WHOOSH!** Minty mist. |
| Callback | 3 | The same three consequences, reversed. | Flowers bloom, birds sing, friends run over. |
| The new name | 1 | The town renames them. | **Minty McFreshbreath** |
| The button | 1 | A last joke that calls back to the NOPE. | "**ALREADY DONE!**" |

## The cast

The look of every character lives in [`cast.toml`](cast.toml). Run `fables sheets` to see the cast and every setting drawn from it.

| Character | Born | Habit, and what it does | Look |
| --- | --- | --- | --- |
| **Stinky McStinkbreath** → **Minty McFreshbreath** | Stanley | Won't brush. His breath knocks out anything he says hello to. | Tangerine, plum tuft, teal stripes, the biggest mouth in town. |
| **Bogey McBogeyface** | Bonnie | Picks her nose and wipes it on whatever is nearest. | Sunflower, pigtails, a bow, a little green drip. |
| **Grubby McGrubhands** | Gus | Never washes his hands. Everything he touches turns to mud. | Teal, yellow curls, a star jumper, muddy mitts. |
| **Shouty McShoutface** | Sasha | No indoor voice. Windows rattle and hats blow off. | Tomato, spiky hair, a wide mouth. |
| **Fibby McFibberpants** | Felix | Tells fibs, and his trousers get warmer with each one. | Grape, tangerine bob, glasses, red trousers. |
| **Mum** and **Dad** | | Stanley's parents. Mum asks nicely three times, then uses his whole name. Dad hums while he brushes. | Bubblegum with a bun; cornflower with a moustache. |
| **Parcel McParcelface** | | The postie. Delivers everything, even things nobody ordered. | Lilac, post cap, satchel. |
| **Biscuit** | | The town sausage dog. Faints at the slightest thing, very dramatically. | Toffee. |
| **Wormsley** | | A worm in a bowler hat. Hides on every page. Nobody knows why. | Pink, bowler, round glasses. |

### Grown-ups and names

Grown-ups have usually grown out of their habit names, which is why they are just "Mum" and "Dad". Some, like Parcel McParcelface, have a name for what they *love* doing rather than a bad habit. That is the series' quiet promise: names can be good, too.

## Places

Every book reuses the same small town, so readers learn its geography. Settings are named in scripts with `# setting`.

| Setting | What's there | Good for |
| --- | --- | --- |
| `town` | Crooked painted houses up the hill to the leaning clock tower, a sandy path; lit windows and a glowing clock after dark. | Openers, name reveals, endings. |
| `street` | Big house fronts, bunting, a red postbox, a lamp post, hedges, and a side street off to the clock tower. | Meeting grown-ups, the postie. |
| `garden` | A picket fence, a bed of flowers with faces, a striped lawn and the neighbours' roofs. | Wilting and blooming (`wilt=`). |
| `park` | Lollipop trees, swings and a slide, a lamp, and fireflies at dusk. | Friends, playing, birds. |
| `bathroom` | Tiles, a bubbly bath with a duck, the round mirror over the sink, the toothbrush cup, and a moon night light. | Habits at home. |
| `bedroom` | Striped wallpaper, a window (with a moon at `night`), a bed, a lamp, a rug, a rocket on the shelf. | Bedtime, mornings. |
| `kitchen` | Cupboards, a cooker with a steaming pan, a window over the sink, the fridge with drawings on it. | Breakfast, tea, grown-ups at home. |
| `classroom` | A chalkboard of doodles, bookshelves, the teacher's desk, a window. | School, show and tell. |
| `forest` | Tall trunks fading into the trees, a winding path, mushrooms that glow and an owl at night. | Adventures, getting lost. |
| `beach` | The sea to the horizon, a lighthouse, a striped umbrella, a sandcastle. | Days out, holidays. |
| `hilltop` | A long view of fields, a river and blue mountains, a lone tree, the Milky Way at night. | Big moments, looking far. |
| `shop` | Shelves of jars, the counter and till, a window onto the street. | Errands, buying sweets. |
| `playroom` | A play tent, a toy box, bunting and a rainbow mat floor. | Playing indoors, sharing. |
| `stage` | Red curtains, footlights, a painted backdrop and a spotlight. | Shows, being brave, the finale. |
| `plain` | Paper with one big colour blob (`tint=`). | Close-ups, montages, the end. |

Scene weather works everywhere: `wilt` (plants droop), `stink` (green haze), `mint` (icy freshness) and `night`.

## Visual style

**Riso-print folk art.** Flat spot colours, a slightly wobbly aubergine ink line, and fills that sit a few units off their outlines, as if two print passes didn't quite line up. Paper grain over everything, and halftone dots for skies and shading. The look is meant to feel printed and hand-made, never glossy or 3D.

- **Ink set.** A fixed palette of named colours in `src/art/palette.rs`: paper, ink, tangerine, sunflower, bubblegum, teal, mint, cornflower, grape, moss and friends. Cast files use the names, so the whole series stays in one set of inks.
- **Stink and mint use one shape.** The stink cloud and the mint mist are the same billowing shape in opposite inks: moss and bog with flies, then ice and mint with sparkles. The turn of the book is the cloud changing colour.
- **The puppet.** Kerfuffle folk are cut-out puppets, the way TV cartoons are built:
  - a big head (round, egg or wide) on a small body (bell, bean, pear or round), so every character has its own silhouette;
  - eyes with coloured irises and real lids;
  - tapered brows, a button, round or long nose, and a nine-shape mouth chart for lip-sync;
  - tapered noodle arms with cuffs, bending like rubber when the hand comes close;
  - mitten hands with thumbs that open, make fists and point;
  - shoes with soles, a collar, one tone of shadow for volume, and a soft contact shadow so nobody floats.

  There are 15 moods and 15 poses (`fables sheets --moods stinky`). In video they also act: anticipation and overshoot, hair that follows through, walk cycles, squash-and-stretch hops, listeners who watch the speaker, and a camera with parallax depth.
- **The rig.** Under every puppet is a full skeleton with IK elbows and knees, seven stances (stand, crouch, sit, kneel, jump, bow, tiptoe) and walk and run cycles. `fables rig stinky` shows it.
- **Styles.** Riso is the house default. `tv`, `bold`, `cut-paper`, `crayon` and `storybook` are alternative skins over the same puppets (`fables styles`). All six stay available: any render takes `--style` with one style, a comma list or `all`. A series still reads best in one look, so once a favourite emerges, set it in each book's front matter.
- **Type.**
  - **Story text: Andika**, a typeface designed by SIL for children learning to read, with a single-storey *a* and clear letter shapes.
  - **Shouted words** (`**NOPE**`): **Lilita One** in tomato red.
  - Both fonts are under the SIL Open Font License and are embedded in every output.
- **Text cards.** Story text sits on a wobbly paper card at the top of the page (or the bottom, with `text=bottom`). It reads over any art and becomes a recognisable series detail.

## Voice and tone

- Short sentences, built to be read aloud. Leave room for a pause and a page turn.
- Rule of three, always: three escalations, three callbacks.
- Give the kid a catchphrase (NOPE) and let the ending answer it.
- Funny for the grown-up reading it too ("and *he* once licked a slug").
- No shaming. The kid is never the butt of the joke; the habit is.
- End with something kids can do tonight.

## Next tales (loglines)

- **Bogey McBogeyface.** Bonnie's bogeys stick to everything: library books, the cat, the mayor. Her nose drips green on every page until she meets a tissue that's quicker than her finger. She becomes **Bonnie McBlowface**? (Find a better name.)
- **Grubby McGrubhands.** Gus never washes, so his high-fives leave handprints and the bakery's buns go brown. A bubble bath of a hand-wash turns him into **Sparkly McCleanhands**.
- **Shouty McShoutface.** Sasha's voice blows hats off and rattles windows, until a whisper game in the library makes her the best secret-keeper in town: **Whispy McWhisperface**.
- **Fibby McFibberpants.** Felix's trousers get hotter with every fib, until he's hopping. Telling the truth cools them (the sizzle into steam is the magic spread). He becomes **Honest McTruepants**.

Each needs one new effect in `src/art/fx.rs`, such as smouldering trousers or shockwave rings. Everything else already exists.
