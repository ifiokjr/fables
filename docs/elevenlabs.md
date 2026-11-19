# ElevenLabs: real voices and sound for the cartoons

fables can voice every character, the narrator, every sound effect and every ambience bed with [ElevenLabs](https://elevenlabs.io). This guide goes from no account to a finished cartoon.

Everything is optional. Without a key, a book sounds exactly as it does today: macOS voices, and synthesised effects, music and ambience.

## 1. Get a key

1. Make a key at [elevenlabs.io/app/settings/api-keys](https://elevenlabs.io/app/settings/api-keys). If you restrict it, allow **Text to Speech**, **Sound Effects**, **Voices** (read and write, for designing voices) and **User** (read, so fables can tell you how much of your plan is left).
2. Give it to the run, never to a file in the repository:

   ```bash
   export ELEVENLABS_API_KEY=sk_…        # in ~/.zshrc, or just for one run:
   ELEVENLABS_API_KEY=sk_… fables video worlds/kerfuffle/stories/stinky-mcstinkbreath/story.md
   ```

3. `fables voice providers` shows `elevenlabs ready` once the key is there.

For a data-residency account, point fables at its host with `ELEVENLABS_API_URL`, e.g. `https://api.eu.residency.elevenlabs.io`.

## 2. Cast the voices

Each character's voice lives in the world's `cast.toml`, so they sound the same in every book. There are two ways to get one.

**Pick one from your library.** `fables voice list --provider elevenlabs` lists your voices with their ids and labels; `--search grandad` narrows it. Add any voice from the [Voice Library](https://elevenlabs.io/app/voice-library) to your account first to see it here.

**Design one from a casting note.** This is how a cartoon gets voices nobody else has:

```bash
fables voice design mum "A warm, brisk British mum in her thirties who is running out of patience, crisp and quick"
#   1. 6.1s  out/voices/mum-design-1.mp3
#   2. 5.8s  out/voices/mum-design-2.mp3
#   3. 6.4s  out/voices/mum-design-3.mp3
fables voice keep mum 2
```

`keep` saves the voice to your ElevenLabs library and writes `[mum.voice]` into the cast file. If Mum used to speak with a macOS voice, it is kept as her `fallback`. ElevenLabs does not allow child-like voices in its shared library, and may refuse to design one; describe an adult performing the child, as animation studios cast them ("a playful adult actress doing a cheeky six-year-old boy, bright and bouncy").

The whole table, with every setting optional except `id`:

```toml
[stinky.voice]
provider = "elevenlabs"
id = "<voice id>"
model = "eleven_v4"     # the default: the most expressive model, and it takes audio tags
stability = 0.35        # 0 to 1: lower acts more, higher holds steadier
similarity = 0.8        # 0 to 1: how closely a take keeps to the voice
fallback = "Junior"     # the macOS voice that reads where ElevenLabs cannot (default Daniel)
# exaggeration = 0.3    # 0 to 1, and speed = 0.7 to 1.2: older models only, not eleven_v4
```

The narrator is the same table under `[narrator]` in `world.toml`, or `[voice]` in one script's front matter.

Hear any of them before a whole book: `fables voice audition stinky "NOPE!" --mood defiant`.

## 3. Let the script act

Nothing in the script changes. The mood a direction gives a character becomes the line's delivery:

| Script | What ElevenLabs is asked for |
| --- | --- |
| `@stinky defiant talk` then `"Nope," said Stanley.` | `[defiant] Nope,` |
| `"**NOPE**," said Stanley.` | `[shouting] NOPE,` |
| `@mum cross talk` | `[angry] …` |
| The narrator | the line as written |

`eleven_v3` and `eleven_v4` take these audio tags. Older models (`eleven_multilingual_v2`, `eleven_flash_v2_5`) would read a tag aloud, so they get the mood as a pace instead: slower when sad or sleepy, quicker when cross.

Every line comes back with the time of every letter, so mouths lip-sync exactly.

## 4. Sound effects and ambience

Add a `[sound]` table to `world.toml` (or to one script's front matter):

```toml
[sound]
provider = "elevenlabs"

[sound.effects]            # optional: what an effect sounds like in this world
bonk = "a frying pan clanging off a cartoon head"

[sound.ambience]           # optional: what a bed sounds like in this world
waves = "a busy harbour: lapping water, rigging clinking, distant gulls"
```

Every `@sfx` cue and every ambience bed is then recorded by ElevenLabs instead of synthesised. Each of the 18 effects and 8 beds has a built-in description, so the table above is all a world needs. Beds are recorded as seamless 20-second loops.

A script can describe a one-off sound after the effect that stands in for it when nothing can be recorded:

```text
@sfx bonk "a wooden spoon tapping a saucepan, twice"
```

Recorded effects are trimmed to land on their cue and brought to the level of the built-in ones; music still ducks under speech, and ambience still sits about 25 dB under the narrator.

## 5. Make the cartoon

```bash
fables video worlds/kerfuffle/stories/stinky-mcstinkbreath/story.md
#   recording 31 new lines (2,412 characters) with ElevenLabs; 0 already recorded
#   ElevenLabs plan: 27,588 of 30,000 characters left this period
#   voiced 31 lines (4 character voices), 142s total
#   recording 9 sounds with ElevenLabs; 0 already recorded
```

Before it records, fables says how many characters it is about to spend and how many your plan has left, and warns if the book needs more than that. New lines are recorded three at a time.

## What it costs, and why a re-render is free

- Every take is cached in `out/<book>/cache/voice`, keyed by the voice, the mood and the words; every sound in `out/<book>/cache/sound`, keyed by its description. Editing one line re-records one line; re-rendering, or rendering in another page skin, records nothing.
- Each take is asked for with a seed made from its cache key, so a line recorded again after the cache is cleared comes back as close to the first take as the service allows.
- A busy reply from ElevenLabs (too many requests at once) made no audio, so fables waits and asks again, up to four times. Any other refusal (a wrong voice id, an empty plan) stops the build with ElevenLabs' own message and is never retried. The takes that did record are kept, so the next run only does the rest.
- A Kerfuffle book is about 2,400 characters. Check [elevenlabs.io/pricing](https://elevenlabs.io/pricing) for what that is on your plan.

## When ElevenLabs cannot speak

A build without the key, or offline, never stops for it:

- Lines and sounds already recorded play as recorded.
- A new line is read by the voice's `fallback` macOS voice (Daniel if it names none), and the build says so once per voice. On a machine without `say`, the line is silent and timed by its estimate.
- A new sound plays the built-in synthesised effect or bed.

Once the key is back, the next build records just those lines and sounds in the real voices.

## Over MCP

An agent driving `fables mcp` has the same tools: `voice_library` lists the account's voices, `voice_design` returns the numbered previews as audio, `voice_keep` keeps one and casts it, `voice_cast` takes every setting above, and `voice_audition` plays a line back.
