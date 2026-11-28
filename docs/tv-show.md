# From picture book to TV show

This guide covers turning Kerfuffle into a show where every character has their own voice, sounds the same in every episode, acts with emotion and lip-syncs:

- where the voices come from;
- which APIs to use;
- what is already built into `fables`;
- what comes next.

Facts about providers were checked on 16 October 2026. Prices and policies change, so recheck them before you commit money.

## What is already built

| Piece | How it works |
| --- | --- |
| A voice per character | `[stinky.voice]` in `worlds/kerfuffle/cast.toml`. Once set, a character sounds the same in every line of every book. |
| Characters speak their own lines | With `[video] cast-voices = true` in a script, `"Brush your teeth!" said Mum.` is split. Mum's voice says the quote, the narrator says "said Mum.", and Mum's mouth only moves for her own words. |
| Show mode | `[video] drop-tags = true` drops short tags like "said Mum." so scenes play like a cartoon, not a read-aloud. |
| Emotion | The mood the script gives a character (`@stinky defiant`) becomes the line's delivery: an audio tag such as `[defiant]`, an acting instruction such as "say this defiantly, like a cheeky child refusing", or a change of pace. `**SHOUTED**` words are delivered as shouts. |
| Lip-sync | Nine mouth shapes from the Preston Blair chart that 2D studios use. With ElevenLabs, timings are exact per letter. Otherwise the text is paced through the voiced parts of the audio, so mouths close on pauses. |
| Acting | Listeners turn their eyes to the speaker. Speakers nod into stressed syllables and raise their brows on shouts. Moves wind up and overshoot, hair follows through, characters walk on and off, hops squash on landing, the camera bumps on big impacts, and the background has parallax depth. |
| Caching | Every take is stored by voice, text and mood, so editing one line re-records one line. API costs only apply to changed lines. |
| Providers | `say` (macOS, offline), `command` (any TTS program: Piper, mlx-audio, a script), `elevenlabs`, `openai`, `cartesia`. |
| ElevenLabs | Voice design from a casting note, sound effects and ambience as well as voices, and a macOS fallback when there is no key: see [`elevenlabs.md`](elevenlabs.md). |

```toml
# worlds/kerfuffle/cast.toml
[stinky.voice]
provider = "elevenlabs"
id = "<voice id from your ElevenLabs library>"
model = "eleven_v4"

[mum.voice]
provider = "openai"
name = "coral"
style = "A warm, brisk British mum who is running out of patience."
```

API keys are read from `ELEVENLABS_API_KEY` and `OPENAI_API_KEY`. Put the provider's key in the environment for the run:

```bash
ELEVENLABS_API_KEY=<key> fables all worlds/kerfuffle
```

A failed API call is reported once and never retried in a loop.

## Where voices come from

There are three routes. Most indie cartoons mix them.

### 1. Synthetic voices: fastest and cheapest

| Provider | Strength | Emotion control | Lip-sync timing | Price (cheapest paid tier) |
| --- | --- | --- | --- | --- |
| **ElevenLabs** | Best acting, Voice Design, a dialogue endpoint | Audio tags in the text: `[whispers]`, `[laughs]`, `[shouting]` | **Yes**, per character | v3/v4 about $80 per million characters |
| **OpenAI** (`gpt-4o-mini-tts`) | Natural delivery from plain-English direction | `instructions`, e.g. "sleepy, grumpy bear" | No | $0.60/M text tokens + $12/M audio tokens |
| **Inworld** (`inworld-tts-2`) | **Returns visemes directly** | `[say excitedly]` tags | Visemes and words | About $25 per million characters |
| **Cartesia** (`sonic-3.6`) | Fast; phoneme timestamps over SSE | `emotion` setting | Phonemes | About $37 per million characters |
| **Azure Neural TTS** | Some voices have `role="Girl"` / `"Boy"` | SSML `express-as` styles | Visemes via the SDK only | About $15/M (unconfirmed) |
| **Amazon Polly** | Official child voices (Ivy, Justin, Kevin) | Weak: prosody only | Visemes via speech marks | Neural $16/M |

**What an episode costs:** the Stinky script is about 2,400 characters, so voicing the whole book with ElevenLabs v4 is roughly **$0.20**, and an edit re-records only the changed lines. Voice quality, not money, is the constraint.

**Policy notes that matter for a kids' show:**

- **ElevenLabs:** commercial use needs a paid plan. Child-like voices are banned from the shared Voice Library. Whether you may design a child-like voice for private use is unconfirmed, so ask them, or follow the industry habit of adults voicing kids (below).
- **OpenAI:** you must tell listeners the voice is AI-generated. A line in the credits or the video description covers it.
- **Google Gemini TTS:** the API terms say it can't be used in a service "directed towards or likely to be accessed by" under-18s. Confirm with Google before a kids' channel depends on it.
- **Hume** shuts down its TTS API on 13 November 2026, so don't build on it.

**Running voices locally (free, offline, no account):**

- **Qwen3-TTS** (Apache-2.0) via [mlx-audio](https://github.com/Blaizzy/mlx-audio) on Apple silicon. It can design a voice from a description and take plain-language direction, which makes it the best local fit for characters.
- **Chatterbox** (MIT), which has an `exaggeration` dial for intensity. It watermarks its output.
- **Kokoro** (Apache-2.0) is a fine narrator but can't act.
- **Avoid for commercial work:** F5-TTS (non-commercial weights) and Piper (now GPL).

Any of these plugs in with the `command` provider. The simplest pattern is a small wrapper script per engine that reads the line on stdin and writes audio to the path it is given:

```toml
[stinky.voice]
provider = "command"
command = ["voices/qwen3.sh", "{voice}", "{mood}", "{out}"]
name = "stinky"   # passed as {voice}, e.g. to pick a reference clip or a voice description
```

The wrapper maps `{mood}` (e.g. `defiant`) onto whatever the engine accepts and writes any audio format to `{out}`. ffmpeg normalises it afterwards.

### 2. Perform it yourself, then convert the voice: the best acting for the money

1. Record yourself (or a friend) acting each line: the timing, the giggle, the sulk.
2. Run it through a voice changer: ElevenLabs speech-to-speech (`POST /v1/speech-to-speech/{voice_id}`, about $0.12 a minute) or Cartesia's voice changer.
3. The character's voice comes out with *your* performance: whispers, laughs and comic timing intact. Because timing is kept, lip-sync from the converted audio still lines up.

This is how a one-person studio gets real comic timing. It isn't built into `fables` yet; see [What comes next](#what-comes-next).

### 3. Human voice actors: a signature cast

- **Where:**
  - Voices.com and Voice123: vetted marketplaces.
  - Backstage: casting calls.
  - Casting Call Club: indie and hobby, lots of enthusiasm.
  - Fiverr: cheapest, uneven quality.
- **Who:** kids' characters are almost always voiced by adults. Child-labour rules limit studio hours, children's voices change, and a character can't age with them.
- **What it costs:** rates for a short indie piece are roughly $200–$1,200 flat (from secondary sources, unconfirmed). Union animation scale is about $1,060 per episode.
- **The contract:** get a buyout covering YouTube, books' companion videos and future episodes. **If you ever want to clone an actor's voice for synthetic lines, get explicit written consent and pay for that use separately.**

Human recordings drop straight into the pipeline as files once the `recorded` provider exists. Until then, a `command` provider that copies `takes/<line>.wav` works.

## Keeping every character consistent

1. **One voice per character, forever.** The voice lives in the cast file, never in a book. Commit `worlds/kerfuffle/cast.toml`, and treat changing a voice id like recasting a role.
2. **Design once.** Use ElevenLabs Voice Design (describe the character, get a voice id), or audition offline with `say`/Qwen3 first. Write the `style` line like a casting note; it is sent as acting direction on every line.
3. **Same mood, same delivery.** The mapping from mood to delivery is fixed in code (`src/video/voice.rs`), so "defiant" sounds the same in book 1 and book 9.
4. **Pronunciation.** Spell tricky names phonetically in `>` video-only lines if a voice stumbles ("Mc-STINK-breath").
5. **Takes don't drift.** Cached takes are reused until the text, mood or voice changes, so a re-render never quietly swaps a performance.

## Emotion, step by step

```text
@stinky defiant arms-crossed talk          ← the script sets mood and pose
"**NOPE**," said Stanley.                  ← the quote is Stanley's, the tag is the narrator's
```

| Layer | What happens |
| --- | --- |
| Voice | ElevenLabs gets `[shouting] NOPE`. OpenAI gets: "A cheeky, bouncy six-year-old… Say this line shouting it at the top of their voice." `say` speaks faster. |
| Face | Defiant mood: half-lidded eyes, a smug brow, mouth shapes from the lip-sync track. |
| Body | Arms crossed with fists; a head nod into the stressed syllable; brows lift on the shout. |
| Others | Mum's eyes turn to Stanley while he speaks. |
| Sound | Music ducks under speech. |

## Tools for consistent cartoon characters, and why `fables` builds its own

| Tool | Best at | Pipeline-friendly? |
| --- | --- | --- |
| **Toon Boom Harmony** | Broadcast TV, cut-out and hand-drawn | Batch rendering from scripts; $30–139 a month |
| **Moho Pro** | Vector bone rigs; imports lip-sync data | Command-line rendering exists |
| **Cartoon Animator 5** | Fast puppets, 360° heads, automatic lip-sync | No scripting; GUI only |
| **Adobe Character Animator** | Live webcam and mic puppetry | No CLI |
| **Spine** | Skeletal game animation | CLI export, but needs a display |
| **Rive** | Interactive, state-machine characters for apps | The CLI renders **one frame per run**; no video export |

All of them are rig-based cut-out animation: build a character once from parts, then swap mouths and expressions. `fables` does the same thing in code, which is why it can:

- turn a script straight into a book, an ePub, a print file and two videos without a GUI;
- render headlessly and in parallel;
- keep every output identical on every run.

Use Harmony or Moho if you hire animators for a hand-polished special. Use **Rive** for an interactive Flutter app of the books, where kids tap Stinky and he breathes on them. The cast's parts and moods map naturally onto Rive state machines.

## Drawn art on the same rig

The six built-in styles are drawn in code. For hand-drawn, illustrator-quality characters, keep everything else (rig, scripts, voices, lip-sync, books, videos) and swap the art for **drawn parts pinned to the skeleton**. TV studios do the same with Harmony or Moho.

### The parts a rig needs (brief for an illustrator or an image model)

| Part | Variants |
| --- | --- |
| Head | Front and 3/4 left/right, plus hair front and back as separate layers. |
| Eyes | Open, half-lidded, closed, happy arcs, wide, X (knocked out), with pupils on their own layer. |
| Brows | Neutral, raised, angry, worried, smug. |
| Mouths | The nine lip-sync shapes (rest, MBP, teeth, open, wide, round, pucker, F/V, tongue), plus smile, frown and grin. |
| Torso | One per outfit, drawn standing straight. |
| Arms | Upper arm, forearm and hands (open, fist, point), each with its joint pivot marked. |
| Legs | Thigh, shin and shoe, with pivots. |
| Props | Toothbrush, parcel, jam jar… |

**Delivery:** layered SVG, or PNGs at 4× with a small file of pivot coordinates, in a consistent naming scheme. A new skin type in `src/art/` would place each part on its bone, exactly where the code-drawn parts go today.

### With an image model (checked October 2026)

1. **Design sheet.** Generate a turnaround and expression sheet for each character. Then generate every part as a masked edit of that same sheet, so the character stays on-model.
   - **gpt-image-2.5** supports transparent backgrounds and masked edits. About $0.05–0.20 per image.
   - **Nano Banana 2.1** handles up to 14 reference images. About $0.03 per image, with a SynthID watermark.
2. **Split and vectorise.**
   - SAM 3 ($0.005) or Qwen-Image-Layered ($0.05) separates parts.
   - The Rust crate `vtracer` turns them into SVG locally, so they print crisply at any size.
3. **Rig.** Pin the parts to the skeleton. Everything else in the pipeline is unchanged.
4. **Disclose.** KDP requires you to disclose AI-generated images, covers included. YouTube needs no label for animated content.

**A whole series:** train a FLUX.2 LoRA per character (about $6 on fal) for the most stable on-model results across many books.

**AI video models** (Veo 3.1, Kling) can animate stills, but they drift off-model and Veo blocks child characters from reference images. Rigging drawn parts is cheaper and stays consistent.

### With an illustrator

Hand them the parts list above and the rig sheet (`fables rig <id>`) as the pose reference. One character's full parts set is a few days' work. The art is wholly yours, and it needs no AI disclosure.

## Publishing quickly without getting demonetised

YouTube labels AI only for *realistic* content, so animated characters with synthetic voices generally need no label. You must still set **"Made for kids"** on every video.

The real risk is the **"inauthentic content"** policy against mass-produced, templated videos (2026 enforcement reports are from secondary sources and unconfirmed). Speed is fine; sameness isn't. Every episode should have its own story, jokes and art beats. The recipe is a skeleton, not a script generator.

The fast loop:

```bash
fables new "Bogey McBogeyface" --star bogey                        # a 12-beat recipe skeleton
fables proofs worlds/kerfuffle/stories/bogey-mcbogeyface/story.md  # iterate on words and staging (about 1 second)
fables video worlds/kerfuffle/stories/bogey-mcbogeyface/story.md --preview --format portrait
fables all worlds/kerfuffle                                        # every book: ePub, print PDFs, both videos
```

## What comes next

These are listed roughly in order of payoff for a show:

1. **Recorded takes and voice changing.** A `recorded` provider that reads `takes/<book>/<line>.wav` and optionally converts it to the character's voice through ElevenLabs speech-to-speech. Your performance, the character's voice.
2. **Dialogue-first scripts.** A `Stinky: Nope!` line form for episodes that aren't books, with automatic `talk` and shot changes.
3. **Shot grammar.** Over-the-shoulder and two-shot framings, reaction close-ups and cut-on-action, so dialogue scenes edit like TV.
4. **Character voices in one call.** ElevenLabs text-to-dialogue (up to 10 voices per request) for more natural back-and-forth.
5. **An episode intro and theme.** A recurring title sequence and a theme sung over the synthesised ukulele.
6. **Rive export** of the cast for an interactive picture-book app.
